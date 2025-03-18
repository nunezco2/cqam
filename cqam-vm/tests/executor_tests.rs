// cqam-vm/tests/executor_tests.rs

use cqam_vm::{context::ExecutionContext, executor::execute_instruction};

#[test]
fn test_executor_progresses_pc() {
    let prog = vec![
        "CL:LOAD R1, 1".to_string(),
        "CL:ADD R2, R1, 2".to_string(),
    ];
    let mut ctx = ExecutionContext::new(prog);

    execute_instruction(&mut ctx);
    assert_eq!(ctx.pc, 1);

    execute_instruction(&mut ctx);
    assert_eq!(ctx.pc, 2);
}
