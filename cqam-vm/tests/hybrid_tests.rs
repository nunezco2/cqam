use cqam_core::instruction::Instruction;
use cqam_core::register::CValue;
use cqam_vm::context::ExecutionContext;
use cqam_vm::hybrid::execute_hybrid;

#[test]
fn test_hyb_fork_sets_flags() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_hybrid(&mut ctx, Instruction::HybFork);
    assert!(ctx.psw.hf);
    assert!(ctx.psw.forked);
}

#[test]
fn test_hyb_merge_sets_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_hybrid(&mut ctx, Instruction::HybMerge);
    assert!(ctx.psw.merged);
}

#[test]
fn test_hyb_cond_exec_jump_on_qf() {
    let mut ctx = ExecutionContext::new(vec![
        Instruction::Label("THEN".into()),
        Instruction::ClLoad { dst: "R1".into(), src: "42".into() },
    ]);
    ctx.psw.qf = true;
    ctx.pc = 0;

    execute_hybrid(&mut ctx, Instruction::HybCondExec {
        flag: "QF".into(),
        then_label: "THEN".into(),
    });

    assert_eq!(ctx.pc, 1); // Already jumped to THEN
}

#[test]
fn test_hyb_reduce_all_modes() {
    let inputs = vec![
        ("round", 3),
        ("floor", 2),
        ("trunc", 2),
        ("ceil", 3),
        ("abs", 2),
        ("negate", -2),
    ];

    for (func, expected) in inputs {
        let mut ctx = ExecutionContext::new(vec![]);
        ctx.registers.store_c("hybX", CValue::Float(2.7));
        let instr = Instruction::HybReduce {
            src: "hybX".into(),
            dst: "outX".into(),
            function: func.into(),
        };
        execute_hybrid(&mut ctx, instr);
        assert_eq!(ctx.registers.load_c("outX"), Some(&CValue::Int(expected)));
    }
}
