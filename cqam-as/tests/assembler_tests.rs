// cqam-as/tests/assembler_tests.rs
//
// Phase 5: Integration tests for the two-pass assembler.

use cqam_as::assembler::{assemble, assemble_source, AssemblyResult};
use cqam_core::instruction::Instruction;
use cqam_core::opcode;

// =============================================================================
// Helper functions
// =============================================================================

/// Assemble a program and assert it succeeds.
fn assemble_ok(instrs: Vec<Instruction>) -> AssemblyResult {
    assemble(&instrs).expect("assembly should succeed")
}

/// Assemble source text and assert it succeeds.
fn assemble_source_ok(source: &str) -> AssemblyResult {
    assemble_source(source).expect("assembly should succeed")
}

/// Decode a word and assert it succeeds.
fn decode_ok(word: u32) -> Instruction {
    opcode::decode(word).expect("decode should succeed")
}

// =============================================================================
// Single-instruction round-trip tests
// =============================================================================

#[test]
fn test_roundtrip_each_integer_op() {
    let cases: Vec<Instruction> = vec![
        Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ISub { dst: 3, lhs: 4, rhs: 5 },
        Instruction::IMul { dst: 6, lhs: 7, rhs: 8 },
        Instruction::IDiv { dst: 9, lhs: 10, rhs: 11 },
        Instruction::IMod { dst: 12, lhs: 13, rhs: 14 },
        Instruction::IAnd { dst: 0, lhs: 15, rhs: 1 },
        Instruction::IOr { dst: 2, lhs: 3, rhs: 4 },
        Instruction::IXor { dst: 5, lhs: 6, rhs: 7 },
        Instruction::INot { dst: 8, src: 9 },
        Instruction::IShl { dst: 10, src: 11, amt: 5 },
        Instruction::IShr { dst: 12, src: 13, amt: 63 },
        Instruction::ILdi { dst: 0, imm: 0 },
        Instruction::ILdi { dst: 15, imm: -32768 },
        Instruction::ILdi { dst: 1, imm: 32767 },
        Instruction::ILdm { dst: 2, addr: 0 },
        Instruction::ILdm { dst: 3, addr: 65535 },
        Instruction::IStr { src: 4, addr: 1024 },
        Instruction::IEq { dst: 5, lhs: 6, rhs: 7 },
        Instruction::ILt { dst: 8, lhs: 9, rhs: 10 },
        Instruction::IGt { dst: 11, lhs: 12, rhs: 13 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_each_float_op() {
    let cases: Vec<Instruction> = vec![
        Instruction::FAdd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::FSub { dst: 3, lhs: 4, rhs: 5 },
        Instruction::FMul { dst: 6, lhs: 7, rhs: 8 },
        Instruction::FDiv { dst: 9, lhs: 10, rhs: 11 },
        Instruction::FLdi { dst: 0, imm: 100 },
        Instruction::FLdm { dst: 1, addr: 256 },
        Instruction::FStr { src: 2, addr: 512 },
        Instruction::FEq { dst: 3, lhs: 4, rhs: 5 },
        Instruction::FLt { dst: 6, lhs: 7, rhs: 8 },
        Instruction::FGt { dst: 9, lhs: 10, rhs: 11 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_each_complex_op() {
    let cases: Vec<Instruction> = vec![
        Instruction::ZAdd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ZSub { dst: 3, lhs: 4, rhs: 5 },
        Instruction::ZMul { dst: 6, lhs: 7, rhs: 8 },
        Instruction::ZDiv { dst: 9, lhs: 10, rhs: 11 },
        Instruction::ZLdi { dst: 0, imm_re: 127, imm_im: -128 },
        Instruction::ZLdm { dst: 1, addr: 100 },
        Instruction::ZStr { src: 2, addr: 200 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_conversions() {
    let cases: Vec<Instruction> = vec![
        Instruction::CvtIF { dst_f: 0, src_i: 1 },
        Instruction::CvtFI { dst_i: 2, src_f: 3 },
        Instruction::CvtFZ { dst_z: 4, src_f: 5 },
        Instruction::CvtZF { dst_f: 6, src_z: 7 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_no_operand_instructions() {
    let cases: Vec<Instruction> = vec![
        Instruction::Nop,
        Instruction::Ret,
        Instruction::Halt,
        Instruction::HFork,
        Instruction::HMerge,
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_quantum_ops() {
    let cases: Vec<Instruction> = vec![
        Instruction::QPrep { dst: 0, dist: 0 },
        Instruction::QPrep { dst: 7, dist: 3 },
        Instruction::QKernel { dst: 1, src: 2, kernel: 4, ctx0: 3, ctx1: 4 },
        Instruction::QObserve { dst_h: 0, src_q: 1 },
        Instruction::QLoad { dst_q: 3, addr: 255 },
        Instruction::QStore { src_q: 7, addr: 0 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_hreduce() {
    let instr = Instruction::HReduce { src: 2, dst: 5, func: 10 };
    let result = assemble_ok(vec![instr.clone()]);
    let decoded = decode_ok(result.code[0]);
    assert_eq!(decoded, instr);
}

// =============================================================================
// Label resolution tests
// =============================================================================

#[test]
fn test_forward_label_reference() {
    let program = vec![
        Instruction::Jmp { target: "end".to_string() },
        Instruction::Nop,
        Instruction::Label("end".to_string()),
        Instruction::Halt,
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["end"], 2);
    // The JMP word should encode address 2
    let decoded = decode_ok(result.code[0]);
    assert_eq!(decoded, Instruction::Jmp { target: "@2".to_string() });
}

#[test]
fn test_backward_label_reference() {
    let program = vec![
        Instruction::Label("loop".to_string()),
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Jmp { target: "loop".to_string() },
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["loop"], 0);
    let decoded = decode_ok(result.code[2]);
    assert_eq!(decoded, Instruction::Jmp { target: "@0".to_string() });
}

#[test]
fn test_nested_call_ret() {
    let program = vec![
        Instruction::Call { target: "sub".to_string() },
        Instruction::Halt,
        Instruction::Label("sub".to_string()),
        Instruction::ILdi { dst: 0, imm: 99 },
        Instruction::Ret,
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["sub"], 2);
}

#[test]
fn test_jif_label_resolution() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Jif { pred: 0, target: "taken".to_string() },
        Instruction::Halt,
        Instruction::Label("taken".to_string()),
        Instruction::ILdi { dst: 1, imm: 42 },
        Instruction::Halt,
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["taken"], 3);
}

#[test]
fn test_hcexec_label_resolution() {
    let program = vec![
        Instruction::HCExec { flag: 0, target: "branch".to_string() },
        Instruction::Halt,
        Instruction::Label("branch".to_string()),
        Instruction::HMerge,
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["branch"], 2);
}

// =============================================================================
// Entry point tests
// =============================================================================

#[test]
fn test_entry_point_no_labels() {
    let result = assemble_ok(vec![Instruction::Halt]);
    assert_eq!(result.entry_point, 0);
}

#[test]
fn test_entry_point_leading_labels() {
    let result = assemble_ok(vec![
        Instruction::Label("a".to_string()),
        Instruction::Label("b".to_string()),
        Instruction::Halt,
    ]);
    assert_eq!(result.entry_point, 2);
}

// =============================================================================
// Error case tests
// =============================================================================

#[test]
fn test_error_undefined_label() {
    let program = vec![Instruction::Jmp { target: "missing".to_string() }];
    let result = assemble(&program);
    assert!(result.is_err(), "Expected error for undefined label");
}

#[test]
fn test_error_duplicate_label() {
    let program = vec![
        Instruction::Label("dup".to_string()),
        Instruction::Label("dup".to_string()),
    ];
    let result = assemble(&program);
    assert!(result.is_err(), "Expected error for duplicate label");
}

// =============================================================================
// Source-level assembly tests
// =============================================================================

#[test]
fn test_assemble_source_basic() {
    let source = "\
ILDI R0, 42
ILDI R1, 10
IADD R2, R0, R1
HALT
";
    let result = assemble_source_ok(source);
    assert_eq!(result.code.len(), 4);
}

#[test]
fn test_assemble_source_with_labels_and_comments() {
    let source = "\
# This is a comment
LABEL: start
ILDI R0, 1       // load counter
LABEL: loop
ISUB R0, R0, R0  # decrement
JIF R0, loop
HALT
";
    let result = assemble_source_ok(source);
    assert!(result.labels.contains_key("start"));
    assert!(result.labels.contains_key("loop"));
}

#[test]
fn test_assemble_source_all_quantum() {
    let source = "\
QPREP Q0, 0
QKERNEL Q1, Q0, 2, R0, R1
QOBSERVE H0, Q1
QLOAD Q2, 10
QSTORE Q3, 20
HALT
";
    let result = assemble_source_ok(source);
    assert_eq!(result.code.len(), 6);
}
