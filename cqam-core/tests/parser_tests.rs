// cqam-core/tests/parser_tests.rs
//
// Phase 4: Test the parser with Result-based error handling.

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
        Instruction::QObserve { dst_h: 0, src_q: 1 }
    );
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
        Instruction::QObserve { dst_h: 7, src_q: 7 }
    );
}

// ===========================================================================
// Error cases (Phase 4: now return Err instead of Nop)
// ===========================================================================

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
    assert_eq!(program[2], Instruction::QObserve { dst_h: 0, src_q: 1 });
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

// ===========================================================================
// New error case tests (Phase 4)
// ===========================================================================

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
