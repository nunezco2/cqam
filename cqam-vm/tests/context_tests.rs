// cqam-vm/tests/context_tests.rs
//
// Phase 2: Test the updated ExecutionContext with separate register files.

use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;

#[test]
fn test_execution_context_basics() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::IAdd { dst: 1, lhs: 0, rhs: 0 },
        Instruction::Label("LOOP_START".into()),
        Instruction::Jmp { target: "LOOP_START".into() },
    ];

    let ctx = ExecutionContext::new(program.clone());

    assert_eq!(ctx.pc, 0);
    assert_eq!(ctx.program.len(), 4);
    assert_eq!(ctx.program[0], program[0]);
    assert_eq!(ctx.program[2], Instruction::Label("LOOP_START".into()));
    assert_eq!(ctx.program[3], Instruction::Jmp { target: "LOOP_START".into() });
}

#[test]
fn test_execution_context_pc_reset_and_advance() {
    let program = vec![Instruction::ILdi { dst: 0, imm: 1 }];
    let mut ctx = ExecutionContext::new(program);

    assert_eq!(ctx.pc, 0);
    ctx.advance_pc();
    assert_eq!(ctx.pc, 1);
    ctx.reset_pc();
    assert_eq!(ctx.pc, 0);
}

#[test]
fn test_label_resolution_cache() {
    let program = vec![
        Instruction::Label("START".into()),
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Label("END".into()),
        Instruction::Halt,
    ];

    let ctx = ExecutionContext::new(program);

    assert_eq!(ctx.labels.get("START"), Some(&0));
    assert_eq!(ctx.labels.get("END"), Some(&2));
    assert_eq!(ctx.labels.get("NONEXISTENT"), None);
}

#[test]
fn test_jump_to_label() {
    let program = vec![
        Instruction::Label("TARGET".into()),
        Instruction::ILdi { dst: 0, imm: 1 },
    ];

    let mut ctx = ExecutionContext::new(program);
    ctx.pc = 1;
    assert!(ctx.jump_to_label("TARGET"));
    assert_eq!(ctx.pc, 0);
}

#[test]
fn test_jump_to_nonexistent_label_returns_false() {
    let program = vec![Instruction::Halt];
    let mut ctx = ExecutionContext::new(program);
    ctx.pc = 0;
    assert!(!ctx.jump_to_label("NONEXISTENT"));
    assert_eq!(ctx.pc, 0); // PC unchanged
}

#[test]
fn test_call_stack_push_and_pop() {
    let program = vec![];
    let mut ctx = ExecutionContext::new(program);

    ctx.pc = 5;
    ctx.push_call();
    assert_eq!(ctx.call_stack.len(), 1);

    let ret_addr = ctx.pop_call();
    assert_eq!(ret_addr, Some(6)); // PC + 1

    assert_eq!(ctx.pop_call(), None); // empty stack
}

#[test]
fn test_register_files_initialized() {
    let ctx = ExecutionContext::new(vec![]);

    // All integer registers should be zero
    for i in 0..16u8 {
        assert_eq!(ctx.iregs.get(i), 0);
    }

    // All quantum registers should be None
    for qreg in &ctx.qregs {
        assert!(qreg.is_none());
    }
}
