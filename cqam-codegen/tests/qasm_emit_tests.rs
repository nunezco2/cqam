//! Tests for the QASM code generator: instruction-level emit correctness.

use cqam_core::instruction::*;
use cqam_codegen::qasm::{
    EmitConfig, QasmFormat, UsedRegisters,
    emit_declarations, emit_qasm_program, scan_registers,
};

// ===========================================================================
// Section 1: scan_registers tests
// ===========================================================================

#[test]
fn test_scan_empty_program() {
    let program: Vec<Instruction> = vec![];
    let used = scan_registers(&program);
    assert!(used.int_regs.is_empty());
    assert!(used.float_regs.is_empty());
    assert!(used.complex_regs.is_empty());
    assert!(used.quantum_regs.is_empty());
    assert!(used.hybrid_regs.is_empty());
    assert!(!used.uses_cmem);
    assert!(!used.uses_qmem);
    assert!(used.kernel_ids.is_empty());
    assert!(used.labels.is_empty());
}

#[test]
fn test_scan_int_regs() {
    let program = vec![
        Instruction::IAdd { dst: 3, lhs: 1, rhs: 2 },
    ];
    let used = scan_registers(&program);
    assert!(used.int_regs.contains(&1));
    assert!(used.int_regs.contains(&2));
    assert!(used.int_regs.contains(&3));
    assert_eq!(used.int_regs.len(), 3);
}

#[test]
fn test_scan_int_regs_deduplicated() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::IAdd { dst: 1, lhs: 0, rhs: 0 },
    ];
    let used = scan_registers(&program);
    assert_eq!(used.int_regs.len(), 2);
    assert!(used.int_regs.contains(&0));
    assert!(used.int_regs.contains(&1));
}

#[test]
fn test_scan_float_regs() {
    let program = vec![
        Instruction::FAdd { dst: 2, lhs: 0, rhs: 1 },
    ];
    let used = scan_registers(&program);
    assert!(used.float_regs.contains(&0));
    assert!(used.float_regs.contains(&1));
    assert!(used.float_regs.contains(&2));
    assert_eq!(used.float_regs.len(), 3);
}

#[test]
fn test_scan_complex_regs() {
    let program = vec![
        Instruction::ZAdd { dst: 3, lhs: 1, rhs: 2 },
    ];
    let used = scan_registers(&program);
    assert!(used.complex_regs.contains(&1));
    assert!(used.complex_regs.contains(&2));
    assert!(used.complex_regs.contains(&3));
    assert_eq!(used.complex_regs.len(), 3);
}

#[test]
fn test_scan_quantum_regs() {
    let program = vec![
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        Instruction::QKernel { dst: 1, src: 0, kernel: kernel_id::FOURIER, ctx0: 2, ctx1: 3 },
        Instruction::QObserve { dst_h: 0, src_q: 1, mode: 0, ctx0: 0, ctx1: 0 },
    ];
    let used = scan_registers(&program);
    assert!(used.quantum_regs.contains(&0));
    assert!(used.quantum_regs.contains(&1));
    assert_eq!(used.quantum_regs.len(), 2);
}

#[test]
fn test_scan_hybrid_regs() {
    let program = vec![
        Instruction::QObserve { dst_h: 0, src_q: 1, mode: 0, ctx0: 0, ctx1: 0 },
    ];
    let used = scan_registers(&program);
    assert!(used.hybrid_regs.contains(&0));
    assert_eq!(used.hybrid_regs.len(), 1);
}

#[test]
fn test_scan_cmem_flag() {
    let program_with = vec![
        Instruction::ILdm { dst: 0, addr: 100 },
    ];
    let program_without = vec![
        Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 },
    ];
    let used_with = scan_registers(&program_with);
    let used_without = scan_registers(&program_without);
    assert!(used_with.uses_cmem);
    assert!(!used_without.uses_cmem);
}

#[test]
fn test_scan_cmem_flag_from_fldm() {
    let program = vec![
        Instruction::FLdm { dst: 0, addr: 100 },
    ];
    let used = scan_registers(&program);
    assert!(used.uses_cmem);
}

#[test]
fn test_scan_cmem_flag_from_zstr() {
    let program = vec![
        Instruction::ZStr { src: 0, addr: 100 },
    ];
    let used = scan_registers(&program);
    assert!(used.uses_cmem);
}

#[test]
fn test_scan_qmem_flag() {
    let program = vec![
        Instruction::QLoad { dst_q: 0, addr: 10 },
    ];
    let used = scan_registers(&program);
    assert!(used.uses_qmem);
}

#[test]
fn test_scan_qmem_flag_from_qstore() {
    let program = vec![
        Instruction::QStore { src_q: 0, addr: 10 },
    ];
    let used = scan_registers(&program);
    assert!(used.uses_qmem);
}

#[test]
fn test_scan_kernel_ids() {
    let program = vec![
        Instruction::QKernel { dst: 0, src: 1, kernel: kernel_id::ENTANGLE, ctx0: 0, ctx1: 0 },
        Instruction::QKernel { dst: 2, src: 3, kernel: kernel_id::FOURIER, ctx0: 0, ctx1: 0 },
    ];
    let used = scan_registers(&program);
    assert!(used.kernel_ids.contains(&kernel_id::ENTANGLE));
    assert!(used.kernel_ids.contains(&kernel_id::FOURIER));
    assert_eq!(used.kernel_ids.len(), 2);
}

// -- Register-indirect scan tests --

#[test]
fn test_scan_ildx_regs() {
    let program = vec![
        Instruction::ILdx { dst: 0, addr_reg: 3 },
    ];
    let used = scan_registers(&program);
    assert!(used.int_regs.contains(&0));
    assert!(used.int_regs.contains(&3));
    assert!(used.uses_cmem);
}

#[test]
fn test_scan_istrx_regs() {
    let program = vec![
        Instruction::IStrx { src: 5, addr_reg: 2 },
    ];
    let used = scan_registers(&program);
    assert!(used.int_regs.contains(&5));
    assert!(used.int_regs.contains(&2));
    assert!(used.uses_cmem);
}

#[test]
fn test_scan_fldx_regs() {
    let program = vec![
        Instruction::FLdx { dst: 1, addr_reg: 4 },
    ];
    let used = scan_registers(&program);
    assert!(used.float_regs.contains(&1));
    assert!(used.int_regs.contains(&4));
    assert!(used.uses_cmem);
}

#[test]
fn test_scan_fstrx_regs() {
    let program = vec![
        Instruction::FStrx { src: 7, addr_reg: 6 },
    ];
    let used = scan_registers(&program);
    assert!(used.float_regs.contains(&7));
    assert!(used.int_regs.contains(&6));
    assert!(used.uses_cmem);
}

#[test]
fn test_scan_zldx_regs() {
    let program = vec![
        Instruction::ZLdx { dst: 2, addr_reg: 8 },
    ];
    let used = scan_registers(&program);
    assert!(used.complex_regs.contains(&2));
    assert!(used.int_regs.contains(&8));
    assert!(used.uses_cmem);
}

#[test]
fn test_scan_zstrx_regs() {
    let program = vec![
        Instruction::ZStrx { src: 3, addr_reg: 9 },
    ];
    let used = scan_registers(&program);
    assert!(used.complex_regs.contains(&3));
    assert!(used.int_regs.contains(&9));
    assert!(used.uses_cmem);
}

#[test]
fn test_scan_indirect_no_qmem() {
    // Indirect memory uses CMEM, not QMEM
    let program = vec![
        Instruction::ILdx { dst: 0, addr_reg: 1 },
        Instruction::FStrx { src: 2, addr_reg: 3 },
        Instruction::ZLdx { dst: 4, addr_reg: 5 },
    ];
    let used = scan_registers(&program);
    assert!(used.uses_cmem);
    assert!(!used.uses_qmem);
}

#[test]
fn test_scan_cross_file_ops() {
    let program = vec![
        Instruction::FEq { dst: 5, lhs: 0, rhs: 1 },
    ];
    let used = scan_registers(&program);
    assert!(used.int_regs.contains(&5));
    assert!(used.float_regs.contains(&0));
    assert!(used.float_regs.contains(&1));
}

#[test]
fn test_scan_hreduce_int_target() {
    let program = vec![
        Instruction::HReduce { src: 0, dst: 3, func: reduce_fn::ROUND },
    ];
    let used = scan_registers(&program);
    assert!(used.hybrid_regs.contains(&0));
    assert!(used.int_regs.contains(&3));
}

#[test]
fn test_scan_hreduce_float_target() {
    let program = vec![
        Instruction::HReduce { src: 1, dst: 4, func: reduce_fn::MAGNITUDE },
    ];
    let used = scan_registers(&program);
    assert!(used.hybrid_regs.contains(&1));
    assert!(used.float_regs.contains(&4));
}

#[test]
fn test_scan_conversion_ops() {
    let program = vec![
        Instruction::CvtIF { dst_f: 0, src_i: 1 },
    ];
    let used = scan_registers(&program);
    assert!(used.int_regs.contains(&1));
    assert!(used.float_regs.contains(&0));
}

#[test]
fn test_scan_conversion_fi() {
    let program = vec![
        Instruction::CvtFI { dst_i: 2, src_f: 3 },
    ];
    let used = scan_registers(&program);
    assert!(used.int_regs.contains(&2));
    assert!(used.float_regs.contains(&3));
}

#[test]
fn test_scan_conversion_fz() {
    let program = vec![
        Instruction::CvtFZ { dst_z: 0, src_f: 1 },
    ];
    let used = scan_registers(&program);
    assert!(used.complex_regs.contains(&0));
    assert!(used.float_regs.contains(&1));
}

#[test]
fn test_scan_conversion_zf() {
    let program = vec![
        Instruction::CvtZF { dst_f: 2, src_z: 3 },
    ];
    let used = scan_registers(&program);
    assert!(used.float_regs.contains(&2));
    assert!(used.complex_regs.contains(&3));
}

#[test]
fn test_scan_labels() {
    let program = vec![
        Instruction::Label("START".into()),
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Label("END".into()),
    ];
    let used = scan_registers(&program);
    assert_eq!(used.labels, vec!["START".to_string(), "END".to_string()]);
}

#[test]
fn test_scan_jif_registers_pred() {
    let program = vec![
        Instruction::Jif { pred: 5, target: "LBL".into() },
    ];
    let used = scan_registers(&program);
    assert!(used.int_regs.contains(&5));
}

#[test]
fn test_scan_qkernel_registers_ctx() {
    let program = vec![
        Instruction::QKernel { dst: 0, src: 1, kernel: kernel_id::FOURIER, ctx0: 7, ctx1: 8 },
    ];
    let used = scan_registers(&program);
    assert!(used.int_regs.contains(&7));
    assert!(used.int_regs.contains(&8));
    assert!(used.quantum_regs.contains(&0));
    assert!(used.quantum_regs.contains(&1));
}

// ===========================================================================
// Section 2: emit_declarations tests
// ===========================================================================

#[test]
fn test_decl_int_regs() {
    let mut used = UsedRegisters::default();
    used.int_regs.insert(0);
    used.int_regs.insert(3);
    let decls = emit_declarations(&used);
    assert!(decls.contains("int[64] R0;"));
    assert!(decls.contains("int[64] R3;"));
}

#[test]
fn test_decl_float_regs() {
    let mut used = UsedRegisters::default();
    used.float_regs.insert(0);
    used.float_regs.insert(2);
    let decls = emit_declarations(&used);
    assert!(decls.contains("float[64] F0;"));
    assert!(decls.contains("float[64] F2;"));
}

#[test]
fn test_decl_complex_regs() {
    let mut used = UsedRegisters::default();
    used.complex_regs.insert(1);
    let decls = emit_declarations(&used);
    assert!(decls.contains("float[64] Z1_re;"));
    assert!(decls.contains("float[64] Z1_im;"));
}

#[test]
fn test_decl_quantum_regs() {
    let mut used = UsedRegisters::default();
    used.quantum_regs.insert(0);
    used.quantum_regs.insert(1);
    let decls = emit_declarations(&used);
    assert!(decls.contains("qubit[16] q0;"));
    assert!(decls.contains("qubit[16] q1;"));
}

#[test]
fn test_decl_hybrid_regs() {
    let mut used = UsedRegisters::default();
    used.hybrid_regs.insert(0);
    let decls = emit_declarations(&used);
    assert!(decls.contains("bit[16] H0;"));
}

#[test]
fn test_decl_cmem() {
    let used = UsedRegisters { uses_cmem: true, ..Default::default() };
    let decls = emit_declarations(&used);
    assert!(decls.contains("// @cqam.cmem: classical memory (65536 x int[64]) -- no QASM equivalent"));
}

#[test]
fn test_decl_no_cmem() {
    let used = UsedRegisters::default();
    let decls = emit_declarations(&used);
    assert!(!decls.contains("CMEM"));
}

#[test]
fn test_decl_empty() {
    let used = UsedRegisters::default();
    let decls = emit_declarations(&used);
    assert!(decls.is_empty());
}

#[test]
fn test_decl_ordering() {
    // Declarations should be emitted in order: int, float, complex, quantum, hybrid, cmem
    let mut used = UsedRegisters::default();
    used.int_regs.insert(0);
    used.float_regs.insert(0);
    used.complex_regs.insert(0);
    used.quantum_regs.insert(0);
    used.hybrid_regs.insert(0);
    used.uses_cmem = true;
    let decls = emit_declarations(&used);

    let int_pos = decls.find("int[64] R0;").unwrap();
    let float_pos = decls.find("float[64] F0;").unwrap();
    let complex_pos = decls.find("float[64] Z0_re;").unwrap();
    let quantum_pos = decls.find("qubit[16] q0;").unwrap();
    let hybrid_pos = decls.find("bit[16] H0;").unwrap();
    let cmem_pos = decls.find("@cqam.cmem").unwrap();

    assert!(int_pos < float_pos);
    assert!(float_pos < complex_pos);
    assert!(complex_pos < quantum_pos);
    assert!(quantum_pos < hybrid_pos);
    assert!(hybrid_pos < cmem_pos);
}

// ===========================================================================
// Section 3: Individual instruction emission tests
// ===========================================================================

fn fragment_config() -> EmitConfig {
    EmitConfig::fragment()
}

// -- Integer arithmetic --

#[test]
fn test_emit_iadd() {
    let instr = Instruction::IAdd { dst: 3, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "R3 = R1 + R2;");
}

#[test]
fn test_emit_iadd_no_type_prefix() {
    let instr = Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert!(!lines[0].contains("int[64]"));
}

#[test]
fn test_emit_isub() {
    let instr = Instruction::ISub { dst: 4, lhs: 3, rhs: 1 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R4 = R3 - R1;");
}

#[test]
fn test_emit_imul() {
    let instr = Instruction::IMul { dst: 5, lhs: 3, rhs: 4 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R5 = R3 * R4;");
}

#[test]
fn test_emit_idiv() {
    let instr = Instruction::IDiv { dst: 6, lhs: 5, rhs: 3 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R6 = R5 / R3;");
}

#[test]
fn test_emit_imod() {
    let instr = Instruction::IMod { dst: 7, lhs: 5, rhs: 4 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R7 = R5 % R4;");
}

#[test]
fn test_emit_iand() {
    let instr = Instruction::IAnd { dst: 0, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R0 = R1 & R2;");
}

#[test]
fn test_emit_ior() {
    let instr = Instruction::IOr { dst: 3, lhs: 4, rhs: 5 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R3 = R4 | R5;");
}

#[test]
fn test_emit_ixor() {
    let instr = Instruction::IXor { dst: 6, lhs: 7, rhs: 8 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R6 = R7 ^ R8;");
}

#[test]
fn test_emit_inot() {
    let instr = Instruction::INot { dst: 9, src: 10 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R9 = R10 ^ -1;");
}

#[test]
fn test_emit_ishl() {
    let instr = Instruction::IShl { dst: 11, src: 12, amt: 4 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R11 = R12 << 4;");
}

#[test]
fn test_emit_ishr() {
    let instr = Instruction::IShr { dst: 13, src: 14, amt: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R13 = R14 >> 2;");
}

#[test]
fn test_emit_ildi() {
    let instr = Instruction::ILdi { dst: 0, imm: 42 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R0 = 42;");
}

#[test]
fn test_emit_ildi_negative() {
    let instr = Instruction::ILdi { dst: 5, imm: -100 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R5 = -100;");
}

#[test]
fn test_emit_ildm() {
    let instr = Instruction::ILdm { dst: 0, addr: 256 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "// @cqam.ldm R0, CMEM[256]");
}

#[test]
fn test_emit_istr() {
    let instr = Instruction::IStr { src: 3, addr: 100 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "// @cqam.str CMEM[100], R3");
}

#[test]
fn test_emit_ieq() {
    let instr = Instruction::IEq { dst: 0, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "if (R1 == R2) { R0 = 1; } else { R0 = 0; }");
}

#[test]
fn test_emit_ilt() {
    let instr = Instruction::ILt { dst: 3, lhs: 4, rhs: 5 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "if (R4 < R5) { R3 = 1; } else { R3 = 0; }");
}

#[test]
fn test_emit_igt() {
    let instr = Instruction::IGt { dst: 6, lhs: 7, rhs: 8 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "if (R7 > R8) { R6 = 1; } else { R6 = 0; }");
}

// -- Float arithmetic --

#[test]
fn test_emit_fadd() {
    let instr = Instruction::FAdd { dst: 2, lhs: 0, rhs: 1 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "F2 = F0 + F1;");
}

#[test]
fn test_emit_fsub() {
    let instr = Instruction::FSub { dst: 3, lhs: 2, rhs: 0 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "F3 = F2 - F0;");
}

#[test]
fn test_emit_fmul() {
    let instr = Instruction::FMul { dst: 4, lhs: 2, rhs: 3 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "F4 = F2 * F3;");
}

#[test]
fn test_emit_fdiv() {
    let instr = Instruction::FDiv { dst: 5, lhs: 4, rhs: 3 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "F5 = F4 / F3;");
}

#[test]
fn test_emit_fldi() {
    let instr = Instruction::FLdi { dst: 0, imm: 314 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "F0 = 314.0;");
}

#[test]
fn test_emit_fldm() {
    let instr = Instruction::FLdm { dst: 1, addr: 200 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "// @cqam.ldm F1, CMEM[200]");
}

#[test]
fn test_emit_fstr() {
    let instr = Instruction::FStr { src: 2, addr: 300 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "// @cqam.str CMEM[300], F2");
}

#[test]
fn test_emit_feq() {
    let instr = Instruction::FEq { dst: 0, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "if (F1 == F2) { R0 = 1; } else { R0 = 0; }");
}

#[test]
fn test_emit_flt() {
    let instr = Instruction::FLt { dst: 3, lhs: 4, rhs: 5 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "if (F4 < F5) { R3 = 1; } else { R3 = 0; }");
}

#[test]
fn test_emit_fgt() {
    let instr = Instruction::FGt { dst: 6, lhs: 7, rhs: 8 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "if (F7 > F8) { R6 = 1; } else { R6 = 0; }");
}

// -- Complex arithmetic (lowered to paired floats) --

#[test]
fn test_emit_zadd() {
    let instr = Instruction::ZAdd { dst: 0, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "Z0_re = Z1_re + Z2_re;");
    assert_eq!(lines[1], "Z0_im = Z1_im + Z2_im;");
}

#[test]
fn test_emit_zsub() {
    let instr = Instruction::ZSub { dst: 3, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "Z3_re = Z1_re - Z2_re;");
    assert_eq!(lines[1], "Z3_im = Z1_im - Z2_im;");
}

#[test]
fn test_emit_zmul() {
    let instr = Instruction::ZMul { dst: 0, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 5);
    assert!(lines[0].starts_with("// ZMUL:"));
    assert!(lines[1].contains("_tmp_re"));
    assert!(lines[1].contains("Z1_re * Z2_re"));
    assert!(lines[1].contains("Z1_im * Z2_im"));
    assert!(lines[2].contains("_tmp_im"));
    assert!(lines[2].contains("Z1_re * Z2_im"));
    assert!(lines[2].contains("Z1_im * Z2_re"));
    assert_eq!(lines[3], "Z0_re = _tmp_re;");
    assert_eq!(lines[4], "Z0_im = _tmp_im;");
}

#[test]
fn test_emit_zdiv() {
    let instr = Instruction::ZDiv { dst: 3, lhs: 1, rhs: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 6);
    assert!(lines[0].starts_with("// ZDIV:"));
    assert!(lines[1].contains("_denom"));
    assert!(lines[2].contains("_tmp_re"));
    assert!(lines[3].contains("_tmp_im"));
    assert_eq!(lines[4], "Z3_re = _tmp_re;");
    assert_eq!(lines[5], "Z3_im = _tmp_im;");
}

#[test]
fn test_emit_zldi() {
    let instr = Instruction::ZLdi { dst: 0, imm_re: 3, imm_im: -2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "Z0_re = 3.0;");
    assert_eq!(lines[1], "Z0_im = -2.0;");
}

#[test]
fn test_emit_zldm() {
    let instr = Instruction::ZLdm { dst: 1, addr: 100 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "// @cqam.ldm Z1_re, CMEM[100]");
    assert_eq!(lines[1], "// @cqam.ldm Z1_im, CMEM[101]");
}

#[test]
fn test_emit_zstr() {
    let instr = Instruction::ZStr { src: 2, addr: 200 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "// @cqam.str CMEM[200], Z2_re");
    assert_eq!(lines[1], "// @cqam.str CMEM[201], Z2_im");
}

// -- Register-indirect memory --

#[test]
fn test_emit_ildx() {
    let instr = Instruction::ILdx { dst: 0, addr_reg: 3 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "// @cqam.ldx R0, CMEM[R3]");
}

#[test]
fn test_emit_istrx() {
    let instr = Instruction::IStrx { src: 5, addr_reg: 2 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "// @cqam.strx CMEM[R2], R5");
}

#[test]
fn test_emit_fldx() {
    let instr = Instruction::FLdx { dst: 1, addr_reg: 4 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "// @cqam.ldx F1, CMEM[R4]");
}

#[test]
fn test_emit_fstrx() {
    let instr = Instruction::FStrx { src: 7, addr_reg: 6 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "// @cqam.strx CMEM[R6], F7");
}

#[test]
fn test_emit_zldx() {
    let instr = Instruction::ZLdx { dst: 2, addr_reg: 8 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "// @cqam.ldx Z2_re, CMEM[R8]");
    assert_eq!(lines[1], "// @cqam.ldx Z2_im, CMEM[R8 + 1]");
}

#[test]
fn test_emit_zstrx() {
    let instr = Instruction::ZStrx { src: 3, addr_reg: 9 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "// @cqam.strx CMEM[R9], Z3_re");
    assert_eq!(lines[1], "// @cqam.strx CMEM[R9 + 1], Z3_im");
}

// -- Type conversion --

#[test]
fn test_emit_cvtif() {
    let instr = Instruction::CvtIF { dst_f: 0, src_i: 1 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "F0 = float[64](R1);");
}

#[test]
fn test_emit_cvtfi() {
    let instr = Instruction::CvtFI { dst_i: 0, src_f: 1 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "R0 = int[64](F1);");
}

#[test]
fn test_emit_cvtfz() {
    let instr = Instruction::CvtFZ { dst_z: 0, src_f: 1 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "Z0_re = F1;");
    assert_eq!(lines[1], "Z0_im = 0.0;");
}

#[test]
fn test_emit_cvtzf() {
    let instr = Instruction::CvtZF { dst_f: 0, src_z: 1 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "F0 = Z1_re;");
}

// -- Control flow --

#[test]
fn test_emit_jmp() {
    let instr = Instruction::Jmp { target: "LOOP".into() };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "// @cqam.jmp LOOP");
}

#[test]
fn test_emit_jif() {
    let instr = Instruction::Jif { pred: 0, target: "THEN".into() };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "if (bool(R0)) { } // @cqam.branch THEN");
}

#[test]
fn test_emit_call() {
    let instr = Instruction::Call { target: "FUNC".into() };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("//"));
    assert!(lines[0].contains("CALL FUNC"));
}

#[test]
fn test_emit_ret() {
    let instr = Instruction::Ret;
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("//"));
    assert!(lines[0].contains("RET"));
}

#[test]
fn test_emit_halt() {
    let instr = Instruction::Halt;
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("HALT"));
}

// -- Quantum operations --

#[test]
fn test_emit_qprep() {
    let instr = Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM };
    let lines = instr.to_qasm(&fragment_config());
    assert!(lines.len() >= 2);
    assert_eq!(lines[0], "reset q0;");
    assert!(lines[1].contains("uniform"));
}

#[test]
fn test_emit_qprep_zero_dist() {
    let instr = Instruction::QPrep { dst: 1, dist: dist_id::ZERO };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "reset q1;");
    assert!(lines[1].contains("zero"));
}

#[test]
fn test_emit_qkernel_no_expand() {
    let config = EmitConfig {
        expand_templates: false,
        ..EmitConfig::default()
    };
    let instr = Instruction::QKernel {
        dst: 0, src: 1, kernel: kernel_id::FOURIER, ctx0: 2, ctx1: 3,
    };
    let lines = instr.to_qasm(&config);
    assert!(lines.len() >= 2);
    assert!(lines[0].contains("QKernel:"));
    assert!(lines[0].contains("fourier"));
    assert_eq!(lines[1], "fourier q0;");
}

#[test]
fn test_emit_qkernel_with_expand_missing_template() {
    let config = EmitConfig {
        expand_templates: true,
        template_dir: "/nonexistent/path".to_string(),
        ..EmitConfig::default()
    };
    let instr = Instruction::QKernel {
        dst: 0, src: 1, kernel: kernel_id::FOURIER, ctx0: 0, ctx1: 0,
    };
    let lines = instr.to_qasm(&config);
    assert!(lines.len() >= 2);
    assert!(lines[1].contains("[Missing QASM template for fourier]"));
}

#[test]
fn test_emit_qobserve() {
    let instr = Instruction::QObserve { dst_h: 0, src_q: 1, mode: 0, ctx0: 0, ctx1: 0 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines[0], "H0 = measure q1;");
}

#[test]
fn test_emit_qload() {
    let instr = Instruction::QLoad { dst_q: 2, addr: 10 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("//"));
    assert!(lines[0].contains("QLOAD q2 from QMEM[10]"));
}

#[test]
fn test_emit_qstore() {
    let instr = Instruction::QStore { src_q: 3, addr: 20 };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("//"));
    assert!(lines[0].contains("QSTORE q3 to QMEM[20]"));
}

// -- Hybrid operations --

#[test]
fn test_emit_hfork() {
    let instr = Instruction::HFork;
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("@cqam.hfork"));
}

#[test]
fn test_emit_hmerge() {
    let instr = Instruction::HMerge;
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("@cqam.hmerge"));
}

#[test]
fn test_emit_hcexec() {
    let instr = Instruction::HCExec { flag: flag_id::QF, target: "LBL".into() };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("@cqam.hcexec"));
    assert!(lines[0].contains("QF"));
    assert!(lines[0].contains("LBL"));
}

#[test]
fn test_emit_hreduce_int() {
    let instr = Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ROUND };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("@cqam.hreduce"));
    assert!(lines[0].contains("R1"));
    assert!(lines[0].contains("round"));
}

#[test]
fn test_emit_hreduce_float() {
    let instr = Instruction::HReduce { src: 0, dst: 2, func: reduce_fn::MAGNITUDE };
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("@cqam.hreduce"));
    assert!(lines[0].contains("F2"));
    assert!(lines[0].contains("magnitude"));
}

// -- Pseudo-instructions --

#[test]
fn test_emit_nop() {
    let instr = Instruction::Nop;
    let lines = instr.to_qasm(&fragment_config());
    assert!(lines.is_empty());
}

#[test]
fn test_emit_label() {
    let instr = Instruction::Label("LOOP".into());
    let lines = instr.to_qasm(&fragment_config());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "// @cqam.label LOOP");
}

// ===========================================================================
// Section 4: emit_qasm_program integration tests
// ===========================================================================

#[test]
fn test_standalone_has_header() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.starts_with("OPENQASM 3.0;"));
}

#[test]
fn test_standalone_has_include() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("include \"stdgates.inc\";"));
}

#[test]
fn test_standalone_has_declarations() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::IAdd { dst: 1, lhs: 0, rhs: 0 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("int[64] R0;"));
    assert!(output.contains("int[64] R1;"));
}

#[test]
fn test_standalone_declarations_before_body() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    let decl_pos = output.find("int[64] R0;").unwrap();
    let body_pos = output.find("R0 = 42;").unwrap();
    assert!(decl_pos < body_pos);
}

#[test]
fn test_standalone_no_repeated_declarations() {
    let program = vec![
        Instruction::ILdi { dst: 3, imm: 1 },
        Instruction::IAdd { dst: 3, lhs: 3, rhs: 3 },
        Instruction::IMul { dst: 3, lhs: 3, rhs: 3 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    let decl_count = output.matches("int[64] R3;").count();
    assert_eq!(decl_count, 1);
}

#[test]
fn test_fragment_no_header() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
    ];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);
    assert!(!output.contains("OPENQASM"));
    assert!(!output.contains("include"));
}

#[test]
fn test_fragment_no_declarations() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::FAdd { dst: 0, lhs: 1, rhs: 2 },
    ];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);
    assert!(!output.contains("int[64] R0;"));
    assert!(!output.contains("float[64]"));
}

#[test]
fn test_fragment_body_only() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::IAdd { dst: 1, lhs: 0, rhs: 0 },
    ];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("R0 = 42;"));
    assert!(output.contains("R1 = R0 + R0;"));
    assert!(!output.contains("OPENQASM"));
    assert!(!output.contains("==="));
}

#[test]
fn test_kernel_stubs_emitted() {
    let program = vec![
        Instruction::QKernel {
            dst: 0, src: 1, kernel: kernel_id::FOURIER, ctx0: 0, ctx1: 0,
        },
    ];
    let config = EmitConfig {
        expand_templates: false,
        ..EmitConfig::standalone()
    };
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("gate fourier q {"));
}

#[test]
fn test_kernel_stubs_deduplicated() {
    let program = vec![
        Instruction::QKernel {
            dst: 0, src: 1, kernel: kernel_id::ENTANGLE, ctx0: 0, ctx1: 0,
        },
        Instruction::QKernel {
            dst: 2, src: 3, kernel: kernel_id::ENTANGLE, ctx0: 0, ctx1: 0,
        },
    ];
    let config = EmitConfig {
        expand_templates: false,
        ..EmitConfig::standalone()
    };
    let output = emit_qasm_program(&program, &config);
    let gate_count = output.matches("gate entangle").count();
    assert_eq!(gate_count, 1);
}

#[test]
fn test_kernel_stubs_not_emitted_when_expanding() {
    let program = vec![
        Instruction::QKernel {
            dst: 0, src: 1, kernel: kernel_id::FOURIER, ctx0: 0, ctx1: 0,
        },
    ];
    let config = EmitConfig::standalone(); // expand_templates = true
    let output = emit_qasm_program(&program, &config);
    // Gate stubs should NOT be present when expanding templates
    assert!(!output.contains("gate fourier q {"));
}

#[test]
fn test_mixed_program() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 5 },
        Instruction::FLdi { dst: 0, imm: 314 },
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("int[64] R0;"));
    assert!(output.contains("float[64] F0;"));
    assert!(output.contains("qubit[16] q0;"));
    assert!(output.contains("bit[16] H0;"));
}

#[test]
fn test_empty_program() {
    let program: Vec<Instruction> = vec![];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("OPENQASM 3.0;"));
}

#[test]
fn test_complex_ops_produce_paired_floats() {
    let program = vec![
        Instruction::ZAdd { dst: 0, lhs: 1, rhs: 2 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("Z0_re ="));
    assert!(output.contains("Z0_im ="));
    assert!(output.contains("float[64] Z0_re;"));
    assert!(output.contains("float[64] Z0_im;"));
}

#[test]
fn test_label_emitted_in_body() {
    let program = vec![
        Instruction::Label("START".into()),
        Instruction::ILdi { dst: 0, imm: 1 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("// @cqam.label START"));
}

#[test]
fn test_standalone_has_footer() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    assert!(output.contains("// === End CQAM Generated QASM ==="));
}

#[test]
fn test_standalone_indirect_declarations() {
    let program = vec![
        Instruction::ILdx { dst: 0, addr_reg: 1 },
        Instruction::FStrx { src: 2, addr_reg: 3 },
        Instruction::ZLdx { dst: 4, addr_reg: 5 },
    ];
    let config = EmitConfig::standalone();
    let output = emit_qasm_program(&program, &config);
    // Should declare integer regs for addr_reg operands
    assert!(output.contains("int[64] R1;"));
    assert!(output.contains("int[64] R3;"));
    assert!(output.contains("int[64] R5;"));
    // Should declare data regs
    assert!(output.contains("int[64] R0;"));
    assert!(output.contains("float[64] F2;"));
    assert!(output.contains("float[64] Z4_re;"));
    assert!(output.contains("float[64] Z4_im;"));
    // Should declare CMEM
    assert!(output.contains("CMEM"));
    // Body should contain pragma comments for indirect memory access
    assert!(output.contains("@cqam.ldx"));
    assert!(output.contains("@cqam.strx"));
}

#[test]
fn test_fragment_no_footer() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
    ];
    let config = EmitConfig::fragment();
    let output = emit_qasm_program(&program, &config);
    assert!(!output.contains("End CQAM"));
}
