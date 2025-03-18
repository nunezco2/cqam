use cqam_vm::context::ExecutionContext;
use cqam_vm::psw::Trap;
use cqam_vm::isr::handle_trap;

#[test]
fn test_interrupt_handler_sets_halt() {
    let mut ctx = ExecutionContext::new(vec![]);
    assert!(!ctx.psw.trap_halt);

    handle_trap(Trap::QuantumError, &mut ctx);
    assert!(ctx.psw.trap_halt);
}
