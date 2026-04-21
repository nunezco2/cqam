//! Binary file I/O for the `.cqb` (CQAM Binary) format.
//!
//! File layout (v2):
//!   Header   (12 bytes, little-endian)
//!   Code     (code_length * 4 bytes, little-endian u32 words)
//!   CQDT     (optional, data cells)
//!   CQMD     (always present in v2, program metadata)
//!   CQSH     (optional, shared section cells)
//!   CQPV     (optional, per-thread private size)
//!   CQDB     (optional, debug symbols — must be last)
//!
//! See `design/specs/BINARY_DATA_SPEC.md` for the full specification.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

use cqam_core::error::CqamError;
use cqam_core::parser::ProgramMetadata;

use crate::assembler::AssemblyResult;

// =============================================================================
// Constants
// =============================================================================

/// Magic bytes at the start of every .cqb file.
const CQB_MAGIC: [u8; 4] = *b"CQAM";

/// Magic bytes at the start of the optional debug section.
const DEBUG_MAGIC: [u8; 4] = *b"CQDB";

/// Magic bytes for the data section.
const DATA_MAGIC: [u8; 4] = *b"CQDT";

/// Magic bytes for the metadata section.
const METADATA_MAGIC: [u8; 4] = *b"CQMD";

/// Magic bytes for the shared section.
const SHARED_MAGIC: [u8; 4] = *b"CQSH";

/// Magic bytes for the private section.
const PRIVATE_MAGIC: [u8; 4] = *b"CQPV";

/// Current binary format version.
const CQB_VERSION: u16 = 2;

/// Header size in bytes.
const HEADER_SIZE: usize = 12;

// =============================================================================
// Public types
// =============================================================================

/// A fully loaded .cqb binary image.
///
/// This is the in-memory representation of a .cqb file after reading.
/// It can be passed to the VM for execution or to the disassembler for
/// text output.
#[derive(Debug, Clone)]
pub struct CqbImage {
    /// Binary format version (1 or 2).
    pub version: u16,

    /// Word offset of the first non-label instruction.
    ///
    /// The VM should set the initial PC to this value.
    pub entry_point: u16,

    /// Encoded instruction words (one per instruction).
    pub code: Vec<u32>,

    /// Optional debug symbol table: label_id -> label_name.
    ///
    /// Present only if the .cqb file was assembled with `--debug`.
    /// Used by the disassembler to restore human-readable label names.
    pub debug_symbols: Option<HashMap<u16, String>>,

    /// Pre-loaded data cells from the `.data` section (empty if absent).
    pub data_cells: Vec<i64>,

    /// Program metadata from `#!` pragma directives.
    pub metadata: ProgramMetadata,

    /// Pre-loaded data cells from the `.shared` section (empty if absent).
    pub shared_cells: Vec<i64>,

    /// Base address of the shared section in CMEM.
    pub shared_base: u16,

    /// Per-thread private memory size in cells (0 if no `.private` section).
    pub private_size: u16,
}

// =============================================================================
// Writing
// =============================================================================

/// Write a .cqb binary to a generic writer.
///
/// # Layout (v2)
///
/// ```text
/// Offset  Size  Field
/// 0       4     magic = b"CQAM"
/// 4       2     version = 2 (u16 LE)
/// 6       2     entry_point (u16 LE)
/// 8       4     code_length (u32 LE, number of instruction words)
/// 12      N*4   code words (u32 LE each)
/// ...     ...   optional CQDT / CQMD / CQSH / CQPV sections
/// ...     ...   optional CQDB debug section (must be last)
/// ```
///
/// # Arguments
///
/// * `writer` - Destination for the binary data.
/// * `result` - The assembly result containing code and metadata.
/// * `include_debug` - If true, append the debug string table.
///
/// # Errors
///
/// Returns `CqamError::IoError` on write failure.
pub fn write_cqb<W: Write>(
    writer: &mut W,
    result: &AssemblyResult,
    include_debug: bool,
) -> Result<(), CqamError> {
    // -- Header (12 bytes) ---------------------------------------------------
    writer.write_all(&CQB_MAGIC)?;
    writer.write_all(&CQB_VERSION.to_le_bytes())?;
    writer.write_all(&result.entry_point.to_le_bytes())?;
    writer.write_all(&(result.code.len() as u32).to_le_bytes())?;

    // -- Code section --------------------------------------------------------
    for &word in &result.code {
        writer.write_all(&word.to_le_bytes())?;
    }

    // -- Data section (present when data_cells is non-empty) -----------------
    if !result.data_cells.is_empty() {
        writer.write_all(&DATA_MAGIC)?;
        writer.write_all(&(result.data_cells.len() as u32).to_le_bytes())?;
        for &cell in &result.data_cells {
            writer.write_all(&cell.to_le_bytes())?;
        }
    }

    // -- Metadata section (always written in v2) -----------------------------
    writer.write_all(&METADATA_MAGIC)?;
    writer.write_all(&[result.metadata.qubits.unwrap_or(0)])?;
    writer.write_all(&result.metadata.threads.unwrap_or(0).to_le_bytes())?;

    // -- Shared section (present when shared_cells is non-empty) -------------
    if !result.shared_cells.is_empty() {
        writer.write_all(&SHARED_MAGIC)?;
        writer.write_all(&result.shared_base.to_le_bytes())?;
        writer.write_all(&(result.shared_cells.len() as u32).to_le_bytes())?;
        for &cell in &result.shared_cells {
            writer.write_all(&cell.to_le_bytes())?;
        }
    }

    // -- Private section (present when private_size > 0) ---------------------
    if result.private_size > 0 {
        writer.write_all(&PRIVATE_MAGIC)?;
        writer.write_all(&result.private_size.to_le_bytes())?;
    }

    // -- Optional debug section (must remain last) ---------------------------
    if include_debug && !result.debug_symbols.is_empty() {
        write_debug_section(writer, &result.debug_symbols)?;
    }

    Ok(())
}

/// Write the debug section to a writer.
///
/// # Layout
///
/// ```text
/// Offset  Size       Field
/// 0       4          debug_magic = b"CQDB"
/// 4       2          num_entries (u16 LE)
/// 6..     variable   entries: [id:u16 LE][len:u16 LE][name: UTF-8 bytes]
/// ```
fn write_debug_section<W: Write>(
    writer: &mut W,
    symbols: &[(u16, String)],
) -> Result<(), CqamError> {
    writer.write_all(&DEBUG_MAGIC)?;
    writer.write_all(&(symbols.len() as u16).to_le_bytes())?;

    for (id, name) in symbols {
        let name_bytes = name.as_bytes();
        writer.write_all(&id.to_le_bytes())?;
        writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
        writer.write_all(name_bytes)?;
    }

    Ok(())
}

/// Write a .cqb file to a filesystem path.
///
/// Convenience wrapper around `write_cqb` that handles file creation.
///
/// # Errors
///
/// Returns `CqamError::IoError` if the file cannot be created or written.
pub fn write_cqb_file(
    path: &Path,
    result: &AssemblyResult,
    include_debug: bool,
) -> Result<(), CqamError> {
    let mut file = std::fs::File::create(path)?;
    write_cqb(&mut file, result, include_debug)
}

// =============================================================================
// Reading
// =============================================================================

/// Read a .cqb binary from a generic reader.
///
/// Accepts version 1 (legacy, code + optional CQDB) and version 2 (code +
/// optional CQDT/CQMD/CQSH/CQPV + optional CQDB).  Sections are identified
/// by their 4-byte magic tags; the reader loops until EOF.
///
/// # Errors
///
/// - `CqamError::InvalidBinary` if the magic bytes are wrong, the version
///   is unsupported, or the file is truncated.
/// - `CqamError::IoError` on read failure.
pub fn read_cqb<R: Read>(reader: &mut R) -> Result<CqbImage, CqamError> {
    // -- Read header (12 bytes) ----------------------------------------------
    let mut header = [0u8; HEADER_SIZE];
    reader
        .read_exact(&mut header)
        .map_err(|_| CqamError::InvalidBinary("File too short for header".to_string()))?;

    // Validate magic
    if header[0..4] != CQB_MAGIC {
        return Err(CqamError::InvalidBinary(format!(
            "Bad magic: expected {:?}, got {:?}",
            CQB_MAGIC,
            &header[0..4]
        )));
    }

    // Parse header fields
    let version = u16::from_le_bytes([header[4], header[5]]);
    if version > 2 {
        return Err(CqamError::InvalidBinary(format!(
            "Unsupported version: {} (max supported: 2)",
            version
        )));
    }

    let entry_point = u16::from_le_bytes([header[6], header[7]]);
    let code_length = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);

    // -- Read code section ---------------------------------------------------
    let mut code = Vec::with_capacity(code_length as usize);
    let mut word_buf = [0u8; 4];
    for _ in 0..code_length {
        reader.read_exact(&mut word_buf).map_err(|_| {
            CqamError::InvalidBinary("File truncated in code section".to_string())
        })?;
        code.push(u32::from_le_bytes(word_buf));
    }

    // -- Loop-based trailing-section reader ----------------------------------
    // Handles both v1 (only CQDB may follow) and v2 (CQDT/CQMD/CQSH/CQPV/CQDB).
    let mut data_cells: Vec<i64> = Vec::new();
    let mut metadata = ProgramMetadata::default();
    let mut shared_cells: Vec<i64> = Vec::new();
    let mut shared_base: u16 = 0;
    let mut private_size: u16 = 0;
    let mut debug_symbols: Option<HashMap<u16, String>> = None;

    loop {
        let mut magic_buf = [0u8; 4];
        match reader.read_exact(&mut magic_buf) {
            Ok(()) => {}
            Err(_) => break, // EOF — no more sections
        }

        if magic_buf == DATA_MAGIC {
            let mut count_buf = [0u8; 4];
            reader.read_exact(&mut count_buf).map_err(|_| {
                CqamError::InvalidBinary("CQDT section truncated at count".into())
            })?;
            let num = u32::from_le_bytes(count_buf) as usize;
            data_cells = Vec::with_capacity(num);
            let mut cell_buf = [0u8; 8];
            for _ in 0..num {
                reader.read_exact(&mut cell_buf).map_err(|_| {
                    CqamError::InvalidBinary("CQDT section truncated in cells".into())
                })?;
                data_cells.push(i64::from_le_bytes(cell_buf));
            }
        } else if magic_buf == METADATA_MAGIC {
            let mut qubits_buf = [0u8; 1];
            reader.read_exact(&mut qubits_buf).map_err(|_| {
                CqamError::InvalidBinary("CQMD section truncated at qubits".into())
            })?;
            let mut threads_buf = [0u8; 2];
            reader.read_exact(&mut threads_buf).map_err(|_| {
                CqamError::InvalidBinary("CQMD section truncated at threads".into())
            })?;
            let q = qubits_buf[0];
            let t = u16::from_le_bytes(threads_buf);
            metadata.qubits = if q == 0 { None } else { Some(q) };
            metadata.threads = if t == 0 { None } else { Some(t) };
        } else if magic_buf == SHARED_MAGIC {
            let mut base_buf = [0u8; 2];
            reader.read_exact(&mut base_buf).map_err(|_| {
                CqamError::InvalidBinary("CQSH section truncated at base".into())
            })?;
            shared_base = u16::from_le_bytes(base_buf);
            let mut count_buf = [0u8; 4];
            reader.read_exact(&mut count_buf).map_err(|_| {
                CqamError::InvalidBinary("CQSH section truncated at count".into())
            })?;
            let num = u32::from_le_bytes(count_buf) as usize;
            shared_cells = Vec::with_capacity(num);
            let mut cell_buf = [0u8; 8];
            for _ in 0..num {
                reader.read_exact(&mut cell_buf).map_err(|_| {
                    CqamError::InvalidBinary("CQSH section truncated in cells".into())
                })?;
                shared_cells.push(i64::from_le_bytes(cell_buf));
            }
        } else if magic_buf == PRIVATE_MAGIC {
            let mut size_buf = [0u8; 2];
            reader.read_exact(&mut size_buf).map_err(|_| {
                CqamError::InvalidBinary("CQPV section truncated at size".into())
            })?;
            private_size = u16::from_le_bytes(size_buf);
        } else if magic_buf == DEBUG_MAGIC {
            // Magic already consumed; read the body directly.
            debug_symbols = read_debug_section_body(reader)?;
        } else {
            return Err(CqamError::InvalidBinary(format!(
                "Unknown section magic: {:?}",
                magic_buf
            )));
        }
    }

    Ok(CqbImage {
        version,
        entry_point,
        code,
        debug_symbols,
        data_cells,
        metadata,
        shared_cells,
        shared_base,
        private_size,
    })
}

/// Read the body of a debug section (after the magic has already been consumed).
///
/// Returns `Some(map)` on success.
fn read_debug_section_body<R: Read>(
    reader: &mut R,
) -> Result<Option<HashMap<u16, String>>, CqamError> {
    // Read number of entries
    let mut count_buf = [0u8; 2];
    reader.read_exact(&mut count_buf).map_err(|_| {
        CqamError::InvalidBinary("Debug section truncated at count".to_string())
    })?;
    let num_entries = u16::from_le_bytes(count_buf);

    let mut map = HashMap::with_capacity(num_entries as usize);
    for _ in 0..num_entries {
        // Read id
        let mut id_buf = [0u8; 2];
        reader.read_exact(&mut id_buf).map_err(|_| {
            CqamError::InvalidBinary("Debug section truncated at entry id".to_string())
        })?;
        let id = u16::from_le_bytes(id_buf);

        // Read name length
        let mut len_buf = [0u8; 2];
        reader.read_exact(&mut len_buf).map_err(|_| {
            CqamError::InvalidBinary("Debug section truncated at entry length".to_string())
        })?;
        let name_len = u16::from_le_bytes(len_buf) as usize;

        // Read name bytes
        let mut name_buf = vec![0u8; name_len];
        reader.read_exact(&mut name_buf).map_err(|_| {
            CqamError::InvalidBinary("Debug section truncated at entry name".to_string())
        })?;

        let name = String::from_utf8(name_buf).map_err(|e| {
            CqamError::InvalidBinary(format!("Invalid UTF-8 in debug entry {}: {}", id, e))
        })?;

        map.insert(id, name);
    }

    Ok(Some(map))
}

/// Read a .cqb file from a filesystem path.
///
/// Convenience wrapper around `read_cqb` that handles file opening.
///
/// # Errors
///
/// Returns `CqamError::IoError` if the file cannot be opened, or
/// `CqamError::InvalidBinary` if the file content is malformed.
pub fn read_cqb_file(path: &Path) -> Result<CqbImage, CqamError> {
    let mut file = std::fs::File::open(path)?;
    read_cqb(&mut file)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal AssemblyResult for testing.
    fn minimal_result() -> AssemblyResult {
        AssemblyResult {
            code: vec![0x0C00002A, 0x2B000000], // ILDI R0, 42; HALT
            labels: HashMap::new(),
            debug_symbols: Vec::new(),
            entry_point: 0,
            data_cells: Vec::new(),
            metadata: ProgramMetadata::default(),
            shared_cells: Vec::new(),
            shared_base: 0,
            private_size: 0,
        }
    }

    /// Build a result with debug symbols for testing.
    fn result_with_debug() -> AssemblyResult {
        let mut labels = HashMap::new();
        labels.insert("start".to_string(), 0);
        AssemblyResult {
            code: vec![0x2C000000, 0x0C00002A, 0x2B000000],
            labels,
            debug_symbols: vec![(0, "start".to_string())],
            entry_point: 1,
            data_cells: Vec::new(),
            metadata: ProgramMetadata::default(),
            shared_cells: Vec::new(),
            shared_base: 0,
            private_size: 0,
        }
    }

    #[test]
    fn test_roundtrip_no_debug() {
        let result = minimal_result();
        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, false).unwrap();

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert_eq!(image.version, CQB_VERSION);
        assert_eq!(image.entry_point, 0);
        assert_eq!(image.code, result.code);
        assert!(image.debug_symbols.is_none());
    }

    #[test]
    fn test_roundtrip_with_debug() {
        let result = result_with_debug();
        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, true).unwrap();

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert_eq!(image.version, CQB_VERSION);
        assert_eq!(image.entry_point, 1);
        assert_eq!(image.code, result.code);

        let debug = image.debug_symbols.unwrap();
        assert_eq!(debug.get(&0), Some(&"start".to_string()));
    }

    #[test]
    fn test_bad_magic() {
        let buf = b"BADM\x01\x00\x00\x00\x00\x00\x00\x00";
        let result = read_cqb(&mut &buf[..]);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_header() {
        let buf = b"CQAM\x01";
        let result = read_cqb(&mut &buf[..]);
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_version() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&CQB_MAGIC);
        buf.extend_from_slice(&99u16.to_le_bytes()); // unsupported version
        buf.extend_from_slice(&0u16.to_le_bytes()); // entry_point
        buf.extend_from_slice(&0u32.to_le_bytes()); // code_length
        let result = read_cqb(&mut &buf[..]);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_code() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&CQB_MAGIC);
        buf.extend_from_slice(&CQB_VERSION.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes()); // entry_point
        buf.extend_from_slice(&5u32.to_le_bytes()); // claim 5 words...
        // ...but provide only 2
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        let result = read_cqb(&mut &buf[..]);
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // New v2 round-trip tests
    // -------------------------------------------------------------------------

    /// Hand-build a v1 binary (header version=1, code, then CQDB debug section).
    /// Assert it loads correctly with the v2 reader, with empty data/metadata defaults.
    #[test]
    fn test_v1_binary_loads_with_v2_reader() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&CQB_MAGIC);
        buf.extend_from_slice(&1u16.to_le_bytes()); // version 1
        buf.extend_from_slice(&0u16.to_le_bytes()); // entry_point
        buf.extend_from_slice(&1u32.to_le_bytes()); // code_length = 1
        buf.extend_from_slice(&0x2B000000u32.to_le_bytes()); // HALT word

        // CQDB debug section
        buf.extend_from_slice(&DEBUG_MAGIC);
        buf.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
        buf.extend_from_slice(&0u16.to_le_bytes()); // id=0
        let name = b"main";
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
        buf.extend_from_slice(name);

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert_eq!(image.version, 1);
        assert_eq!(image.code.len(), 1);
        assert!(image.data_cells.is_empty());
        assert!(image.metadata.qubits.is_none());
        assert!(image.metadata.threads.is_none());
        assert!(image.shared_cells.is_empty());
        assert_eq!(image.shared_base, 0);
        assert_eq!(image.private_size, 0);
        let debug = image.debug_symbols.unwrap();
        assert_eq!(debug.get(&0), Some(&"main".to_string()));
    }

    /// Round-trip: data section is serialized and deserialized correctly.
    #[test]
    fn test_roundtrip_data_section() {
        let mut result = minimal_result();
        result.data_cells = vec![72, 101, 108, 108, 111, 0, 42, 99]; // "Hello\0", 42, 99

        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, false).unwrap();

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert_eq!(image.data_cells, result.data_cells);
    }

    /// Round-trip: metadata (qubits + threads) survives write/read.
    #[test]
    fn test_roundtrip_metadata() {
        let mut result = minimal_result();
        result.metadata.qubits = Some(4);
        result.metadata.threads = Some(8);

        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, false).unwrap();

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert_eq!(image.metadata.qubits, Some(4));
        assert_eq!(image.metadata.threads, Some(8));
    }

    /// Round-trip: metadata defaults (None/None) round-trip as None/None.
    #[test]
    fn test_roundtrip_metadata_defaults() {
        let result = minimal_result();

        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, false).unwrap();

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert!(image.metadata.qubits.is_none());
        assert!(image.metadata.threads.is_none());
    }

    /// Round-trip: shared section cells and base address survive write/read.
    #[test]
    fn test_roundtrip_shared_section() {
        let mut result = minimal_result();
        result.shared_cells = vec![1, 2, 3];
        result.shared_base = 64;

        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, false).unwrap();

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert_eq!(image.shared_cells, vec![1, 2, 3]);
        assert_eq!(image.shared_base, 64);
    }

    /// Round-trip: private_size survives write/read.
    #[test]
    fn test_roundtrip_private_section() {
        let mut result = minimal_result();
        result.private_size = 16;

        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, false).unwrap();

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert_eq!(image.private_size, 16);
    }

    /// Round-trip: all sections together (data, metadata, shared, private, debug).
    #[test]
    fn test_roundtrip_all_sections() {
        let mut result = result_with_debug();
        result.data_cells = vec![10, 20, 30];
        result.metadata.qubits = Some(2);
        result.metadata.threads = Some(4);
        result.shared_cells = vec![100, 200];
        result.shared_base = 128;
        result.private_size = 8;

        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, true).unwrap();

        let image = read_cqb(&mut &buf[..]).unwrap();
        assert_eq!(image.code, result.code);
        assert_eq!(image.entry_point, result.entry_point);
        assert_eq!(image.data_cells, vec![10, 20, 30]);
        assert_eq!(image.metadata.qubits, Some(2));
        assert_eq!(image.metadata.threads, Some(4));
        assert_eq!(image.shared_cells, vec![100, 200]);
        assert_eq!(image.shared_base, 128);
        assert_eq!(image.private_size, 8);
        let debug = image.debug_symbols.unwrap();
        assert_eq!(debug.get(&0), Some(&"start".to_string()));
    }

    /// When data_cells is empty, CQDT magic must not appear in the binary.
    #[test]
    fn test_empty_data_not_written() {
        let result = minimal_result();
        assert!(result.data_cells.is_empty());

        let mut buf = Vec::new();
        write_cqb(&mut buf, &result, false).unwrap();

        // Scan buf for DATA_MAGIC bytes
        let data_magic_bytes = &DATA_MAGIC;
        let found = buf.windows(4).any(|w| w == data_magic_bytes);
        assert!(!found, "CQDT magic should not appear when data_cells is empty");
    }

    /// An unknown 4-byte magic tag after code must cause an error.
    #[test]
    fn test_unknown_section_magic_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&CQB_MAGIC);
        buf.extend_from_slice(&CQB_VERSION.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes()); // entry_point
        buf.extend_from_slice(&0u32.to_le_bytes()); // code_length = 0
        // Unknown magic
        buf.extend_from_slice(b"????");

        let result = read_cqb(&mut &buf[..]);
        assert!(
            result.is_err(),
            "Unknown section magic should be rejected"
        );
    }
}
