//! Parser tests for QPREPS, QPREPSM, and .qstate directive.

use cqam_core::instruction::{Instruction};
use cqam_core::parser::{parse_instruction, parse_program};

// =============================================================================
// QPREPS instruction parser tests
// =============================================================================

#[test]
fn test_parse_qpreps_basic() {
    let result = parse_instruction("QPREPS Q0, Z0, 3").unwrap();
    assert_eq!(result, Instruction::QPreps { dst: 0, z_start: 0, count: 3 });
}

#[test]
fn test_parse_qpreps_different_registers() {
    let result = parse_instruction("QPREPS Q1, Z2, 2").unwrap();
    assert_eq!(result, Instruction::QPreps { dst: 1, z_start: 2, count: 2 });
}

#[test]
fn test_parse_qpreps_count_one() {
    let result = parse_instruction("QPREPS Q0, Z0, 1").unwrap();
    assert_eq!(result, Instruction::QPreps { dst: 0, z_start: 0, count: 1 });
}

#[test]
fn test_parse_qpreps_count_4_at_z0() {
    // z_start=0, count=4: 0 + 2*4 = 8 = limit, should pass
    let result = parse_instruction("QPREPS Q0, Z0, 4").unwrap();
    assert_eq!(result, Instruction::QPreps { dst: 0, z_start: 0, count: 4 });
}

#[test]
fn test_parse_qpreps_z_overflow_error() {
    // z_start=2, count=4: 2 + 2*4 = 10 > 8, should error
    let result = parse_instruction("QPREPS Q0, Z2, 4");
    assert!(result.is_err(), "Should error when z_start + 2*count exceeds Z-file size");
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("exceeds Z-file size") || msg.contains("QPREPS"), "Error should mention QPREPS: {}", msg);
}

#[test]
fn test_parse_qpreps_wrong_operand_count() {
    assert!(parse_instruction("QPREPS Q0, Z0").is_err(), "QPREPS needs 3 operands");
    assert!(parse_instruction("QPREPS Q0").is_err(), "QPREPS needs 3 operands");
    assert!(parse_instruction("QPREPS").is_err(), "QPREPS needs 3 operands");
}

// =============================================================================
// QPREPSM instruction parser tests
// =============================================================================

#[test]
fn test_parse_qprepsm_basic() {
    let result = parse_instruction("QPREPSM Q0, R4, R5").unwrap();
    assert_eq!(result, Instruction::QPrepsm { dst: 0, r_base: 4, r_count: 5 });
}

#[test]
fn test_parse_qprepsm_different_registers() {
    let result = parse_instruction("QPREPSM Q2, R0, R1").unwrap();
    assert_eq!(result, Instruction::QPrepsm { dst: 2, r_base: 0, r_count: 1 });
}

#[test]
fn test_parse_qprepsm_wrong_operand_count() {
    assert!(parse_instruction("QPREPSM Q0, R4").is_err(), "QPREPSM needs 3 operands");
    assert!(parse_instruction("QPREPSM Q0").is_err(), "QPREPSM needs 3 operands");
    assert!(parse_instruction("QPREPSM").is_err(), "QPREPSM needs 3 operands");
}

// =============================================================================
// .qstate directive parser tests
// =============================================================================

/// Helper: parse a program with a .data section containing .qstate directives.
fn parse_with_qstate(qstate_lines: &str) -> Result<cqam_core::parser::ParsedProgram, cqam_core::error::CqamError> {
    let source = format!(
        ".data\namps:\n{}\n.code\nHALT\n",
        qstate_lines
    );
    parse_program(&source)
}

#[test]
fn test_qstate_plus_state() {
    // |+> = alpha=(1/sqrt(2), 0.0), beta=(1/sqrt(2), 0.0). Use full precision.
    let result = parse_with_qstate(".qstate 0.7071067811865476, 0.0, 0.7071067811865476, 0.0").unwrap();
    let cells = &result.data_section.cells;
    assert_eq!(cells.len(), 4, "One .qstate should produce 4 cells, got {}", cells.len());

    // Decode the f64 values back
    let re_a = f64::from_bits(cells[0] as u64);
    let im_a = f64::from_bits(cells[1] as u64);
    let re_b = f64::from_bits(cells[2] as u64);
    let im_b = f64::from_bits(cells[3] as u64);

    assert!((re_a - 0.707106781).abs() < 1e-9, "re_a wrong: {}", re_a);
    assert!((im_a).abs() < 1e-15, "im_a should be 0");
    assert!((re_b - 0.707106781).abs() < 1e-9, "re_b wrong: {}", re_b);
    assert!((im_b).abs() < 1e-15, "im_b should be 0");
}

#[test]
fn test_qstate_zero_state() {
    let result = parse_with_qstate(".qstate 1.0, 0.0, 0.0, 0.0").unwrap();
    let cells = &result.data_section.cells;
    assert_eq!(cells.len(), 4);

    let re_a = f64::from_bits(cells[0] as u64);
    let im_a = f64::from_bits(cells[1] as u64);
    let re_b = f64::from_bits(cells[2] as u64);
    let im_b = f64::from_bits(cells[3] as u64);

    assert!((re_a - 1.0).abs() < 1e-15);
    assert!(im_a.abs() < 1e-15);
    assert!(re_b.abs() < 1e-15);
    assert!(im_b.abs() < 1e-15);
}

#[test]
fn test_qstate_one_state() {
    let result = parse_with_qstate(".qstate 0.0, 0.0, 1.0, 0.0").unwrap();
    let cells = &result.data_section.cells;
    assert_eq!(cells.len(), 4);

    let re_a = f64::from_bits(cells[0] as u64);
    let re_b = f64::from_bits(cells[2] as u64);

    assert!(re_a.abs() < 1e-15);
    assert!((re_b - 1.0).abs() < 1e-15);
}

#[test]
fn test_qstate_multiple_entries() {
    // Three .qstate lines → 12 CMEM cells (use full-precision 1/sqrt(2))
    let source = ".qstate 0.7071067811865476, 0.0, 0.7071067811865476, 0.0\n\
                  .qstate 1.0, 0.0, 0.0, 0.0\n\
                  .qstate 0.0, 0.0, 1.0, 0.0";
    let result = parse_with_qstate(source).unwrap();
    assert_eq!(result.data_section.cells.len(), 12, "3 .qstate directives should produce 12 cells");
}

#[test]
fn test_qstate_unnormalized_error() {
    // norm_sq = 1.5 ≠ 1.0 → assembler error
    let result = parse_with_qstate(".qstate 1.0, 0.0, 0.7071, 0.0");
    assert!(result.is_err(), "Unnormalized .qstate should produce an error");
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("normalized") || msg.contains("qstate"), "Error should mention normalization: {}", msg);
}

#[test]
fn test_qstate_wrong_token_count_error() {
    // Only 3 values instead of 4
    let result = parse_with_qstate(".qstate 1.0, 0.0, 0.0");
    assert!(result.is_err(), "Wrong number of .qstate values should produce an error");
}

#[test]
fn test_qstate_invalid_float_error() {
    let result = parse_with_qstate(".qstate 1.0, 0.0, abc, 0.0");
    assert!(result.is_err(), "Non-float value in .qstate should produce an error");
}

// =============================================================================
// Full program parse tests
// =============================================================================

/// Parse the reference program from the spec (section 10).
/// Verifies that the full program parses without errors.
#[test]
fn test_parse_reference_program() {
    let source = r#"#! qubits 3
.data
    .org 500
amps:
    .qstate 0.7071067811865476, 0.0, 0.7071067811865476, 0.0
    .qstate 1.0, 0.0, 0.0, 0.0
    .qstate 0.0, 0.0, 1.0, 0.0

.code
    QPREP Q0, ZERO
    ILDI R0, 500
    ILDI R1, 3
    QPREPSM Q0, R0, R1
    HALT
"#;
    let result = parse_program(source);
    assert!(result.is_ok(), "Reference program should parse successfully: {:?}", result.err());

    let parsed = result.unwrap();
    // 12 data cells: 3 * 4 cells each, starting at .org 500
    assert_eq!(parsed.data_section.cells.len(), 500 + 12,
        "Data section should have 500 + 12 cells");

    // Verify QPREPSM is in the program
    let has_qprepsm = parsed.instructions.iter().any(|i| matches!(i, Instruction::QPrepsm { .. }));
    assert!(has_qprepsm, "Program should contain QPREPSM instruction");
}

/// Verify QPREPS round-trips through encode/decode.
#[test]
fn test_qpreps_encode_decode_roundtrip() {
    use cqam_core::opcode::{encode, decode};
    use std::collections::HashMap;

    let instr = Instruction::QPreps { dst: 1, z_start: 2, count: 2 };
    let label_map = HashMap::new();
    let word = encode(&instr, &label_map).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

/// Verify QPREPSM round-trips through encode/decode.
#[test]
fn test_qprepsm_encode_decode_roundtrip() {
    use cqam_core::opcode::{encode, decode};
    use std::collections::HashMap;

    let instr = Instruction::QPrepsm { dst: 0, r_base: 4, r_count: 5 };
    let label_map = HashMap::new();
    let word = encode(&instr, &label_map).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}
