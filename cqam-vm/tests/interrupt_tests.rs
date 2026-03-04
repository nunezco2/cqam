// cqam-vm/tests/interrupt_tests.rs
//
// Phase 2: Test the two-level interrupt model with ISR vector table.

use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;
use cqam_vm::isr::{IsrTable, NmiTrap, MaskableTrap, Trap, handle_trap};

#[test]
fn test_nmi_halt_default_sets_trap_halt() {
    let mut ctx = ExecutionContext::new(vec![]);
    let isr = IsrTable::new();
    assert!(!ctx.psw.trap_halt);

    handle_trap(Trap::Nmi(NmiTrap::Halt), &mut ctx, &isr, true);
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_nmi_illegal_pc_default_sets_trap_halt() {
    let mut ctx = ExecutionContext::new(vec![]);
    let isr = IsrTable::new();

    handle_trap(Trap::Nmi(NmiTrap::IllegalPC), &mut ctx, &isr, true);
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_maskable_arithmetic_with_interrupts_enabled() {
    let mut ctx = ExecutionContext::new(vec![]);
    let isr = IsrTable::new();

    handle_trap(
        Trap::Maskable(MaskableTrap::Arithmetic),
        &mut ctx,
        &isr,
        true, // interrupts enabled
    );
    assert!(ctx.psw.trap_arith);
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_maskable_arithmetic_with_interrupts_disabled() {
    let mut ctx = ExecutionContext::new(vec![]);
    let isr = IsrTable::new();

    handle_trap(
        Trap::Maskable(MaskableTrap::Arithmetic),
        &mut ctx,
        &isr,
        false, // interrupts disabled
    );
    // Should be silently ignored
    assert!(!ctx.psw.trap_arith);
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

    let mut isr = IsrTable::new();
    isr.set_handler(&Trap::Nmi(NmiTrap::Halt), 2); // handler at index 2

    handle_trap(Trap::Nmi(NmiTrap::Halt), &mut ctx, &isr, true);

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

    let mut isr = IsrTable::new();
    isr.set_handler(&Trap::Maskable(MaskableTrap::Arithmetic), 1);

    handle_trap(
        Trap::Maskable(MaskableTrap::Arithmetic),
        &mut ctx,
        &isr,
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
    let isr = IsrTable::new();

    handle_trap(
        Trap::Maskable(MaskableTrap::QuantumError),
        &mut ctx,
        &isr,
        true,
    );
    assert!(ctx.psw.int_quantum_err);
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_sync_failure_does_not_halt() {
    let mut ctx = ExecutionContext::new(vec![]);
    let isr = IsrTable::new();

    handle_trap(
        Trap::Maskable(MaskableTrap::SyncFailure),
        &mut ctx,
        &isr,
        true,
    );
    assert!(ctx.psw.int_sync_fail);
    assert!(!ctx.psw.trap_halt); // sync failure doesn't halt by default
}
