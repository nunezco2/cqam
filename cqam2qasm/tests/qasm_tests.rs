// cqam2qasm/tests/qasm_tests.rs
//
// Phase 7: Integration tests for cqam2qasm.
// These tests exercise the full pipeline: Instruction -> EmitConfig -> QASM output.

use cqam_core::instruction::*;
use cqam_codegen::qasm::{
    EmitConfig, QasmFormat, emit_qasm_program,
};

// ===========================================================================
// Section 1: Integer operation tests
// ===========================================================================

#[test]
fn test_qasm_integer_ops_no_redeclaration() {
    // Multiple uses of R3 must produce exactly one "int[64] R3;" declaration.
    let program = vec![
        Instruction::ILdi { dst: 3, imm: 10 },
        Instruction::IAdd { dst: 3, lhs: 3, rhs: 3 },
        Instruction::IMul { dst: 3, lhs: 3, rhs: 3 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);

    // Exactly one declaration
    let decl_count = output.matches("int[64] R3;").count();
    assert_eq!(decl_count, 1, "R3 should be declared exactly once");

    // Body lines should NOT have type prefix
    assert!(!output.contains("int[64] R3 = "), "Body lines must not redeclare type");

    // Body lines should be bare assignments
    assert!(output.contains("R3 = 10;"));
    assert!(output.contains("R3 = R3 + R3;"));
    assert!(output.contains("R3 = R3 * R3;"));
}

#[test]
fn test_qasm_integer_all_ops() {
    let program = vec![
        Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ISub { dst: 3, lhs: 4, rhs: 5 },
        Instruction::IMul { dst: 6, lhs: 7, rhs: 8 },
        Instruction::IDiv { dst: 9, lhs: 10, rhs: 11 },
        Instruction::IMod { dst: 12, lhs: 13, rhs: 14 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("R0 = R1 + R2;"));
    assert!(output.contains("R3 = R4 - R5;"));
    assert!(output.contains("R6 = R7 * R8;"));
    assert!(output.contains("R9 = R10 / R11;"));
    assert!(output.contains("R12 = R13 % R14;"));
}

#[test]
fn test_qasm_integer_bitwise() {
    let program = vec![
        Instruction::IAnd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::IOr { dst: 3, lhs: 4, rhs: 5 },
        Instruction::IXor { dst: 6, lhs: 7, rhs: 8 },
        Instruction::INot { dst: 9, src: 10 },
        Instruction::IShl { dst: 11, src: 12, amt: 4 },
        Instruction::IShr { dst: 13, src: 14, amt: 2 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("R0 = R1 & R2;"));
    assert!(output.contains("R3 = R4 | R5;"));
    assert!(output.contains("R6 = R7 ^ R8;"));
    assert!(output.contains("R9 = ~R10;"));
    assert!(output.contains("R11 = R12 << 4;"));
    assert!(output.contains("R13 = R14 >> 2;"));
}

#[test]
fn test_qasm_integer_memory() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdm { dst: 1, addr: 100 },
        Instruction::IStr { src: 0, addr: 200 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);

    // CMEM should be declared since ILdm and IStr are present.
    assert!(output.contains("array[int[64], 65536] CMEM;"));

    // Body lines reference CMEM
    assert!(output.contains("R0 = 42;"));
    assert!(output.contains("R1 = CMEM[100];"));
    assert!(output.contains("CMEM[200] = R0;"));
}

#[test]
fn test_qasm_integer_comparison() {
    let program = vec![
        Instruction::IEq { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ILt { dst: 3, lhs: 4, rhs: 5 },
        Instruction::IGt { dst: 6, lhs: 7, rhs: 8 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("R0 = (R1 == R2) ? 1 : 0;"));
    assert!(output.contains("R3 = (R4 < R5) ? 1 : 0;"));
    assert!(output.contains("R6 = (R7 > R8) ? 1 : 0;"));
}

// ===========================================================================
// Section 2: Float operation tests
// ===========================================================================

#[test]
fn test_qasm_float_ops() {
    let program = vec![
        Instruction::FAdd { dst: 2, lhs: 0, rhs: 1 },
        Instruction::FSub { dst: 3, lhs: 2, rhs: 0 },
        Instruction::FMul { dst: 4, lhs: 2, rhs: 3 },
        Instruction::FDiv { dst: 5, lhs: 4, rhs: 3 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);

    // Declarations
    assert!(output.contains("float[64] F0;"));
    assert!(output.contains("float[64] F2;"));

    // Body lines (no type prefix)
    assert!(output.contains("F2 = F0 + F1;"));
    assert!(output.contains("F3 = F2 - F0;"));
    assert!(output.contains("F4 = F2 * F3;"));
    assert!(output.contains("F5 = F4 / F3;"));
}

#[test]
fn test_qasm_float_comparison_cross_file() {
    let program = vec![
        Instruction::FEq { dst: 0, lhs: 1, rhs: 2 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("int[64] R0;"));
    assert!(output.contains("float[64] F1;"));
    assert!(output.contains("float[64] F2;"));
    assert!(output.contains("R0 = (F1 == F2) ? 1 : 0;"));
}

#[test]
fn test_qasm_float_memory() {
    let program = vec![
        Instruction::FLdi { dst: 0, imm: 314 },
        Instruction::FLdm { dst: 1, addr: 500 },
        Instruction::FStr { src: 0, addr: 600 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("F0 = 314.0;"));
    assert!(output.contains("F1 = CMEM[500];"));
    assert!(output.contains("CMEM[600] = F0;"));
    assert!(output.contains("array[int[64], 65536] CMEM;"));
}

// ===========================================================================
// Section 3: Complex arithmetic tests
// ===========================================================================

#[test]
fn test_qasm_complex_lowering() {
    let program = vec![
        Instruction::ZAdd { dst: 0, lhs: 1, rhs: 2 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);

    // Declarations should include paired float vars for Z0, Z1, Z2
    assert!(output.contains("float[64] Z0_re;"));
    assert!(output.contains("float[64] Z0_im;"));
    assert!(output.contains("float[64] Z1_re;"));
    assert!(output.contains("float[64] Z1_im;"));
    assert!(output.contains("float[64] Z2_re;"));
    assert!(output.contains("float[64] Z2_im;"));

    // Body should contain real float operations
    assert!(output.contains("Z0_re = Z1_re + Z2_re;"));
    assert!(output.contains("Z0_im = Z1_im + Z2_im;"));
}

#[test]
fn test_qasm_complex_mul_lowering() {
    let program = vec![
        Instruction::ZMul { dst: 0, lhs: 1, rhs: 2 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    // Check cross-term multiplication formula
    assert!(output.contains("Z0_re = Z1_re * Z2_re - Z1_im * Z2_im;"));
    assert!(output.contains("Z0_im = Z1_re * Z2_im + Z1_im * Z2_re;"));
}

#[test]
fn test_qasm_complex_div_lowering() {
    let program = vec![
        Instruction::ZDiv { dst: 3, lhs: 1, rhs: 2 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("Z3_re ="));
    assert!(output.contains("Z3_im ="));
    assert!(output.contains("ZDIV"));
}

#[test]
fn test_qasm_complex_ldi() {
    let program = vec![
        Instruction::ZLdi { dst: 0, imm_re: 3, imm_im: -2 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("Z0_re = 3.0;"));
    assert!(output.contains("Z0_im = -2.0;"));
}

#[test]
fn test_qasm_complex_memory() {
    let program = vec![
        Instruction::ZLdm { dst: 1, addr: 100 },
        Instruction::ZStr { src: 2, addr: 200 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("Z1_re = CMEM[100];"));
    assert!(output.contains("Z1_im = CMEM[101];"));
    assert!(output.contains("CMEM[200] = Z2_re;"));
    assert!(output.contains("CMEM[201] = Z2_im;"));
    assert!(output.contains("array[int[64], 65536] CMEM;"));
}

// ===========================================================================
// Section 4: Quantum flow tests
// ===========================================================================

#[test]
fn test_qasm_quantum_flow() {
    let program = vec![
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        Instruction::QKernel {
            dst: 0, src: 0, kernel: kernel_id::FOURIER, ctx0: 0, ctx1: 0,
        },
        Instruction::QObserve { dst_h: 0, src_q: 0 },
    ];
    let config = EmitConfig {
        expand_templates: false,
        ..EmitConfig::standalone()
    };
    let output = emit_qasm_program(&program, &config);

    // Declarations
    assert!(output.contains("qubit[16] q0;"));
    assert!(output.contains("bit[16] H0;"));
    assert!(output.contains("int[64] R0;"));

    // Body sequence
    assert!(output.contains("reset q0;"));
    assert!(output.contains("fourier q0;"));
    assert!(output.contains("H0 = measure q0;"));
}

#[test]
fn test_qasm_qprep_all_distributions() {
    for (dist, name) in [
        (dist_id::UNIFORM, "uniform"),
        (dist_id::ZERO, "zero"),
        (dist_id::BELL, "bell"),
        (dist_id::GHZ, "ghz"),
    ] {
        let instr = Instruction::QPrep { dst: 0, dist };
        let lines = instr.to_qasm(&EmitConfig::fragment());
        assert_eq!(lines[0], "reset q0;");
        assert!(lines[1].contains(name), "Expected comment to contain '{}'", name);
    }
}

#[test]
fn test_qasm_qobserve_uses_lowercase_q() {
    let instr = Instruction::QObserve { dst_h: 0, src_q: 1 };
    let lines = instr.to_qasm(&EmitConfig::fragment());
    assert!(lines[0].contains("measure q1"));
    assert!(!lines[0].contains("measure Q1"));
}

// ===========================================================================
// Section 5: Standalone vs fragment tests
// ===========================================================================

#[test]
fn test_qasm_standalone_vs_fragment() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        Instruction::QObserve { dst_h: 0, src_q: 0 },
    ];
    let standalone = emit_qasm_program(&program, &EmitConfig::standalone());
    let fragment = emit_qasm_program(&program, &EmitConfig::fragment());

    // Standalone has header, fragment does not
    assert!(standalone.contains("OPENQASM 3.0;"));
    assert!(!fragment.contains("OPENQASM 3.0;"));

    // Standalone has declarations, fragment does not
    assert!(standalone.contains("int[64] R0;"));
    assert!(!fragment.contains("int[64] R0;"));

    // Both have body lines
    assert!(standalone.contains("R0 = 42;"));
    assert!(fragment.contains("R0 = 42;"));
    assert!(standalone.contains("reset q0;"));
    assert!(fragment.contains("reset q0;"));
    assert!(standalone.contains("H0 = measure q0;"));
    assert!(fragment.contains("H0 = measure q0;"));
}

#[test]
fn test_qasm_fragment_embeddable() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
    ];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);

    assert!(!output.contains("OPENQASM"));
    assert!(!output.contains("include"));
    assert!(!output.contains("int[64] R0;"));
    assert!(!output.contains("==="));
    assert_eq!(output.trim(), "R0 = 1;");
}

// ===========================================================================
// Section 6: Label and control flow tests
// ===========================================================================

#[test]
fn test_qasm_label_emission() {
    let program = vec![
        Instruction::Label("START".into()),
        Instruction::ILdi { dst: 0, imm: 0 },
        Instruction::Label("LOOP".into()),
        Instruction::IAdd { dst: 0, lhs: 0, rhs: 0 },
        Instruction::Jmp { target: "LOOP".into() },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("START:"));
    assert!(output.contains("LOOP:"));
    assert!(output.contains("goto LOOP;"));
}

#[test]
fn test_qasm_control_flow() {
    let program = vec![
        Instruction::Jmp { target: "END".into() },
        Instruction::Jif { pred: 0, target: "THEN".into() },
        Instruction::Call { target: "FUNC".into() },
        Instruction::Ret,
        Instruction::Halt,
    ];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("goto END;"));
    assert!(output.contains("if (R0 != 0) goto THEN;"));
    assert!(output.contains("// CALL FUNC"));
    assert!(output.contains("// RET"));
    assert!(output.contains("// HALT"));
}

// ===========================================================================
// Section 7: Hybrid annotation tests
// ===========================================================================

#[test]
fn test_qasm_hybrid_annotations() {
    let program = vec![
        Instruction::HFork,
        Instruction::HCExec { flag: flag_id::QF, target: "QBRANCH".into() },
        Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::MEAN },
        Instruction::HMerge,
    ];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);

    assert!(output.contains("@cqam.hfork"));
    assert!(output.contains("@cqam.hcexec"));
    assert!(output.contains("@cqam.hreduce"));
    assert!(output.contains("@cqam.hmerge"));
    assert!(output.contains("QF"));
    assert!(output.contains("mean"));
}

#[test]
fn test_qasm_hreduce_int_vs_float_file() {
    // Round (func 0) -> int register (R)
    let reduce_int = Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ROUND };
    let lines_int = reduce_int.to_qasm(&EmitConfig::fragment());
    assert!(lines_int[0].contains("R1"));
    assert!(lines_int[0].contains("round"));

    // Magnitude (func 6) -> float register (F)
    let reduce_float = Instruction::HReduce { src: 0, dst: 2, func: reduce_fn::MAGNITUDE };
    let lines_float = reduce_float.to_qasm(&EmitConfig::fragment());
    assert!(lines_float[0].contains("F2"));
    assert!(lines_float[0].contains("magnitude"));
}

// ===========================================================================
// Section 8: Type conversion tests
// ===========================================================================

#[test]
fn test_qasm_type_conversion() {
    let program = vec![
        Instruction::CvtIF { dst_f: 0, src_i: 1 },
        Instruction::CvtFI { dst_i: 2, src_f: 3 },
        Instruction::CvtFZ { dst_z: 0, src_f: 1 },
        Instruction::CvtZF { dst_f: 2, src_z: 0 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);

    // Declarations: int regs, float regs, complex regs should all be present
    assert!(output.contains("int[64]"));
    assert!(output.contains("float[64]"));
    assert!(output.contains("Z0_re"));

    // Body should use QASM 3.0 cast syntax
    assert!(output.contains("F0 = float[64](R1);"));
    assert!(output.contains("R2 = int[64](F3);"));
    assert!(output.contains("Z0_re = F1;"));
    assert!(output.contains("Z0_im = 0.0;"));
    assert!(output.contains("F2 = Z0_re;"));
}

// ===========================================================================
// Section 9: Kernel template expansion tests
// ===========================================================================

#[test]
fn test_qasm_kernel_expansion() {
    // When expand_templates is true and template does NOT exist,
    // we get the fallback comment.
    let instr = Instruction::QKernel {
        dst: 0, src: 1, kernel: kernel_id::FOURIER, ctx0: 2, ctx1: 3,
    };
    let config = EmitConfig {
        expand_templates: true,
        template_dir: "/nonexistent/path".to_string(),
        ..EmitConfig::standalone()
    };
    let lines = instr.to_qasm(&config);
    assert!(lines.iter().any(|l| l.contains("[Missing QASM template for fourier]")));
}

#[test]
fn test_qasm_kernel_no_expansion() {
    let instr = Instruction::QKernel {
        dst: 0, src: 1, kernel: kernel_id::ENTANGLE, ctx0: 2, ctx1: 3,
    };
    let config = EmitConfig {
        expand_templates: false,
        ..EmitConfig::standalone()
    };
    let lines = instr.to_qasm(&config);
    assert!(lines.iter().any(|l| l == "entangle q0;"));
}

// ===========================================================================
// Section 10: Edge case tests
// ===========================================================================

#[test]
fn test_qasm_nop_produces_nothing() {
    let program = vec![Instruction::Nop, Instruction::Nop];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);
    assert!(output.trim().is_empty());
}

#[test]
fn test_qasm_all_register_files_declared() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },         // int
        Instruction::FLdi { dst: 0, imm: 1 },         // float
        Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: 0 }, // complex
        Instruction::QPrep { dst: 0, dist: 0 },       // quantum
        Instruction::QObserve { dst_h: 0, src_q: 0 }, // hybrid
        Instruction::ILdm { dst: 1, addr: 0 },        // cmem
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("int[64]"));
    assert!(output.contains("float[64]"));
    assert!(output.contains("qubit[16]"));
    assert!(output.contains("bit[16]"));
    assert!(output.contains("array[int[64], 65536] CMEM;"));
}

#[test]
fn test_qasm_body_order_matches_program_order() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::ILdi { dst: 1, imm: 2 },
        Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 },
    ];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);

    let pos0 = output.find("R0 = 1;").unwrap();
    let pos1 = output.find("R1 = 2;").unwrap();
    let pos2 = output.find("R2 = R0 + R1;").unwrap();
    assert!(pos0 < pos1);
    assert!(pos1 < pos2);
}
