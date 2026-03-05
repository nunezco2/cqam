//! Binary round-trip tests: source text -> assemble -> write `.cqb` ->
//! read `.cqb` -> disassemble -> re-parse -> verify structural equivalence.

use std::io::Cursor;

use cqam_as::assembler::assemble_source;
use cqam_as::binary::{write_cqb, read_cqb};
use cqam_as::disassembler::disassemble;
use cqam_core::parser::parse_program;
use cqam_core::instruction::Instruction;

/// Run the full assemble -> write -> read -> disassemble -> re-parse pipeline.
fn roundtrip_parse(source: &str) -> Vec<Instruction> {
    let result = assemble_source(source).unwrap();
    let mut buf = Vec::new();
    write_cqb(&mut buf, &result, true).unwrap();
    let image = read_cqb(&mut Cursor::new(&buf)).unwrap();
    let text = disassemble(&image.code, image.debug_symbols.as_ref()).unwrap();
    parse_program(&text).unwrap()
}

#[test]
fn test_binary_roundtrip_arithmetic() {
    let source = "ILDI R0, 42\nILDI R1, 8\nIADD R2, R0, R1\nHALT\n";
    let original = parse_program(source).unwrap();
    let roundtripped = roundtrip_parse(source);

    assert_eq!(roundtripped.len(), original.len(),
        "Instruction count mismatch: expected {}, got {}", original.len(), roundtripped.len());

    // Verify operand-level equivalence
    assert!(matches!(roundtripped[0], Instruction::ILdi { dst: 0, imm: 42 }));
    assert!(matches!(roundtripped[1], Instruction::ILdi { dst: 1, imm: 8 }));
    assert!(matches!(roundtripped[2], Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 }));
    assert!(matches!(roundtripped[3], Instruction::Halt));
}

#[test]
fn test_binary_roundtrip_quantum() {
    let source = "\
QPREP Q0, 0
QKERNEL Q1, Q0, 1, R0, R1
QOBSERVE H0, Q1
HALT
";
    let original = parse_program(source).unwrap();
    let roundtripped = roundtrip_parse(source);
    assert_eq!(roundtripped.len(), original.len());

    assert!(matches!(roundtripped[0], Instruction::QPrep { dst: 0, dist: 0 }));
    assert!(matches!(roundtripped[1], Instruction::QKernel { dst: 1, src: 0, kernel: 1, ctx0: 0, ctx1: 1 }));
    assert!(matches!(roundtripped[2], Instruction::QObserve { dst_h: 0, src_q: 1 }));
    assert!(matches!(roundtripped[3], Instruction::Halt));
}

#[test]
fn test_binary_roundtrip_control_flow() {
    let source = "\
LABEL: START
ILDI R0, 0
JIF R0, START
CALL START
RET
HALT
";
    let original = parse_program(source).unwrap();
    let roundtripped = roundtrip_parse(source);
    assert_eq!(roundtripped.len(), original.len());

    // Labels are restored via debug symbols
    assert!(matches!(&roundtripped[0], Instruction::Label(name) if name == "START"));
    // Jump/call targets are formatted as @addr after decode (not restored to label names)
    // The label at index 0 maps to address 0, so targets become "@0"
    assert!(matches!(&roundtripped[2], Instruction::Jif { pred: 0, .. }));
    assert!(matches!(&roundtripped[3], Instruction::Call { .. }));
    assert!(matches!(&roundtripped[4], Instruction::Ret));
    assert!(matches!(&roundtripped[5], Instruction::Halt));
}

#[test]
fn test_binary_cqb_header_valid() {
    let source = "ILDI R0, 99\nHALT\n";
    let result = assemble_source(source).unwrap();
    let mut buf = Vec::new();
    write_cqb(&mut buf, &result, false).unwrap();
    let image = read_cqb(&mut Cursor::new(&buf)).unwrap();

    assert_eq!(image.version, 1);
    assert!((image.entry_point as usize) < image.code.len() || image.code.is_empty());
}
