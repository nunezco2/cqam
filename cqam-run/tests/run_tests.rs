// cqam-run/tests/run_tests.rs
//
// Phase 4/6: Integration tests for the runner with Result-based error handling
// and SimConfig enforcement.

use cqam_core::instruction::Instruction;
use cqam_run::runner::{run_program, run_program_with_config};
use cqam_run::simconfig::SimConfig;

#[test]
fn test_no_double_pc_advance() {
    let program = vec![
        Instruction::ILdi { dst: 1, imm: 42 },
        Instruction::ILdi { dst: 2, imm: 7 },
        Instruction::IAdd { dst: 3, lhs: 1, rhs: 2 },
    ];

    let ctx = run_program(program).unwrap();

    assert_eq!(ctx.pc, 3, "PC should be 3 after executing 3 instructions");
    assert_eq!(ctx.iregs.get(1).unwrap(), 42);
    assert_eq!(ctx.iregs.get(2).unwrap(), 7);
    assert_eq!(ctx.iregs.get(3).unwrap(), 49);
}

#[test]
fn test_pc_advance_with_jump() {
    let program = vec![
        Instruction::Jmp { target: "SKIP".into() },
        Instruction::ILdi { dst: 1, imm: 999 },
        Instruction::Label("SKIP".into()),
        Instruction::ILdi { dst: 2, imm: 42 },
    ];

    let ctx = run_program(program).unwrap();

    assert_eq!(ctx.iregs.get(1).unwrap(), 0, "R1 should be 0 (instruction was skipped)");
    assert_eq!(ctx.iregs.get(2).unwrap(), 42);
    assert_eq!(ctx.pc, 4);
}

#[test]
fn test_call_ret_flow() {
    let program = vec![
        Instruction::Call { target: "FUNC".into() },
        Instruction::ILdi { dst: 0, imm: 100 },
        Instruction::Halt,
        Instruction::Label("FUNC".into()),
        Instruction::ILdi { dst: 1, imm: 42 },
        Instruction::Ret,
    ];

    let ctx = run_program(program).unwrap();

    assert_eq!(ctx.iregs.get(1).unwrap(), 42, "R1 should be set in FUNC");
    assert_eq!(ctx.iregs.get(0).unwrap(), 100, "R0 should be set after return");
    assert!(ctx.psw.trap_halt, "Should halt");
}

#[test]
fn test_jif_conditional_execution() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Jif { pred: 0, target: "SKIP".into() },
        Instruction::ILdi { dst: 1, imm: 999 },
        Instruction::Label("SKIP".into()),
        Instruction::ILdi { dst: 2, imm: 42 },
    ];

    let ctx = run_program(program).unwrap();

    assert_eq!(ctx.iregs.get(1).unwrap(), 0, "R1 should not be set (skipped)");
    assert_eq!(ctx.iregs.get(2).unwrap(), 42, "R2 should be set after skip");
}

#[test]
fn test_halt_terminates_execution() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Halt,
        Instruction::ILdi { dst: 1, imm: 999 },
    ];

    let ctx = run_program(program).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 1);
    assert_eq!(ctx.iregs.get(1).unwrap(), 0, "R1 should not be set (after HALT)");
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_arithmetic_with_memory() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 10 },
        Instruction::ILdi { dst: 1, imm: 20 },
        Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 },
        Instruction::IStr { src: 2, addr: 100 },
        Instruction::ILdm { dst: 3, addr: 100 },
    ];

    let ctx = run_program(program).unwrap();

    assert_eq!(ctx.iregs.get(2).unwrap(), 30);
    assert_eq!(ctx.iregs.get(3).unwrap(), 30);
    assert_eq!(ctx.cmem.load(100), 30);
}

// ===========================================================================
// Error propagation tests (Phase 4)
// ===========================================================================

#[test]
fn test_division_by_zero_propagates_error() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 },
    ];

    let result = run_program(program);
    assert!(result.is_err());
    let msg = format!("{}", result.err().unwrap());
    assert!(msg.contains("Division by zero"));
}

// ===========================================================================
// SimConfig enforcement tests (Phase 6.5)
// ===========================================================================

#[test]
fn test_max_cycles_enforcement() {
    // Create an infinite loop: JMP to label at the same position
    let program = vec![
        Instruction::Label("LOOP".into()),
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Jmp { target: "LOOP".into() },
    ];

    let config = SimConfig {
        fidelity_threshold: None,
        max_cycles: Some(10),
        enable_interrupts: Some(false),
    };

    let ctx = run_program_with_config(program, &config).unwrap();

    // The program should have been halted by max_cycles enforcement
    assert!(
        ctx.psw.trap_halt,
        "Program should halt after max_cycles exceeded"
    );
}

#[test]
fn test_max_cycles_allows_short_programs() {
    // A short program should complete without hitting max_cycles
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 7 },
        Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 },
    ];

    let config = SimConfig {
        fidelity_threshold: None,
        max_cycles: Some(100),
        enable_interrupts: Some(false),
    };

    let ctx = run_program_with_config(program, &config).unwrap();

    assert_eq!(ctx.iregs.get(2).unwrap(), 49);
    assert_eq!(ctx.pc, 3);
}

#[test]
fn test_run_program_with_default_config() {
    // run_program uses default config (max_cycles=1000)
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 100 },
        Instruction::Halt,
    ];

    let ctx = run_program(program).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 100);
    assert!(ctx.psw.trap_halt);
}
