// cqam-vm/tests/context_tests.rs

use cqam_vm::context::ExecutionContext;

#[test]
fn test_context_pc_and_program_flow() {
    let prog = vec![
        "CL:LOAD R1, 5".to_string(),
        "CL:ADD R2, R1, 3".to_string(),
    ];
    let mut ctx = ExecutionContext::new(prog);

    assert_eq!(ctx.pc, 0);
    assert_eq!(ctx.current_line(), Some(&"CL:LOAD R1, 5".to_string()));

    ctx.advance_pc();
    assert_eq!(ctx.pc, 1);
    assert_eq!(ctx.current_line(), Some(&"CL:ADD R2, R1, 3".to_string()));

    ctx.advance_pc();
    assert_eq!(ctx.current_line(), None);
}
