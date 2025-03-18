use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;

#[test]
fn test_execution_context_basics() {
    let program = vec![
        Instruction::ClLoad { dst: "X".into(), src: "42".into() },
        Instruction::ClAdd { dst: "Y".into(), lhs: "X".into(), rhs: "X".into() },
        Instruction::Label("LOOP_START".into()),
        Instruction::ClJump { label: "LOOP_START".into() },
    ];

    let ctx = ExecutionContext::new(program.clone());

    assert_eq!(ctx.pc, 0);
    assert_eq!(ctx.program.len(), 4);
    assert_eq!(ctx.program[0], program[0]);
    assert_eq!(ctx.program[2], Instruction::Label("LOOP_START".into()));
    assert_eq!(ctx.program[3], Instruction::ClJump { label: "LOOP_START".into() });
}

#[test]
fn test_execution_context_pc_reset_and_advance() {
    let program = vec![Instruction::ClLoad { dst: "A".into(), src: "1".into() }];
    let mut ctx = ExecutionContext::new(program);

    assert_eq!(ctx.pc, 0);
    ctx.advance_pc();
    assert_eq!(ctx.pc, 1);
    ctx.reset_pc();
    assert_eq!(ctx.pc, 0);
}
