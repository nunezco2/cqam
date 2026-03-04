// cqam-as/src/binary.rs
//
// Phase 5: Binary file I/O for the .cqb (CQAM Binary) format.
//
// File layout:
//   Header   (12 bytes, little-endian)
//   Code     (code_length * 4 bytes, little-endian u32 words)
//   Debug    (optional, starts with b"CQDB" magic)
//
// See design/phase5_design.md section 5 for the full specification.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

use cqam_core::error::CqamError;

use crate::assembler::AssemblyResult;

// =============================================================================
// Constants
// =============================================================================

/// Magic bytes at the start of every .cqb file.
const CQB_MAGIC: [u8; 4] = *b"CQAM";

/// Magic bytes at the start of the optional debug section.
const DEBUG_MAGIC: [u8; 4] = *b"CQDB";

/// Current binary format version.
const CQB_VERSION: u16 = 1;

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
    /// Binary format version (currently always 1).
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
}

// =============================================================================
// Writing
// =============================================================================

/// Write a .cqb binary to a generic writer.
///
/// # Layout
///
/// ```text
/// Offset  Size  Field
/// 0       4     magic = b"CQAM"
/// 4       2     version = 1 (u16 LE)
/// 6       2     entry_point (u16 LE)
/// 8       4     code_length (u32 LE, number of instruction words)
/// 12      N*4   code words (u32 LE each)
/// 12+N*4  ...   optional debug section
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

    // -- Optional debug section ----------------------------------------------
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
/// Validates the magic bytes and version, then reads the code section
/// and optional debug section.
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
    if version != CQB_VERSION {
        return Err(CqamError::InvalidBinary(format!(
            "Unsupported version: {} (expected {})",
            version, CQB_VERSION
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

    // -- Try to read optional debug section ----------------------------------
    let debug_symbols = read_debug_section(reader)?;

    Ok(CqbImage {
        version,
        entry_point,
        code,
        debug_symbols,
    })
}

/// Attempt to read the optional debug section.
///
/// Returns `Some(map)` if a valid debug section is present, `None` otherwise.
/// Does not treat absence of a debug section as an error.
fn read_debug_section<R: Read>(reader: &mut R) -> Result<Option<HashMap<u16, String>>, CqamError> {
    // Try to read the debug magic
    let mut magic_buf = [0u8; 4];
    match reader.read_exact(&mut magic_buf) {
        Ok(()) => {}
        Err(_) => return Ok(None), // No more data: no debug section
    }

    if magic_buf != DEBUG_MAGIC {
        // Trailing data that is not a debug section. This is not an error;
        // the file may have been padded or extended by a future version.
        return Ok(None);
    }

    // Read number of entries
    let mut count_buf = [0u8; 2];
    reader
        .read_exact(&mut count_buf)
        .map_err(|_| CqamError::InvalidBinary("Debug section truncated at count".to_string()))?;
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
        buf.extend_from_slice(&0u16.to_le_bytes());  // entry_point
        buf.extend_from_slice(&0u32.to_le_bytes());  // code_length
        let result = read_cqb(&mut &buf[..]);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_code() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&CQB_MAGIC);
        buf.extend_from_slice(&CQB_VERSION.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());  // entry_point
        buf.extend_from_slice(&5u32.to_le_bytes());  // claim 5 words...
        // ...but provide only 2
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        let result = read_cqb(&mut &buf[..]);
        assert!(result.is_err());
    }
}
