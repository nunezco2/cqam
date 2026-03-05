//! Tests for the `Instruction` enum, its named constant sub-modules,
//! and the helper name-lookup functions.

use cqam_core::instruction::*;

#[test]
fn test_iadd_debug_format() {
    let instr = Instruction::IAdd { dst: 3, lhs: 1, rhs: 2 };
    assert_eq!(
        format!("{:?}", instr),
        "IAdd { dst: 3, lhs: 1, rhs: 2 }"
    );
}

#[test]
fn test_ildi_construction() {
    let instr = Instruction::ILdi { dst: 0, imm: -42 };
    assert_eq!(instr, Instruction::ILdi { dst: 0, imm: -42 });
}

#[test]
fn test_instruction_clone_and_eq() {
    let instr = Instruction::QKernel {
        dst: 1, src: 0, kernel: kernel_id::ENTANGLE, ctx0: 2, ctx1: 3,
    };
    let cloned = instr.clone();
    assert_eq!(instr, cloned);
}

#[test]
fn test_label_variant() {
    let label = Instruction::Label("LOOP".to_string());
    assert!(matches!(label, Instruction::Label(ref s) if s == "LOOP"));
}

#[test]
fn test_nop_variant() {
    let nop = Instruction::Nop;
    assert_eq!(nop, Instruction::Nop);
}

#[test]
fn test_all_integer_arithmetic_variants() {
    let _add = Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 };
    let _sub = Instruction::ISub { dst: 0, lhs: 1, rhs: 2 };
    let _mul = Instruction::IMul { dst: 0, lhs: 1, rhs: 2 };
    let _div = Instruction::IDiv { dst: 0, lhs: 1, rhs: 2 };
    let _modv = Instruction::IMod { dst: 0, lhs: 1, rhs: 2 };
}

#[test]
fn test_all_integer_bitwise_variants() {
    let _and = Instruction::IAnd { dst: 0, lhs: 1, rhs: 2 };
    let _or = Instruction::IOr { dst: 0, lhs: 1, rhs: 2 };
    let _xor = Instruction::IXor { dst: 0, lhs: 1, rhs: 2 };
    let _not = Instruction::INot { dst: 0, src: 1 };
    let _shl = Instruction::IShl { dst: 0, src: 1, amt: 4 };
    let _shr = Instruction::IShr { dst: 0, src: 1, amt: 4 };
}

#[test]
fn test_all_integer_memory_variants() {
    let _ldi = Instruction::ILdi { dst: 0, imm: 42 };
    let _ldm = Instruction::ILdm { dst: 0, addr: 1000 };
    let _str = Instruction::IStr { src: 0, addr: 1000 };
}

#[test]
fn test_all_integer_comparison_variants() {
    let _eq = Instruction::IEq { dst: 0, lhs: 1, rhs: 2 };
    let _lt = Instruction::ILt { dst: 0, lhs: 1, rhs: 2 };
    let _gt = Instruction::IGt { dst: 0, lhs: 1, rhs: 2 };
}

#[test]
fn test_all_float_variants() {
    let _add = Instruction::FAdd { dst: 0, lhs: 1, rhs: 2 };
    let _sub = Instruction::FSub { dst: 0, lhs: 1, rhs: 2 };
    let _mul = Instruction::FMul { dst: 0, lhs: 1, rhs: 2 };
    let _div = Instruction::FDiv { dst: 0, lhs: 1, rhs: 2 };
    let _ldi = Instruction::FLdi { dst: 0, imm: 314 };
    let _ldm = Instruction::FLdm { dst: 0, addr: 100 };
    let _str = Instruction::FStr { src: 0, addr: 100 };
    let _eq = Instruction::FEq { dst: 0, lhs: 1, rhs: 2 };
    let _lt = Instruction::FLt { dst: 0, lhs: 1, rhs: 2 };
    let _gt = Instruction::FGt { dst: 0, lhs: 1, rhs: 2 };
}

#[test]
fn test_all_complex_variants() {
    let _add = Instruction::ZAdd { dst: 0, lhs: 1, rhs: 2 };
    let _sub = Instruction::ZSub { dst: 0, lhs: 1, rhs: 2 };
    let _mul = Instruction::ZMul { dst: 0, lhs: 1, rhs: 2 };
    let _div = Instruction::ZDiv { dst: 0, lhs: 1, rhs: 2 };
    let _ldi = Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: -1 };
    let _ldm = Instruction::ZLdm { dst: 0, addr: 200 };
    let _str = Instruction::ZStr { src: 0, addr: 200 };
}

#[test]
fn test_all_conversion_variants() {
    let _if_ = Instruction::CvtIF { dst_f: 0, src_i: 1 };
    let _fi = Instruction::CvtFI { dst_i: 0, src_f: 1 };
    let _fz = Instruction::CvtFZ { dst_z: 0, src_f: 1 };
    let _zf = Instruction::CvtZF { dst_f: 0, src_z: 1 };
}

#[test]
fn test_all_control_flow_variants() {
    let _jmp = Instruction::Jmp { target: "END".into() };
    let _jif = Instruction::Jif { pred: 0, target: "THEN".into() };
    let _call = Instruction::Call { target: "FUNC".into() };
    let _ret = Instruction::Ret;
    let _halt = Instruction::Halt;
}

#[test]
fn test_all_quantum_variants() {
    let _prep = Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM };
    let _kernel = Instruction::QKernel { dst: 1, src: 0, kernel: kernel_id::ENTANGLE, ctx0: 2, ctx1: 3 };
    let _observe = Instruction::QObserve { dst_h: 0, src_q: 1 };
    let _load = Instruction::QLoad { dst_q: 2, addr: 10 };
    let _store = Instruction::QStore { src_q: 2, addr: 10 };
}

#[test]
fn test_all_hybrid_variants() {
    let _fork = Instruction::HFork;
    let _merge = Instruction::HMerge;
    let _cexec = Instruction::HCExec { flag: flag_id::QF, target: "THEN".into() };
    let _reduce = Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ROUND };
}

#[test]
fn test_qobserve_replaces_qmeas() {
    // QObserve is the sole measurement instruction.
    let observe = Instruction::QObserve { dst_h: 0, src_q: 1 };
    assert!(matches!(observe, Instruction::QObserve { dst_h: 0, src_q: 1 }));
}

#[test]
fn test_dist_name_lookup() {
    assert_eq!(dist_name(dist_id::UNIFORM), "uniform");
    assert_eq!(dist_name(dist_id::ZERO), "zero");
    assert_eq!(dist_name(dist_id::BELL), "bell");
    assert_eq!(dist_name(dist_id::GHZ), "ghz");
    assert_eq!(dist_name(255), "unknown");
}

#[test]
fn test_kernel_name_lookup() {
    assert_eq!(kernel_name(kernel_id::INIT), "init");
    assert_eq!(kernel_name(kernel_id::ENTANGLE), "entangle");
    assert_eq!(kernel_name(kernel_id::FOURIER), "fourier");
    assert_eq!(kernel_name(kernel_id::DIFFUSE), "diffuse");
    assert_eq!(kernel_name(kernel_id::GROVER_ITER), "grover_iter");
    assert_eq!(kernel_name(255), "unknown");
}

#[test]
fn test_flag_name_lookup() {
    assert_eq!(flag_name(flag_id::ZF), "ZF");
    assert_eq!(flag_name(flag_id::NF), "NF");
    assert_eq!(flag_name(flag_id::OF), "OF");
    assert_eq!(flag_name(flag_id::PF), "PF");
    assert_eq!(flag_name(flag_id::QF), "QF");
    assert_eq!(flag_name(flag_id::SF), "SF");
    assert_eq!(flag_name(flag_id::EF), "EF");
    assert_eq!(flag_name(flag_id::HF), "HF");
    assert_eq!(flag_name(255), "unknown");
}

#[test]
fn test_reduce_fn_name_lookup() {
    assert_eq!(reduce_fn_name(reduce_fn::ROUND), "round");
    assert_eq!(reduce_fn_name(reduce_fn::FLOOR), "floor");
    assert_eq!(reduce_fn_name(reduce_fn::CEIL), "ceil");
    assert_eq!(reduce_fn_name(reduce_fn::TRUNC), "trunc");
    assert_eq!(reduce_fn_name(reduce_fn::ABS), "abs");
    assert_eq!(reduce_fn_name(reduce_fn::NEGATE), "negate");
    assert_eq!(reduce_fn_name(reduce_fn::MAGNITUDE), "magnitude");
    assert_eq!(reduce_fn_name(reduce_fn::PHASE), "phase");
    assert_eq!(reduce_fn_name(reduce_fn::REAL), "real");
    assert_eq!(reduce_fn_name(reduce_fn::IMAG), "imag");
    assert_eq!(reduce_fn_name(reduce_fn::MEAN), "mean");
    assert_eq!(reduce_fn_name(reduce_fn::MODE), "mode");
    assert_eq!(reduce_fn_name(reduce_fn::ARGMAX), "argmax");
    assert_eq!(reduce_fn_name(reduce_fn::VARIANCE), "variance");
    assert_eq!(reduce_fn_name(255), "unknown");
}

#[test]
fn test_float_comparison_variants() {
    // FEq/FLt/FGt dst is an integer register index (result is boolean as i64)
    let feq = Instruction::FEq { dst: 5, lhs: 0, rhs: 1 };
    assert!(matches!(feq, Instruction::FEq { dst: 5, .. }));
}

#[test]
fn test_complex_instruction_variants() {
    let zldi = Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: -1 };
    assert!(matches!(zldi, Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: -1 }));
}

#[test]
fn test_qload_qstore_variants() {
    let qload = Instruction::QLoad { dst_q: 2, addr: 10 };
    let qstore = Instruction::QStore { src_q: 2, addr: 10 };
    assert!(matches!(qload, Instruction::QLoad { dst_q: 2, addr: 10 }));
    assert!(matches!(qstore, Instruction::QStore { src_q: 2, addr: 10 }));
}

#[test]
fn test_dist_id_constants() {
    assert_eq!(dist_id::UNIFORM, 0);
    assert_eq!(dist_id::ZERO, 1);
    assert_eq!(dist_id::BELL, 2);
    assert_eq!(dist_id::GHZ, 3);
}

#[test]
fn test_kernel_id_constants() {
    assert_eq!(kernel_id::INIT, 0);
    assert_eq!(kernel_id::ENTANGLE, 1);
    assert_eq!(kernel_id::FOURIER, 2);
    assert_eq!(kernel_id::DIFFUSE, 3);
    assert_eq!(kernel_id::GROVER_ITER, 4);
}

#[test]
fn test_flag_id_constants() {
    assert_eq!(flag_id::ZF, 0);
    assert_eq!(flag_id::NF, 1);
    assert_eq!(flag_id::OF, 2);
    assert_eq!(flag_id::PF, 3);
    assert_eq!(flag_id::QF, 4);
    assert_eq!(flag_id::SF, 5);
    assert_eq!(flag_id::EF, 6);
    assert_eq!(flag_id::HF, 7);
}

#[test]
fn test_reduce_fn_constants() {
    assert_eq!(reduce_fn::ROUND, 0);
    assert_eq!(reduce_fn::FLOOR, 1);
    assert_eq!(reduce_fn::CEIL, 2);
    assert_eq!(reduce_fn::TRUNC, 3);
    assert_eq!(reduce_fn::ABS, 4);
    assert_eq!(reduce_fn::NEGATE, 5);
    assert_eq!(reduce_fn::MAGNITUDE, 6);
    assert_eq!(reduce_fn::PHASE, 7);
    assert_eq!(reduce_fn::REAL, 8);
    assert_eq!(reduce_fn::IMAG, 9);
    assert_eq!(reduce_fn::MEAN, 10);
    assert_eq!(reduce_fn::MODE, 11);
    assert_eq!(reduce_fn::ARGMAX, 12);
    assert_eq!(reduce_fn::VARIANCE, 13);
}

#[test]
fn test_instruction_inequality() {
    let a = Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 };
    let b = Instruction::IAdd { dst: 0, lhs: 1, rhs: 3 };
    assert_ne!(a, b);
}

#[test]
fn test_instruction_different_variants_not_equal() {
    let a = Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 };
    let b = Instruction::ISub { dst: 0, lhs: 1, rhs: 2 };
    assert_ne!(a, b);
}
