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
        dst: 1, src: 0, kernel: KernelId::Entangle, ctx0: 2, ctx1: 3,
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
fn test_iqcfg_variant() {
    let cfg = Instruction::IQCfg { dst: 5 };
    match cfg {
        Instruction::IQCfg { dst } => assert_eq!(dst, 5),
        _ => panic!("Expected IQCfg variant"),
    }
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
    let _prep = Instruction::QPrep { dst: 0, dist: DistId::Uniform };
    let _kernel = Instruction::QKernel { dst: 1, src: 0, kernel: KernelId::Entangle, ctx0: 2, ctx1: 3 };
    let _observe = Instruction::QObserve { dst_h: 0, src_q: 1, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 };
    let _load = Instruction::QLoad { dst_q: 2, addr: 10 };
    let _store = Instruction::QStore { src_q: 2, addr: 10 };
}

#[test]
fn test_all_hybrid_variants() {
    let _fork = Instruction::HFork;
    let _merge = Instruction::HMerge;
    let _cexec = Instruction::JmpF { flag: FlagId::Qf, target: "THEN".into() };
    let _reduce = Instruction::HReduce { src: 0, dst: 1, func: ReduceFn::Round };
}

#[test]
fn test_qobserve_replaces_qmeas() {
    // QObserve is the sole measurement instruction.
    let observe = Instruction::QObserve { dst_h: 0, src_q: 1, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 };
    assert!(matches!(observe, Instruction::QObserve { dst_h: 0, src_q: 1, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 }));
}

#[test]
fn test_dist_name_lookup() {
    assert_eq!(DistId::Uniform.name(), "uniform");
    assert_eq!(DistId::Zero.name(), "zero");
    assert_eq!(DistId::Bell.name(), "bell");
    assert_eq!(DistId::Ghz.name(), "ghz");
}

#[test]
fn test_kernel_name_lookup() {
    assert_eq!(KernelId::Init.name(), "init");
    assert_eq!(KernelId::Entangle.name(), "entangle");
    assert_eq!(KernelId::Fourier.name(), "fourier");
    assert_eq!(KernelId::Diffuse.name(), "diffuse");
    assert_eq!(KernelId::GroverIter.name(), "grover_iter");
}

#[test]
fn test_flag_name_lookup() {
    assert_eq!(FlagId::Zf.mnemonic(), "ZF");
    assert_eq!(FlagId::Nf.mnemonic(), "NF");
    assert_eq!(FlagId::Of.mnemonic(), "OF");
    assert_eq!(FlagId::Pf.mnemonic(), "PF");
    assert_eq!(FlagId::Qf.mnemonic(), "QF");
    assert_eq!(FlagId::Sf.mnemonic(), "SF");
    assert_eq!(FlagId::Ef.mnemonic(), "EF");
    assert_eq!(FlagId::Hf.mnemonic(), "HF");
}

#[test]
fn test_reduce_fn_name_lookup() {
    assert_eq!(ReduceFn::Round.name(), "round");
    assert_eq!(ReduceFn::Floor.name(), "floor");
    assert_eq!(ReduceFn::Ceil.name(), "ceil");
    assert_eq!(ReduceFn::Trunc.name(), "trunc");
    assert_eq!(ReduceFn::Abs.name(), "abs");
    assert_eq!(ReduceFn::Negate.name(), "negate");
    assert_eq!(ReduceFn::Magnitude.name(), "magnitude");
    assert_eq!(ReduceFn::Phase.name(), "phase");
    assert_eq!(ReduceFn::Real.name(), "real");
    assert_eq!(ReduceFn::Imag.name(), "imag");
    assert_eq!(ReduceFn::Mean.name(), "mean");
    assert_eq!(ReduceFn::Mode.name(), "mode");
    assert_eq!(ReduceFn::Argmax.name(), "argmax");
    assert_eq!(ReduceFn::Variance.name(), "variance");
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
    assert_eq!(u8::from(DistId::Uniform), 0);
    assert_eq!(u8::from(DistId::Zero), 1);
    assert_eq!(u8::from(DistId::Bell), 2);
    assert_eq!(u8::from(DistId::Ghz), 3);
}

#[test]
fn test_kernel_id_constants() {
    assert_eq!(u8::from(KernelId::Init), 0);
    assert_eq!(u8::from(KernelId::Entangle), 1);
    assert_eq!(u8::from(KernelId::Fourier), 2);
    assert_eq!(u8::from(KernelId::Diffuse), 3);
    assert_eq!(u8::from(KernelId::GroverIter), 4);
}

#[test]
fn test_flag_id_constants() {
    assert_eq!(u8::from(FlagId::Zf), 0);
    assert_eq!(u8::from(FlagId::Nf), 1);
    assert_eq!(u8::from(FlagId::Of), 2);
    assert_eq!(u8::from(FlagId::Pf), 3);
    assert_eq!(u8::from(FlagId::Qf), 4);
    assert_eq!(u8::from(FlagId::Sf), 5);
    assert_eq!(u8::from(FlagId::Ef), 6);
    assert_eq!(u8::from(FlagId::Hf), 7);
}

#[test]
fn test_reduce_fn_constants() {
    assert_eq!(u8::from(ReduceFn::Round), 0);
    assert_eq!(u8::from(ReduceFn::Floor), 1);
    assert_eq!(u8::from(ReduceFn::Ceil), 2);
    assert_eq!(u8::from(ReduceFn::Trunc), 3);
    assert_eq!(u8::from(ReduceFn::Abs), 4);
    assert_eq!(u8::from(ReduceFn::Negate), 5);
    assert_eq!(u8::from(ReduceFn::Magnitude), 6);
    assert_eq!(u8::from(ReduceFn::Phase), 7);
    assert_eq!(u8::from(ReduceFn::Real), 8);
    assert_eq!(u8::from(ReduceFn::Imag), 9);
    assert_eq!(u8::from(ReduceFn::Mean), 10);
    assert_eq!(u8::from(ReduceFn::Mode), 11);
    assert_eq!(u8::from(ReduceFn::Argmax), 12);
    assert_eq!(u8::from(ReduceFn::Variance), 13);
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
