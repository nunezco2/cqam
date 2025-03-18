// cqam-vm/tests/executor_tests.rs

use cqam_vm::{context::ExecutionContext, executor::run_program};
use cqam_core::register::CValue;

#[test]
fn test_run_program_cl_load_and_add() {
    let prog = vec![
        "CL:LOAD R1, 10".to_string(),
        "CL:LOAD R2, 32".to_string(),
        // We haven't implemented parsing for ADD yet, but placeholder here for future
    ];
    let mut ctx = ExecutionContext::new(prog);
    run_program(&mut ctx);
    assert_eq!(ctx.registers.load_c("R1"), Some(&CValue::Int(10)));
    assert_eq!(ctx.registers.load_c("R2"), Some(&CValue::Int(32)));
}
