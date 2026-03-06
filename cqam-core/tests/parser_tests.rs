//! Tests for the CQAM assembly text parser.
//!
//! Covers successful parsing of every instruction variant, comment stripping,
//! register range validation, error propagation, and `parse_program`.

use cqam_core::instruction::*;
use cqam_core::parser::{parse_instruction, parse_program};

// ===========================================================================
// Blank lines and comments
// ===========================================================================

#[test]
fn test_parse_nop_blank_line() {
    assert_eq!(parse_instruction("").unwrap(), Instruction::Nop);
    assert_eq!(parse_instruction("   ").unwrap(), Instruction::Nop);
    assert_eq!(parse_instruction("\t").unwrap(), Instruction::Nop);
}

#[test]
fn test_parse_comment_hash() {
    assert_eq!(parse_instruction("# comment").unwrap(), Instruction::Nop);
    assert_eq!(parse_instruction("  # indented").unwrap(), Instruction::Nop);
}

#[test]
fn test_parse_comment_double_slash() {
    assert_eq!(parse_instruction("// comment").unwrap(), Instruction::Nop);
}

#[test]
fn test_parse_inline_comment() {
    assert_eq!(
        parse_instruction("IADD R3, R1, R2 # sum").unwrap(),
        Instruction::IAdd { dst: 3, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_inline_comment_double_slash() {
    assert_eq!(
        parse_instruction("ILDI R0, 42 // load immediate").unwrap(),
        Instruction::ILdi { dst: 0, imm: 42 }
    );
}

// ===========================================================================
// NOP
// ===========================================================================

#[test]
fn test_parse_nop_explicit() {
    assert_eq!(parse_instruction("NOP").unwrap(), Instruction::Nop);
}

// ===========================================================================
// Integer arithmetic
// ===========================================================================

#[test]
fn test_parse_iadd() {
    assert_eq!(
        parse_instruction("IADD R3, R1, R2").unwrap(),
        Instruction::IAdd { dst: 3, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_isub() {
    assert_eq!(
        parse_instruction("ISUB R4, R3, R1").unwrap(),
        Instruction::ISub { dst: 4, lhs: 3, rhs: 1 }
    );
}

#[test]
fn test_parse_imul() {
    assert_eq!(
        parse_instruction("IMUL R5, R0, R1").unwrap(),
        Instruction::IMul { dst: 5, lhs: 0, rhs: 1 }
    );
}

#[test]
fn test_parse_idiv() {
    assert_eq!(
        parse_instruction("IDIV R6, R2, R3").unwrap(),
        Instruction::IDiv { dst: 6, lhs: 2, rhs: 3 }
    );
}

#[test]
fn test_parse_imod() {
    assert_eq!(
        parse_instruction("IMOD R7, R5, R1").unwrap(),
        Instruction::IMod { dst: 7, lhs: 5, rhs: 1 }
    );
}

// ===========================================================================
// Integer bitwise
// ===========================================================================

#[test]
fn test_parse_iand() {
    assert_eq!(
        parse_instruction("IAND R0, R1, R2").unwrap(),
        Instruction::IAnd { dst: 0, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_ior() {
    assert_eq!(
        parse_instruction("IOR R3, R4, R5").unwrap(),
        Instruction::IOr { dst: 3, lhs: 4, rhs: 5 }
    );
}

#[test]
fn test_parse_ixor() {
    assert_eq!(
        parse_instruction("IXOR R6, R7, R8").unwrap(),
        Instruction::IXor { dst: 6, lhs: 7, rhs: 8 }
    );
}

#[test]
fn test_parse_inot() {
    assert_eq!(
        parse_instruction("INOT R3, R1").unwrap(),
        Instruction::INot { dst: 3, src: 1 }
    );
}

#[test]
fn test_parse_ishl() {
    assert_eq!(
        parse_instruction("ISHL R4, R1, 3").unwrap(),
        Instruction::IShl { dst: 4, src: 1, amt: 3 }
    );
}

#[test]
fn test_parse_ishr() {
    assert_eq!(
        parse_instruction("ISHR R5, R2, 8").unwrap(),
        Instruction::IShr { dst: 5, src: 2, amt: 8 }
    );
}

// ===========================================================================
// Integer memory
// ===========================================================================

#[test]
fn test_parse_ildi() {
    assert_eq!(
        parse_instruction("ILDI R0, 42").unwrap(),
        Instruction::ILdi { dst: 0, imm: 42 }
    );
}

#[test]
fn test_parse_ildi_negative() {
    assert_eq!(
        parse_instruction("ILDI R1, -100").unwrap(),
        Instruction::ILdi { dst: 1, imm: -100 }
    );
}

#[test]
fn test_parse_ildi_zero() {
    assert_eq!(
        parse_instruction("ILDI R0, 0").unwrap(),
        Instruction::ILdi { dst: 0, imm: 0 }
    );
}

#[test]
fn test_parse_ildm() {
    assert_eq!(
        parse_instruction("ILDM R5, 1000").unwrap(),
        Instruction::ILdm { dst: 5, addr: 1000 }
    );
}

#[test]
fn test_parse_ildm_hex() {
    assert_eq!(
        parse_instruction("ILDM R5, 0x1000").unwrap(),
        Instruction::ILdm { dst: 5, addr: 4096 }
    );
}

#[test]
fn test_parse_istr() {
    assert_eq!(
        parse_instruction("ISTR R1, 100").unwrap(),
        Instruction::IStr { src: 1, addr: 100 }
    );
}

// ===========================================================================
// Integer comparison
// ===========================================================================

#[test]
fn test_parse_ieq() {
    assert_eq!(
        parse_instruction("IEQ R0, R1, R2").unwrap(),
        Instruction::IEq { dst: 0, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_ilt() {
    assert_eq!(
        parse_instruction("ILT R3, R4, R5").unwrap(),
        Instruction::ILt { dst: 3, lhs: 4, rhs: 5 }
    );
}

#[test]
fn test_parse_igt() {
    assert_eq!(
        parse_instruction("IGT R6, R7, R8").unwrap(),
        Instruction::IGt { dst: 6, lhs: 7, rhs: 8 }
    );
}

// ===========================================================================
// Float arithmetic
// ===========================================================================

#[test]
fn test_parse_fadd() {
    assert_eq!(
        parse_instruction("FADD F2, F0, F1").unwrap(),
        Instruction::FAdd { dst: 2, lhs: 0, rhs: 1 }
    );
}

#[test]
fn test_parse_fsub() {
    assert_eq!(
        parse_instruction("FSUB F3, F1, F2").unwrap(),
        Instruction::FSub { dst: 3, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_fmul() {
    assert_eq!(
        parse_instruction("FMUL F4, F0, F1").unwrap(),
        Instruction::FMul { dst: 4, lhs: 0, rhs: 1 }
    );
}

#[test]
fn test_parse_fdiv() {
    assert_eq!(
        parse_instruction("FDIV F5, F2, F3").unwrap(),
        Instruction::FDiv { dst: 5, lhs: 2, rhs: 3 }
    );
}

#[test]
fn test_parse_fldi() {
    assert_eq!(
        parse_instruction("FLDI F0, 314").unwrap(),
        Instruction::FLdi { dst: 0, imm: 314 }
    );
}

#[test]
fn test_parse_fldm() {
    assert_eq!(
        parse_instruction("FLDM F1, 200").unwrap(),
        Instruction::FLdm { dst: 1, addr: 200 }
    );
}

#[test]
fn test_parse_fstr() {
    assert_eq!(
        parse_instruction("FSTR F2, 300").unwrap(),
        Instruction::FStr { src: 2, addr: 300 }
    );
}

#[test]
fn test_parse_feq() {
    assert_eq!(
        parse_instruction("FEQ R0, F1, F2").unwrap(),
        Instruction::FEq { dst: 0, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_flt() {
    assert_eq!(
        parse_instruction("FLT R1, F3, F4").unwrap(),
        Instruction::FLt { dst: 1, lhs: 3, rhs: 4 }
    );
}

#[test]
fn test_parse_fgt() {
    assert_eq!(
        parse_instruction("FGT R2, F5, F6").unwrap(),
        Instruction::FGt { dst: 2, lhs: 5, rhs: 6 }
    );
}

// ===========================================================================
// Complex
// ===========================================================================

#[test]
fn test_parse_zadd() {
    assert_eq!(
        parse_instruction("ZADD Z1, Z0, Z2").unwrap(),
        Instruction::ZAdd { dst: 1, lhs: 0, rhs: 2 }
    );
}

#[test]
fn test_parse_zsub() {
    assert_eq!(
        parse_instruction("ZSUB Z3, Z1, Z2").unwrap(),
        Instruction::ZSub { dst: 3, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_zmul() {
    assert_eq!(
        parse_instruction("ZMUL Z4, Z0, Z1").unwrap(),
        Instruction::ZMul { dst: 4, lhs: 0, rhs: 1 }
    );
}

#[test]
fn test_parse_zdiv() {
    assert_eq!(
        parse_instruction("ZDIV Z5, Z2, Z3").unwrap(),
        Instruction::ZDiv { dst: 5, lhs: 2, rhs: 3 }
    );
}

#[test]
fn test_parse_zldi() {
    assert_eq!(
        parse_instruction("ZLDI Z0, 1, -1").unwrap(),
        Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: -1 }
    );
}

#[test]
fn test_parse_zldm() {
    assert_eq!(
        parse_instruction("ZLDM Z1, 400").unwrap(),
        Instruction::ZLdm { dst: 1, addr: 400 }
    );
}

#[test]
fn test_parse_zstr() {
    assert_eq!(
        parse_instruction("ZSTR Z2, 500").unwrap(),
        Instruction::ZStr { src: 2, addr: 500 }
    );
}

// ===========================================================================
// Register-indirect memory
// ===========================================================================

#[test]
fn test_parse_ildx() {
    assert_eq!(
        parse_instruction("ILDX R3, R5").unwrap(),
        Instruction::ILdx { dst: 3, addr_reg: 5 }
    );
}

#[test]
fn test_parse_istrx() {
    assert_eq!(
        parse_instruction("ISTRX R0, R1").unwrap(),
        Instruction::IStrx { src: 0, addr_reg: 1 }
    );
}

#[test]
fn test_parse_fldx() {
    assert_eq!(
        parse_instruction("FLDX F2, R4").unwrap(),
        Instruction::FLdx { dst: 2, addr_reg: 4 }
    );
}

#[test]
fn test_parse_fstrx() {
    assert_eq!(
        parse_instruction("FSTRX F7, R3").unwrap(),
        Instruction::FStrx { src: 7, addr_reg: 3 }
    );
}

#[test]
fn test_parse_zldx() {
    assert_eq!(
        parse_instruction("ZLDX Z1, R6").unwrap(),
        Instruction::ZLdx { dst: 1, addr_reg: 6 }
    );
}

#[test]
fn test_parse_zstrx() {
    assert_eq!(
        parse_instruction("ZSTRX Z5, R2").unwrap(),
        Instruction::ZStrx { src: 5, addr_reg: 2 }
    );
}

#[test]
fn test_parse_ildx_missing_operand() {
    assert!(parse_instruction("ILDX R3").is_err());
}

#[test]
fn test_parse_istrx_too_many_operands() {
    assert!(parse_instruction("ISTRX R0, R1, R2").is_err());
}

#[test]
fn test_parse_fldx_invalid_register() {
    assert!(parse_instruction("FLDX F2, 100").is_err());
}

#[test]
fn test_parse_zstrx_max_regs() {
    assert_eq!(
        parse_instruction("ZSTRX Z15, R15").unwrap(),
        Instruction::ZStrx { src: 15, addr_reg: 15 }
    );
}

#[test]
fn test_parse_indirect_in_program() {
    let source = "\
ILDI R1, 100
ILDI R0, 42
ISTRX R0, R1
ILDX R2, R1
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 5);
    assert_eq!(program[2], Instruction::IStrx { src: 0, addr_reg: 1 });
    assert_eq!(program[3], Instruction::ILdx { dst: 2, addr_reg: 1 });
}

// ===========================================================================
// Type conversion
// ===========================================================================

#[test]
fn test_parse_cvtif() {
    assert_eq!(
        parse_instruction("CVTIF F0, R1").unwrap(),
        Instruction::CvtIF { dst_f: 0, src_i: 1 }
    );
}

#[test]
fn test_parse_cvtfi() {
    assert_eq!(
        parse_instruction("CVTFI R0, F1").unwrap(),
        Instruction::CvtFI { dst_i: 0, src_f: 1 }
    );
}

#[test]
fn test_parse_cvtfz() {
    assert_eq!(
        parse_instruction("CVTFZ Z0, F1").unwrap(),
        Instruction::CvtFZ { dst_z: 0, src_f: 1 }
    );
}

#[test]
fn test_parse_cvtzf() {
    assert_eq!(
        parse_instruction("CVTZF F0, Z1").unwrap(),
        Instruction::CvtZF { dst_f: 0, src_z: 1 }
    );
}

// ===========================================================================
// Control flow
// ===========================================================================

#[test]
fn test_parse_jmp() {
    assert_eq!(
        parse_instruction("JMP LOOP").unwrap(),
        Instruction::Jmp { target: "LOOP".into() }
    );
}

#[test]
fn test_parse_jif() {
    assert_eq!(
        parse_instruction("JIF R0, THEN").unwrap(),
        Instruction::Jif { pred: 0, target: "THEN".into() }
    );
}

#[test]
fn test_parse_call() {
    assert_eq!(
        parse_instruction("CALL FUNC").unwrap(),
        Instruction::Call { target: "FUNC".into() }
    );
}

#[test]
fn test_parse_ret() {
    assert_eq!(parse_instruction("RET").unwrap(), Instruction::Ret);
}

#[test]
fn test_parse_halt() {
    assert_eq!(parse_instruction("HALT").unwrap(), Instruction::Halt);
}

#[test]
fn test_parse_label() {
    assert_eq!(
        parse_instruction("LABEL: LOOP").unwrap(),
        Instruction::Label("LOOP".into())
    );
}

// ===========================================================================
// Quantum
// ===========================================================================

#[test]
fn test_parse_qprep() {
    assert_eq!(
        parse_instruction("QPREP Q0, 0").unwrap(),
        Instruction::QPrep { dst: 0, dist: 0 }
    );
}

#[test]
fn test_parse_qprep_bell() {
    assert_eq!(
        parse_instruction("QPREP Q1, 2").unwrap(),
        Instruction::QPrep { dst: 1, dist: 2 }
    );
}

#[test]
fn test_parse_qkernel() {
    assert_eq!(
        parse_instruction("QKERNEL Q1, Q0, 1, R2, R3").unwrap(),
        Instruction::QKernel { dst: 1, src: 0, kernel: 1, ctx0: 2, ctx1: 3 }
    );
}

#[test]
fn test_parse_qobserve() {
    assert_eq!(
        parse_instruction("QOBSERVE H0, Q1").unwrap(),
        Instruction::QObserve { dst_h: 0, src_q: 1, mode: 0, ctx0: 0, ctx1: 0 }
    );
}

#[test]
fn test_parse_qsample() {
    assert_eq!(
        parse_instruction("QSAMPLE H0, Q1").unwrap(),
        Instruction::QSample { dst_h: 0, src_q: 1, mode: 0, ctx0: 0, ctx1: 0 }
    );
}

#[test]
fn test_parse_qsample_max_regs() {
    assert_eq!(
        parse_instruction("QSAMPLE H7, Q7").unwrap(),
        Instruction::QSample { dst_h: 7, src_q: 7, mode: 0, ctx0: 0, ctx1: 0 }
    );
}

#[test]
fn test_parse_qsample_missing_operand() {
    assert!(parse_instruction("QSAMPLE H0").is_err());
}

#[test]
fn test_parse_qsample_too_many_operands() {
    assert!(parse_instruction("QSAMPLE H0, Q1, Q2").is_err());
}

#[test]
fn test_parse_qload() {
    assert_eq!(
        parse_instruction("QLOAD Q2, 10").unwrap(),
        Instruction::QLoad { dst_q: 2, addr: 10 }
    );
}

#[test]
fn test_parse_qstore() {
    assert_eq!(
        parse_instruction("QSTORE Q2, 10").unwrap(),
        Instruction::QStore { src_q: 2, addr: 10 }
    );
}

// ===========================================================================
// Hybrid
// ===========================================================================

#[test]
fn test_parse_hfork() {
    assert_eq!(parse_instruction("HFORK").unwrap(), Instruction::HFork);
}

#[test]
fn test_parse_hmerge() {
    assert_eq!(parse_instruction("HMERGE").unwrap(), Instruction::HMerge);
}

#[test]
fn test_parse_hcexec() {
    assert_eq!(
        parse_instruction("HCEXEC 4, THEN").unwrap(),
        Instruction::HCExec { flag: 4, target: "THEN".into() }
    );
}

#[test]
fn test_parse_hreduce() {
    assert_eq!(
        parse_instruction("HREDUCE H0, R1, 0").unwrap(),
        Instruction::HReduce { src: 0, dst: 1, func: 0 }
    );
}

// ===========================================================================
// Register range tests
// ===========================================================================

#[test]
fn test_parse_max_int_register() {
    assert_eq!(
        parse_instruction("IADD R15, R14, R13").unwrap(),
        Instruction::IAdd { dst: 15, lhs: 14, rhs: 13 }
    );
}

#[test]
fn test_parse_max_float_register() {
    assert_eq!(
        parse_instruction("FADD F15, F14, F13").unwrap(),
        Instruction::FAdd { dst: 15, lhs: 14, rhs: 13 }
    );
}

#[test]
fn test_parse_max_complex_register() {
    assert_eq!(
        parse_instruction("ZADD Z15, Z14, Z13").unwrap(),
        Instruction::ZAdd { dst: 15, lhs: 14, rhs: 13 }
    );
}

#[test]
fn test_parse_max_quantum_register() {
    assert_eq!(
        parse_instruction("QPREP Q7, 0").unwrap(),
        Instruction::QPrep { dst: 7, dist: 0 }
    );
}

#[test]
fn test_parse_max_hybrid_register() {
    assert_eq!(
        parse_instruction("QOBSERVE H7, Q7").unwrap(),
        Instruction::QObserve { dst_h: 7, src_q: 7, mode: 0, ctx0: 0, ctx1: 0 }
    );
}

// --- Error cases -------------------------------------------------------------

#[test]
fn test_parse_unknown_returns_error() {
    assert!(parse_instruction("FOOBAR x, y").is_err());
    assert!(parse_instruction("UNKNOWN").is_err());
}

#[test]
fn test_parse_missing_operands_returns_error() {
    assert!(parse_instruction("IADD R3, R1").is_err());
    assert!(parse_instruction("ILDI R0").is_err());
    assert!(parse_instruction("JMP").is_err());
    assert!(parse_instruction("LABEL:").is_err());
}

#[test]
fn test_parse_whitespace_tolerance() {
    assert_eq!(
        parse_instruction("  IADD   R3 ,  R1  ,  R2  ").unwrap(),
        Instruction::IAdd { dst: 3, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_out_of_range_register_returns_error() {
    // R16 is out of range (only R0-R15)
    assert!(parse_instruction("IADD R16, R0, R1").is_err());
    // Q8 is out of range (only Q0-Q7)
    assert!(parse_instruction("QPREP Q8, 0").is_err());
}

// ===========================================================================
// parse_program integration
// ===========================================================================

#[test]
fn test_parse_program_multiline() {
    let source = "\
# This is a comment
ILDI R1, 42
ILDI R2, 8

// Another comment
IADD R3, R1, R2
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 4);
    assert_eq!(program[0], Instruction::ILdi { dst: 1, imm: 42 });
    assert_eq!(program[1], Instruction::ILdi { dst: 2, imm: 8 });
    assert_eq!(program[2], Instruction::IAdd { dst: 3, lhs: 1, rhs: 2 });
    assert_eq!(program[3], Instruction::Halt);
}

#[test]
fn test_parse_program_filters_nops() {
    let source = "\
# comment

// another comment
ILDI R1, 10
HALT
";
    let program = parse_program(source).unwrap();
    for instr in &program {
        assert!(!matches!(instr, Instruction::Nop));
    }
    assert_eq!(program.len(), 2);
}

#[test]
fn test_parse_program_propagates_error() {
    let source = "\
ILDI R1, 10
FOOBAR x, y
HALT
";
    assert!(parse_program(source).is_err());
}

#[test]
fn test_parse_program_with_labels() {
    let source = "\
LABEL: START
ILDI R0, 1
ILDI R1, 10
LABEL: LOOP
IADD R0, R0, R1
JMP LOOP
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 6);
    assert_eq!(program[0], Instruction::Label("START".into()));
    assert_eq!(program[1], Instruction::ILdi { dst: 0, imm: 1 });
    assert_eq!(program[2], Instruction::ILdi { dst: 1, imm: 10 });
    assert_eq!(program[3], Instruction::Label("LOOP".into()));
    assert_eq!(program[4], Instruction::IAdd { dst: 0, lhs: 0, rhs: 1 });
    assert_eq!(program[5], Instruction::Jmp { target: "LOOP".into() });
}

#[test]
fn test_parse_program_quantum_workflow() {
    let source = "\
QPREP Q0, 0
QKERNEL Q1, Q0, 1, R0, R1
QOBSERVE H0, Q1
HREDUCE H0, R2, 12
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 5);
    assert_eq!(program[0], Instruction::QPrep { dst: 0, dist: 0 });
    assert_eq!(program[1], Instruction::QKernel { dst: 1, src: 0, kernel: 1, ctx0: 0, ctx1: 1 });
    assert_eq!(program[2], Instruction::QObserve { dst_h: 0, src_q: 1, mode: 0, ctx0: 0, ctx1: 0 });
    assert_eq!(program[3], Instruction::HReduce { src: 0, dst: 2, func: 12 });
    assert_eq!(program[4], Instruction::Halt);
}

#[test]
fn test_parse_program_empty_source() {
    let program = parse_program("").unwrap();
    assert!(program.is_empty());
}

#[test]
fn test_parse_program_only_comments() {
    let source = "\
# comment 1
// comment 2
# comment 3
";
    let program = parse_program(source).unwrap();
    assert!(program.is_empty());
}

// --- Additional error case tests ---

#[test]
fn test_parse_error_contains_line_info() {
    let source = "ILDI R0, 42\nFOOBAR\nHALT";
    let err = parse_program(source).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("line 2"), "Error message should contain line number: {}", msg);
}

#[test]
fn test_parse_error_missing_operands_message() {
    let err = parse_instruction("IADD R0").unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("IADD"), "Error should mention instruction: {}", msg);
}

#[test]
fn test_parse_error_invalid_register() {
    let err = parse_instruction("IADD R0, R1, X2").unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("invalid register"), "Error should mention invalid register: {}", msg);
}

// --- Edge cases --------------------------------------------------------------

#[test]
fn test_parse_ildi_hex_immediate() {
    assert_eq!(
        parse_instruction("ILDI R0, 0xFF").unwrap(),
        Instruction::ILdi { dst: 0, imm: 255 }
    );
}

#[test]
fn test_parse_ildi_min_negative() {
    assert_eq!(
        parse_instruction("ILDI R0, -32768").unwrap(),
        Instruction::ILdi { dst: 0, imm: -32768 }
    );
}

#[test]
fn test_parse_ildi_max_positive() {
    assert_eq!(
        parse_instruction("ILDI R0, 32767").unwrap(),
        Instruction::ILdi { dst: 0, imm: 32767 }
    );
}

#[test]
fn test_parse_lowercase_opcode_rejected() {
    assert!(parse_instruction("iadd R0, R1, R2").is_err());
}

#[test]
fn test_parse_mixed_case_opcode_rejected() {
    assert!(parse_instruction("Iadd R0, R1, R2").is_err());
}

#[test]
fn test_parse_trailing_whitespace_line() {
    assert_eq!(parse_instruction("   \t  ").unwrap(), Instruction::Nop);
}

#[test]
fn test_parse_program_multiple_empty_lines() {
    let source = "\
ILDI R0, 1



ILDI R1, 2
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 3);
    assert_eq!(program[0], Instruction::ILdi { dst: 0, imm: 1 });
    assert_eq!(program[1], Instruction::ILdi { dst: 1, imm: 2 });
    assert_eq!(program[2], Instruction::Halt);
}

// --- Additional edge cases ---------------------------------------------------

#[test]
fn test_parse_ildi_out_of_range_positive() {
    // 32768 exceeds i16::MAX (32767), should fail
    assert!(parse_instruction("ILDI R0, 32768").is_err());
}

#[test]
fn test_parse_ildi_out_of_range_negative() {
    // -32769 is below i16::MIN (-32768), should fail
    assert!(parse_instruction("ILDI R0, -32769").is_err());
}

#[test]
fn test_parse_ildi_hex_out_of_range() {
    // 0x8000 = 32768, exceeds i16::MAX
    assert!(parse_instruction("ILDI R0, 0x8000").is_err());
}

#[test]
fn test_parse_ildi_hex_max_valid() {
    // 0x7FFF = 32767, should succeed
    assert_eq!(
        parse_instruction("ILDI R0, 0x7FFF").unwrap(),
        Instruction::ILdi { dst: 0, imm: 32767 }
    );
}

#[test]
fn test_parse_fldi_negative() {
    // FLDI uses i16 immediate just like ILDI
    assert_eq!(
        parse_instruction("FLDI F0, -100").unwrap(),
        Instruction::FLdi { dst: 0, imm: -100 }
    );
}

#[test]
fn test_parse_label_with_underscore() {
    assert_eq!(
        parse_instruction("LABEL: MY_LABEL_123").unwrap(),
        Instruction::Label("MY_LABEL_123".into())
    );
}

#[test]
fn test_parse_jmp_to_label_with_digits() {
    assert_eq!(
        parse_instruction("JMP LOOP_42").unwrap(),
        Instruction::Jmp { target: "LOOP_42".into() }
    );
}

// ===========================================================================
// QKERNELF / QKERNELZ parse tests
// ===========================================================================

#[test]
fn test_parse_qkernelf() {
    assert_eq!(
        parse_instruction("QKERNELF Q1, Q0, 5, F3, F4").unwrap(),
        Instruction::QKernelF { dst: 1, src: 0, kernel: 5, fctx0: 3, fctx1: 4 }
    );
}

#[test]
fn test_parse_qkernelf_max_regs() {
    assert_eq!(
        parse_instruction("QKERNELF Q7, Q7, 31, F15, F15").unwrap(),
        Instruction::QKernelF { dst: 7, src: 7, kernel: 31, fctx0: 15, fctx1: 15 }
    );
}

#[test]
fn test_parse_qkernelf_zero_values() {
    assert_eq!(
        parse_instruction("QKERNELF Q0, Q0, 0, F0, F0").unwrap(),
        Instruction::QKernelF { dst: 0, src: 0, kernel: 0, fctx0: 0, fctx1: 0 }
    );
}

#[test]
fn test_parse_qkernelf_missing_operand() {
    assert!(parse_instruction("QKERNELF Q1, Q0, 5, F3").is_err());
}

#[test]
fn test_parse_qkernelf_too_many_operands() {
    assert!(parse_instruction("QKERNELF Q1, Q0, 5, F3, F4, F5").is_err());
}

#[test]
fn test_parse_qkernelf_invalid_register() {
    // "99" is not a valid register token (no R/F/Z/Q/H prefix)
    assert!(parse_instruction("QKERNELF Q1, Q0, 5, 99, F4").is_err());
}

#[test]
fn test_parse_qkernelz() {
    assert_eq!(
        parse_instruction("QKERNELZ Q1, Q0, 6, Z2, Z3").unwrap(),
        Instruction::QKernelZ { dst: 1, src: 0, kernel: 6, zctx0: 2, zctx1: 3 }
    );
}

#[test]
fn test_parse_qkernelz_max_regs() {
    assert_eq!(
        parse_instruction("QKERNELZ Q7, Q7, 31, Z15, Z15").unwrap(),
        Instruction::QKernelZ { dst: 7, src: 7, kernel: 31, zctx0: 15, zctx1: 15 }
    );
}

#[test]
fn test_parse_qkernelz_zero_values() {
    assert_eq!(
        parse_instruction("QKERNELZ Q0, Q0, 0, Z0, Z0").unwrap(),
        Instruction::QKernelZ { dst: 0, src: 0, kernel: 0, zctx0: 0, zctx1: 0 }
    );
}

#[test]
fn test_parse_qkernelz_missing_operand() {
    assert!(parse_instruction("QKERNELZ Q1, Q0, 6, Z2").is_err());
}

#[test]
fn test_parse_qkernelz_too_many_operands() {
    assert!(parse_instruction("QKERNELZ Q1, Q0, 6, Z2, Z3, Z4").is_err());
}

#[test]
fn test_parse_qkernelz_invalid_register() {
    // "abc" is not a valid register token
    assert!(parse_instruction("QKERNELZ Q1, Q0, 6, abc, Z3").is_err());
}

#[test]
fn test_parse_program_qkernelf_workflow() {
    let source = "\
QPREP Q0, 0
FLDI F0, 1
FLDI F1, 0
QKERNELF Q1, Q0, 5, F0, F1
QOBSERVE H0, Q1
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 6);
    assert_eq!(program[3], Instruction::QKernelF { dst: 1, src: 0, kernel: 5, fctx0: 0, fctx1: 1 });
}

#[test]
fn test_parse_program_qkernelz_workflow() {
    let source = "\
QPREP Q0, 0
ZLDI Z0, 1, 2
ZLDI Z1, 0, 0
QKERNELZ Q1, Q0, 6, Z0, Z1
QSAMPLE H0, Q1
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 6);
    assert_eq!(program[3], Instruction::QKernelZ { dst: 1, src: 0, kernel: 6, zctx0: 0, zctx1: 1 });
}

// ===========================================================================
// QPREPR / QENCODE parse tests
// ===========================================================================

#[test]
fn test_parse_qprepr() {
    assert_eq!(
        parse_instruction("QPREPR Q0, R3").unwrap(),
        Instruction::QPrepR { dst: 0, dist_reg: 3 }
    );
}

#[test]
fn test_parse_qprepr_max_regs() {
    assert_eq!(
        parse_instruction("QPREPR Q7, R15").unwrap(),
        Instruction::QPrepR { dst: 7, dist_reg: 15 }
    );
}

#[test]
fn test_parse_qprepr_missing_operand() {
    assert!(parse_instruction("QPREPR Q0").is_err());
}

#[test]
fn test_parse_qprepr_too_many_operands() {
    assert!(parse_instruction("QPREPR Q0, R3, R4").is_err());
}

#[test]
fn test_parse_qprepr_invalid_register() {
    assert!(parse_instruction("QPREPR Q0, 5").is_err());
}

#[test]
fn test_parse_qencode_r_file() {
    assert_eq!(
        parse_instruction("QENCODE Q0, R0, 4, 0").unwrap(),
        Instruction::QEncode { dst: 0, src_base: 0, count: 4, file_sel: 0 }
    );
}

#[test]
fn test_parse_qencode_f_file() {
    assert_eq!(
        parse_instruction("QENCODE Q1, F2, 2, 1").unwrap(),
        Instruction::QEncode { dst: 1, src_base: 2, count: 2, file_sel: 1 }
    );
}

#[test]
fn test_parse_qencode_z_file() {
    assert_eq!(
        parse_instruction("QENCODE Q3, Z0, 8, 2").unwrap(),
        Instruction::QEncode { dst: 3, src_base: 0, count: 8, file_sel: 2 }
    );
}

#[test]
fn test_parse_qencode_invalid_file_sel() {
    assert!(parse_instruction("QENCODE Q0, R0, 4, 5").is_err());
}

#[test]
fn test_parse_qencode_missing_operand() {
    assert!(parse_instruction("QENCODE Q0, R0, 4").is_err());
}

#[test]
fn test_parse_qencode_too_many_operands() {
    assert!(parse_instruction("QENCODE Q0, R0, 4, 0, extra").is_err());
}

#[test]
fn test_parse_program_qprepr_workflow() {
    let source = "\
ILDI R0, 0
QPREPR Q0, R0
QOBSERVE H0, Q0
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 4);
    assert_eq!(program[1], Instruction::QPrepR { dst: 0, dist_reg: 0 });
}

#[test]
fn test_parse_program_qencode_workflow() {
    let source = "\
FLDI F0, 1
FLDI F1, 0
QENCODE Q0, F0, 2, 1
QOBSERVE H0, Q0
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 5);
    assert_eq!(program[2], Instruction::QEncode { dst: 0, src_base: 0, count: 2, file_sel: 1 });
}

// =============================================================================
// QHADM, QFLIP, QPHASE parse tests
// =============================================================================

#[test]
fn test_parse_qhadm() {
    let instr = parse_instruction("QHADM Q0, Q0, R0").unwrap();
    assert_eq!(instr, Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 });
}

#[test]
fn test_parse_qhadm_typical() {
    let instr = parse_instruction("QHADM Q1, Q2, R5").unwrap();
    assert_eq!(instr, Instruction::QHadM { dst: 1, src: 2, mask_reg: 5 });
}

#[test]
fn test_parse_qhadm_wrong_operand_count() {
    assert!(parse_instruction("QHADM Q0, Q0").is_err());
    assert!(parse_instruction("QHADM Q0, Q0, R0, R1").is_err());
}

#[test]
fn test_parse_qflip() {
    let instr = parse_instruction("QFLIP Q0, Q0, R0").unwrap();
    assert_eq!(instr, Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 });
}

#[test]
fn test_parse_qflip_typical() {
    let instr = parse_instruction("QFLIP Q3, Q1, R7").unwrap();
    assert_eq!(instr, Instruction::QFlip { dst: 3, src: 1, mask_reg: 7 });
}

#[test]
fn test_parse_qflip_wrong_operand_count() {
    assert!(parse_instruction("QFLIP Q0, Q0").is_err());
}

#[test]
fn test_parse_qphase() {
    let instr = parse_instruction("QPHASE Q0, Q0, R0").unwrap();
    assert_eq!(instr, Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 });
}

#[test]
fn test_parse_qphase_typical() {
    let instr = parse_instruction("QPHASE Q2, Q0, R3").unwrap();
    assert_eq!(instr, Instruction::QPhase { dst: 2, src: 0, mask_reg: 3 });
}

#[test]
fn test_parse_qphase_wrong_operand_count() {
    assert!(parse_instruction("QPHASE Q0, Q0").is_err());
    assert!(parse_instruction("QPHASE Q0, Q0, R0, R1").is_err());
}

#[test]
fn test_parse_program_masked_workflow() {
    let source = "\
QPREP Q0, 1
ILDI R0, 3
QHADM Q0, Q0, R0
QOBSERVE H0, Q0
HALT
";
    let program = parse_program(source).unwrap();
    assert_eq!(program.len(), 5);
    assert_eq!(program[2], Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 });
    assert_eq!(program[3], Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 });
}

// ===========================================================================
// Example file parsing integration tests
// ===========================================================================

#[test]
fn test_parse_all_example_files() {
    let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples");

    let mut count = 0;
    for entry in std::fs::read_dir(&examples_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "cqam") {
            let source = std::fs::read_to_string(&path).unwrap();
            let result = parse_program(&source);
            assert!(
                result.is_ok(),
                "Failed to parse {}: {:?}",
                path.display(),
                result.unwrap_err()
            );
            let instrs = result.unwrap();
            assert!(
                !instrs.is_empty(),
                "{} parsed to zero instructions",
                path.display()
            );
            count += 1;
        }
    }
    // Ensure we found and parsed a reasonable number of examples
    assert!(count >= 20, "Expected at least 20 example files, found {}", count);
}

// =============================================================================
// QCNOT, QROT, QMEAS parse tests
// =============================================================================

#[test]
fn test_parse_qcnot() {
    assert_eq!(
        parse_instruction("QCNOT Q0, Q1, R2, R3").unwrap(),
        Instruction::QCnot { dst: 0, src: 1, ctrl_qubit_reg: 2, tgt_qubit_reg: 3 }
    );
}

#[test]
fn test_parse_qcnot_max_regs() {
    assert_eq!(
        parse_instruction("QCNOT Q7, Q7, R15, R15").unwrap(),
        Instruction::QCnot { dst: 7, src: 7, ctrl_qubit_reg: 15, tgt_qubit_reg: 15 }
    );
}

#[test]
fn test_parse_qcnot_missing_operand() {
    assert!(parse_instruction("QCNOT Q0, Q1, R2").is_err());
}

#[test]
fn test_parse_qcnot_too_many_operands() {
    assert!(parse_instruction("QCNOT Q0, Q1, R2, R3, R4").is_err());
}

#[test]
fn test_parse_qrot_x_axis() {
    assert_eq!(
        parse_instruction("QROT Q0, Q1, R2, X, F3").unwrap(),
        Instruction::QRot { dst: 0, src: 1, qubit_reg: 2, axis: 0, angle_freg: 3 }
    );
}

#[test]
fn test_parse_qrot_y_axis() {
    assert_eq!(
        parse_instruction("QROT Q0, Q1, R2, Y, F3").unwrap(),
        Instruction::QRot { dst: 0, src: 1, qubit_reg: 2, axis: 1, angle_freg: 3 }
    );
}

#[test]
fn test_parse_qrot_z_axis() {
    assert_eq!(
        parse_instruction("QROT Q0, Q1, R2, Z, F3").unwrap(),
        Instruction::QRot { dst: 0, src: 1, qubit_reg: 2, axis: 2, angle_freg: 3 }
    );
}

#[test]
fn test_parse_qrot_numeric_axis() {
    assert_eq!(
        parse_instruction("QROT Q0, Q1, R2, 0, F3").unwrap(),
        Instruction::QRot { dst: 0, src: 1, qubit_reg: 2, axis: 0, angle_freg: 3 }
    );
}

#[test]
fn test_parse_qrot_missing_operand() {
    assert!(parse_instruction("QROT Q0, Q1, R2, X").is_err());
}

#[test]
fn test_parse_qrot_invalid_axis() {
    assert!(parse_instruction("QROT Q0, Q1, R2, W, F3").is_err());
}

#[test]
fn test_parse_qmeas() {
    assert_eq!(
        parse_instruction("QMEAS R0, Q1, R2").unwrap(),
        Instruction::QMeas { dst_r: 0, src_q: 1, qubit_reg: 2 }
    );
}

#[test]
fn test_parse_qmeas_max_regs() {
    assert_eq!(
        parse_instruction("QMEAS R15, Q7, R15").unwrap(),
        Instruction::QMeas { dst_r: 15, src_q: 7, qubit_reg: 15 }
    );
}

#[test]
fn test_parse_qmeas_missing_operand() {
    assert!(parse_instruction("QMEAS R0, Q1").is_err());
}

#[test]
fn test_parse_qmeas_too_many_operands() {
    assert!(parse_instruction("QMEAS R0, Q1, R2, R3").is_err());
}

// ===========================================================================
// QTENSOR, QCUSTOM, QCZ, QSWAP parse tests
// ===========================================================================

#[test]
fn test_parse_qtensor() {
    assert_eq!(
        parse_instruction("QTENSOR Q0, Q1, Q2").unwrap(),
        Instruction::QTensor { dst: 0, src0: 1, src1: 2 }
    );
}

#[test]
fn test_parse_qtensor_error() {
    assert!(parse_instruction("QTENSOR Q0, Q1").is_err());
    assert!(parse_instruction("QTENSOR Q0, Q1, Q2, Q3").is_err());
}

#[test]
fn test_parse_qcustom() {
    assert_eq!(
        parse_instruction("QCUSTOM Q0, Q1, R2, R3").unwrap(),
        Instruction::QCustom { dst: 0, src: 1, base_addr_reg: 2, dim_reg: 3 }
    );
}

#[test]
fn test_parse_qcustom_error() {
    assert!(parse_instruction("QCUSTOM Q0, Q1, R2").is_err());
}

#[test]
fn test_parse_qcz() {
    assert_eq!(
        parse_instruction("QCZ Q0, Q1, R2, R3").unwrap(),
        Instruction::QCz { dst: 0, src: 1, ctrl_qubit_reg: 2, tgt_qubit_reg: 3 }
    );
}

#[test]
fn test_parse_qcz_error() {
    assert!(parse_instruction("QCZ Q0, Q1").is_err());
}

#[test]
fn test_parse_qswap() {
    assert_eq!(
        parse_instruction("QSWAP Q0, Q1, R2, R3").unwrap(),
        Instruction::QSwap { dst: 0, src: 1, qubit_a_reg: 2, qubit_b_reg: 3 }
    );
}

#[test]
fn test_parse_qswap_error() {
    assert!(parse_instruction("QSWAP Q0").is_err());
}

// ===========================================================================
// Mixed-state, partial-trace, reset, and float math parse tests
// ===========================================================================

#[test]
fn test_parse_qmixed() {
    assert_eq!(
        parse_instruction("QMIXED Q0, R1, R2").unwrap(),
        Instruction::QMixed { dst: 0, base_addr_reg: 1, count_reg: 2 }
    );
}

#[test]
fn test_parse_qmixed_error() {
    assert!(parse_instruction("QMIXED Q0").is_err());
    assert!(parse_instruction("QMIXED Q0, R1").is_err());
}

#[test]
fn test_parse_qprepn() {
    assert_eq!(
        parse_instruction("QPREPN Q0, 1, R2").unwrap(),
        Instruction::QPrepN { dst: 0, dist: 1, qubit_count_reg: 2 }
    );
}

#[test]
fn test_parse_qprepn_error() {
    assert!(parse_instruction("QPREPN Q0, 1").is_err());
}

#[test]
fn test_parse_fsin() {
    assert_eq!(
        parse_instruction("FSIN F0, F1").unwrap(),
        Instruction::FSin { dst: 0, src: 1 }
    );
}

#[test]
fn test_parse_fcos() {
    assert_eq!(
        parse_instruction("FCOS F3, F4").unwrap(),
        Instruction::FCos { dst: 3, src: 4 }
    );
}

#[test]
fn test_parse_fatan2() {
    assert_eq!(
        parse_instruction("FATAN2 F0, F1, F2").unwrap(),
        Instruction::FAtan2 { dst: 0, lhs: 1, rhs: 2 }
    );
}

#[test]
fn test_parse_fsqrt() {
    assert_eq!(
        parse_instruction("FSQRT F5, F6").unwrap(),
        Instruction::FSqrt { dst: 5, src: 6 }
    );
}

#[test]
fn test_parse_qptrace() {
    assert_eq!(
        parse_instruction("QPTRACE Q0, Q1, R2").unwrap(),
        Instruction::QPtrace { dst: 0, src: 1, num_qubits_a_reg: 2 }
    );
}

#[test]
fn test_parse_qreset() {
    assert_eq!(
        parse_instruction("QRESET Q0, Q1, R2").unwrap(),
        Instruction::QReset { dst: 0, src: 1, qubit_reg: 2 }
    );
}

#[test]
fn test_parse_fsin_error() {
    assert!(parse_instruction("FSIN F0").is_err());
}

#[test]
fn test_parse_fatan2_error() {
    assert!(parse_instruction("FATAN2 F0, F1").is_err());
}
