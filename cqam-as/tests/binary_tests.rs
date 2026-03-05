//! Integration tests for `.cqb` binary file I/O.

use std::io::Cursor;

use cqam_as::assembler::assemble_source;
use cqam_as::binary::{read_cqb, write_cqb, read_cqb_file, write_cqb_file};
use cqam_as::disassembler::disassemble;

// =============================================================================
// Round-trip tests: write -> read -> verify
// =============================================================================

#[test]
fn test_roundtrip_minimal_program() {
    let source = "ILDI R0, 42\nHALT\n";
    let result = assemble_source(source).unwrap();

    let mut buf = Vec::new();
    write_cqb(&mut buf, &result, false).unwrap();

    let image = read_cqb(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(image.version, 1);
    assert_eq!(image.entry_point, result.entry_point);
    assert_eq!(image.code, result.code);
    assert!(image.debug_symbols.is_none());
}

#[test]
fn test_roundtrip_with_debug_symbols() {
    let source = "\
LABEL: main
ILDI R0, 1
LABEL: loop
ISUB R0, R0, R0
JIF R0, loop
HALT
";
    let result = assemble_source(source).unwrap();
    assert!(!result.debug_symbols.is_empty());

    let mut buf = Vec::new();
    write_cqb(&mut buf, &result, true).unwrap();

    let image = read_cqb(&mut Cursor::new(&buf)).unwrap();
    let debug = image.debug_symbols.expect("debug symbols should be present");
    assert_eq!(debug.get(&0), Some(&"main".to_string()));
    assert_eq!(debug.get(&1), Some(&"loop".to_string()));
}

#[test]
fn test_roundtrip_no_debug_when_disabled() {
    let source = "LABEL: start\nHALT\n";
    let result = assemble_source(source).unwrap();

    let mut buf = Vec::new();
    write_cqb(&mut buf, &result, false).unwrap();

    let image = read_cqb(&mut Cursor::new(&buf)).unwrap();
    assert!(image.debug_symbols.is_none());
}

// =============================================================================
// Full pipeline: assemble -> write -> read -> disassemble -> reassemble
// =============================================================================

#[test]
fn test_full_pipeline_roundtrip() {
    let source = "\
ILDI R0, 10
ILDI R1, 20
IADD R2, R0, R1
ISTR R2, 100
HALT
";
    // Assemble
    let result1 = assemble_source(source).unwrap();

    // Write binary
    let mut buf = Vec::new();
    write_cqb(&mut buf, &result1, false).unwrap();

    // Read binary
    let image = read_cqb(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(image.code, result1.code);

    // Disassemble
    let text = disassemble(&image.code, image.debug_symbols.as_ref()).unwrap();

    // Reassemble from disassembled text
    let result2 = assemble_source(&text).unwrap();

    // Binary should be identical
    assert_eq!(result1.code, result2.code, "Round-trip binary mismatch");
}

// =============================================================================
// File I/O tests (using tempfile)
// =============================================================================

#[test]
fn test_file_roundtrip() {
    let source = "ILDI R0, 99\nHALT\n";
    let result = assemble_source(source).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.cqb");

    write_cqb_file(&path, &result, true).unwrap();
    let image = read_cqb_file(&path).unwrap();

    assert_eq!(image.code, result.code);
    assert_eq!(image.entry_point, result.entry_point);
}

// =============================================================================
// Error handling tests
// =============================================================================

#[test]
fn test_read_bad_magic() {
    let buf = b"BADM\x01\x00\x00\x00\x00\x00\x00\x00";
    let result = read_cqb(&mut Cursor::new(&buf[..]));
    assert!(result.is_err());
}

#[test]
fn test_read_truncated_header() {
    // Only 6 bytes, header needs 12
    let buf = b"CQAM\x01\x00";
    let result = read_cqb(&mut Cursor::new(&buf[..]));
    assert!(result.is_err());
}

#[test]
fn test_read_unsupported_version() {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"CQAM");
    buf.extend_from_slice(&42u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    let result = read_cqb(&mut Cursor::new(&buf));
    assert!(result.is_err());
}

#[test]
fn test_read_truncated_code_section() {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"CQAM");
    buf.extend_from_slice(&1u16.to_le_bytes());   // version
    buf.extend_from_slice(&0u16.to_le_bytes());   // entry_point
    buf.extend_from_slice(&100u32.to_le_bytes());  // claims 100 words
    // but provides zero
    let result = read_cqb(&mut Cursor::new(&buf));
    assert!(result.is_err());
}

#[test]
fn test_read_empty_program() {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"CQAM");
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // 0 words
    let image = read_cqb(&mut Cursor::new(&buf)).unwrap();
    assert!(image.code.is_empty());
    assert!(image.debug_symbols.is_none());
}

// =============================================================================
// Header field validation
// =============================================================================

#[test]
fn test_header_fields_correct() {
    let source = "\
LABEL: start
ILDI R0, 1
HALT
";
    let result = assemble_source(source).unwrap();
    assert_eq!(result.entry_point, 1); // first non-label at word 1

    let mut buf = Vec::new();
    write_cqb(&mut buf, &result, false).unwrap();

    // Verify header bytes directly
    assert_eq!(&buf[0..4], b"CQAM");
    assert_eq!(u16::from_le_bytes([buf[4], buf[5]]), 1);       // version
    assert_eq!(u16::from_le_bytes([buf[6], buf[7]]), 1);       // entry_point
    assert_eq!(
        u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
        3 // 3 instruction words (LABEL + ILDI + HALT)
    );
}

// =============================================================================
// Binary size validation
// =============================================================================

#[test]
fn test_binary_size_no_debug() {
    let result = assemble_source("HALT\n").unwrap();
    let mut buf = Vec::new();
    write_cqb(&mut buf, &result, false).unwrap();
    // Header (12) + 1 word (4) = 16 bytes
    assert_eq!(buf.len(), 16);
}

#[test]
fn test_binary_size_with_debug() {
    let source = "LABEL: x\nHALT\n";
    let result = assemble_source(source).unwrap();
    let mut buf = Vec::new();
    write_cqb(&mut buf, &result, true).unwrap();
    // Header (12) + 2 words (8) + debug_magic (4) + num_entries (2)
    //   + entry: id (2) + len (2) + "x" (1) = 12 + 8 + 4 + 2 + 5 = 31 bytes
    assert_eq!(buf.len(), 31);
}
