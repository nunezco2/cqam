// cqam-vm/tests/interrupt_tests.rs
//
// Phase 2/8: Test the two-level interrupt model with ISR vector table.
// Phase 8: Updated handle_trap signature (handler_addr instead of &IsrTable).

use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;
use cqam_vm::isr::{IsrTable, NmiTrap, MaskableTrap, Trap, handle_trap};

#[test]
fn test_nmi_halt_default_sets_trap_halt() {
    let mut ctx = ExecutionContext::new(vec![]);
    assert!(!ctx.psw.trap_halt);

    handle_trap(Trap::Nmi(NmiTrap::Halt), &mut ctx, None, true);
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_nmi_illegal_pc_default_sets_trap_halt() {
    let mut ctx = ExecutionContext::new(vec![]);

    handle_trap(Trap::Nmi(NmiTrap::IllegalPC), &mut ctx, None, true);
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_maskable_arithmetic_with_interrupts_enabled() {
    let mut ctx = ExecutionContext::new(vec![]);

    handle_trap(
        Trap::Maskable(MaskableTrap::Arithmetic),
        &mut ctx,
        None,
        true, // interrupts enabled
    );
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_maskable_arithmetic_with_interrupts_disabled() {
    let mut ctx = ExecutionContext::new(vec![]);

    handle_trap(
        Trap::Maskable(MaskableTrap::Arithmetic),
        &mut ctx,
        None,
        false, // interrupts disabled
    );
    // Should be silently ignored
    assert!(!ctx.psw.trap_halt);
}

#[test]
fn test_nmi_with_registered_handler() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },     // 0: main code
        Instruction::Halt,                          // 1: main halt
        Instruction::Label("HANDLER".into()),       // 2: handler entry
        Instruction::ILdi { dst: 15, imm: 99 },    // 3: handler body
        Instruction::Ret,                           // 4: return from handler
    ];

    let mut ctx = ExecutionContext::new(program);
    ctx.pc = 0;

    handle_trap(Trap::Nmi(NmiTrap::Halt), &mut ctx, Some(2), true);

    // Should have jumped to handler
    assert_eq!(ctx.pc, 2);
    // Should have pushed return address onto call stack
    assert_eq!(ctx.call_stack.len(), 1);
    assert_eq!(ctx.call_stack[0], 0); // original PC was 0
}

#[test]
fn test_maskable_with_registered_handler() {
    let program = vec![
        Instruction::Halt,                          // 0
        Instruction::Label("ARITH_HANDLER".into()), // 1
        Instruction::Ret,                           // 2
    ];

    let mut ctx = ExecutionContext::new(program);
    ctx.pc = 0;

    handle_trap(
        Trap::Maskable(MaskableTrap::Arithmetic),
        &mut ctx,
        Some(1),
        true,
    );

    assert_eq!(ctx.pc, 1); // jumped to handler
    assert_eq!(ctx.call_stack.len(), 1);
    // PSW flags should NOT be set when handler exists
    assert!(!ctx.psw.trap_halt);
}

#[test]
fn test_isr_table_get_handler_returns_none_when_not_set() {
    let isr = IsrTable::new();
    assert!(isr.get_handler(&Trap::Nmi(NmiTrap::Halt)).is_none());
    assert!(isr.get_handler(&Trap::Maskable(MaskableTrap::Arithmetic)).is_none());
}

#[test]
fn test_quantum_error_with_interrupts_enabled() {
    let mut ctx = ExecutionContext::new(vec![]);

    handle_trap(
        Trap::Maskable(MaskableTrap::QuantumError),
        &mut ctx,
        None,
        true,
    );
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_sync_failure_default_sets_halt() {
    let mut ctx = ExecutionContext::new(vec![]);

    handle_trap(
        Trap::Maskable(MaskableTrap::SyncFailure),
        &mut ctx,
        None,
        true,
    );
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_isr_table_set_handler_overwrites() {
    let mut isr = IsrTable::new();
    let trap = Trap::Maskable(MaskableTrap::Arithmetic);

    isr.set_handler(&trap, 10);
    assert_eq!(isr.get_handler(&trap), Some(10));

    isr.set_handler(&trap, 20);
    assert_eq!(isr.get_handler(&trap), Some(20), "Second set_handler should overwrite");
}

#[test]
fn test_maskable_handler_with_interrupts_disabled_is_ignored() {
    // Even if a handler is registered, if interrupts are disabled the trap is silently ignored.
    let mut ctx = ExecutionContext::new(vec![Instruction::Halt]);
    ctx.pc = 0;

    handle_trap(
        Trap::Maskable(MaskableTrap::Arithmetic),
        &mut ctx,
        Some(0), // handler exists
        false,   // interrupts disabled
    );

    // PC should not change, no call stack push, no trap_halt
    assert_eq!(ctx.pc, 0, "PC should not change when interrupts disabled");
    assert!(ctx.call_stack.is_empty(), "Call stack should remain empty");
    assert!(!ctx.psw.trap_halt, "trap_halt should not be set");
}

#[test]
fn test_nmi_always_fires_regardless_of_interrupt_flag() {
    // NMI traps must fire even when enable_interrupts is false
    let mut ctx = ExecutionContext::new(vec![]);

    handle_trap(
        Trap::Nmi(NmiTrap::Halt),
        &mut ctx,
        None,
        false, // interrupts disabled -- should not matter for NMI
    );

    assert!(ctx.psw.trap_halt, "NMI Halt must fire even with interrupts disabled");
}

#[test]
fn test_nmi_with_handler_fires_regardless_of_interrupt_flag() {
    let program = vec![
        Instruction::Halt,                       // 0
        Instruction::Label("HANDLER".into()),    // 1
    ];
    let mut ctx = ExecutionContext::new(program);
    ctx.pc = 0;

    handle_trap(
        Trap::Nmi(NmiTrap::IllegalPC),
        &mut ctx,
        Some(1),
        false, // interrupts disabled -- should not matter for NMI
    );

    assert_eq!(ctx.pc, 1, "NMI handler should fire even with interrupts disabled");
    assert_eq!(ctx.call_stack.len(), 1);
}
