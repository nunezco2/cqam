use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;
use cqam_vm::qop::execute_qop;
use cqam_core::register::CValue;

#[test]
fn test_qprep_and_qmeas() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, Instruction::QPrep {
        dst: "q1".into(),
        dist_src: "init".into(),
    });

    execute_qop(&mut ctx, Instruction::QMeas {
        dst: "R1".into(),
        src: "q1".into(),
    });

    assert!(ctx.registers.load_c("R1").is_some());
}

#[test]
fn test_qkernel_entangle_and_observe() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, Instruction::QPrep {
        dst: "q2".into(),
        dist_src: "init".into(),
    });

    execute_qop(&mut ctx, Instruction::QKernel {
        dst: "q3".into(),
        src: "q2".into(),
        kernel: "entangle".into(),
        ctx: None,
    });

    execute_qop(&mut ctx, Instruction::QObserve {
        dst: "avg".into(),
        src: "q3".into(),
    });

    assert!(matches!(ctx.registers.load_c("avg"), Some(CValue::Float(_))));
}
