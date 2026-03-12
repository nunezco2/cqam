//! Integration tests for the CQAM runner: error propagation, simulation
//! configuration enforcement, and ISR wiring with interrupt completion.

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

// --- Error propagation tests ---

#[test]
fn test_division_by_zero_halts_via_isr() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 },
    ];

    let ctx = run_program(program).unwrap();
    // Division by zero sets trap_arith, ISR dispatch (no handler) sets trap_halt
    assert!(ctx.psw.trap_halt);
    assert_eq!(ctx.iregs.get(2).unwrap(), 0); // safe default
}

// --- SimConfig enforcement tests ---

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
        default_qubits: None,
        force_density_matrix: false,
        default_threads: None,
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
        default_qubits: None,
        force_density_matrix: false,
        default_threads: None,
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

// --- ISR wiring and interrupt completion tests ---

#[test]
fn test_setiv_div_by_zero_handler_executes_and_reti_resumes() {
    // SETIV registers a handler for Arithmetic trap (trap_id=0).
    // Div by zero fires the trap, handler runs, RETI resumes after the div.
    let program = vec![
        Instruction::SetIV { trap_id: 0, target: "ARITH_HANDLER".into() },
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }, // trap_arith fires
        // After RETI, execution resumes here (PC was saved before handler jump)
        Instruction::ILdi { dst: 3, imm: 77 },
        Instruction::Halt,
        // Handler:
        Instruction::Label("ARITH_HANDLER".into()),
        Instruction::ILdi { dst: 15, imm: 99 }, // marker: handler ran
        Instruction::Reti,
    ];

    let ctx = run_program(program).unwrap();

    assert_eq!(ctx.iregs.get(15).unwrap(), 99, "Handler should have run");
    assert_eq!(ctx.iregs.get(3).unwrap(), 77, "Execution should resume after handler");
    assert!(ctx.psw.trap_halt, "Should halt normally");
    assert!(!ctx.psw.trap_arith, "RETI should have cleared trap_arith");
}

#[test]
fn test_unregistered_trap_falls_through_to_halt() {
    // No SETIV: div by zero with no handler → default behavior (halt)
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 },
        Instruction::ILdi { dst: 3, imm: 999 }, // should NOT execute
    ];

    let ctx = run_program(program).unwrap();

    assert!(ctx.psw.trap_halt, "Should halt on unhandled trap");
    assert_eq!(ctx.iregs.get(3).unwrap(), 0, "Should not reach instruction after trap");
}

#[test]
fn test_maskable_trap_ignored_when_interrupts_disabled() {
    // With enable_interrupts=false, maskable traps are silently ignored
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }, // trap_arith fires
        Instruction::ILdi { dst: 3, imm: 77 },         // should execute (trap ignored)
    ];

    let config = SimConfig {
        fidelity_threshold: None,
        max_cycles: Some(100),
        enable_interrupts: Some(false),
        default_qubits: None,
        force_density_matrix: false,
        default_threads: None,
    };

    let ctx = run_program_with_config(program, &config).unwrap();

    assert!(!ctx.psw.trap_halt, "Should NOT halt when interrupts disabled");
    assert_eq!(ctx.iregs.get(3).unwrap(), 77, "Execution should continue");
}

#[test]
fn test_reti_with_empty_call_stack_halts() {
    // RETI with no call stack entry acts as HALT
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Reti,
        Instruction::ILdi { dst: 1, imm: 999 },
    ];

    let ctx = run_program(program).unwrap();

    assert!(ctx.psw.trap_halt);
    assert_eq!(ctx.iregs.get(1).unwrap(), 0, "Should not execute after RETI-halt");
}

#[test]
fn test_setiv_invalid_trap_id_returns_error() {
    let program = vec![
        Instruction::SetIV { trap_id: 5, target: "HANDLER".into() },
        Instruction::Label("HANDLER".into()),
        Instruction::Halt,
    ];

    let result = run_program(program);
    assert!(result.is_err(), "Invalid trap_id should return error");
}

#[test]
fn test_fidelity_threshold_wiring() {
    // Verify SimConfig.fidelity_threshold is wired to context config
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Halt,
    ];

    let config = SimConfig {
        fidelity_threshold: Some(0.85),
        max_cycles: Some(100),
        enable_interrupts: Some(true),
        default_qubits: None,
        force_density_matrix: false,
        default_threads: None,
    };

    let ctx = run_program_with_config(program, &config).unwrap();

    assert!((ctx.config.min_purity - 0.85).abs() < f64::EPSILON);
}

#[test]
fn test_setiv_unresolved_label_returns_error() {
    // SetIV referencing a label that does not exist should return UnresolvedLabel error
    let program = vec![
        Instruction::SetIV { trap_id: 0, target: "NONEXISTENT".into() },
        Instruction::Halt,
    ];

    let result = run_program(program);
    assert!(result.is_err(), "SetIV with unresolved label should return error");
}

#[test]
fn test_imod_by_zero_halts_via_isr() {
    // IMod by zero should also set trap_arith and halt through ISR dispatch
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IMod { dst: 2, lhs: 0, rhs: 1 },
    ];

    let ctx = run_program(program).unwrap();
    assert!(ctx.psw.trap_halt, "IMod by zero should halt via ISR default");
    assert_eq!(ctx.iregs.get(2).unwrap(), 0, "IMod by zero should set dst to 0");
}

#[test]
fn test_setiv_overwrites_previous_handler() {
    // Registering a second handler for the same trap_id should overwrite the first.
    // First handler sets R15=11, second handler sets R15=22.
    let program = vec![
        Instruction::SetIV { trap_id: 0, target: "HANDLER_A".into() },
        Instruction::SetIV { trap_id: 0, target: "HANDLER_B".into() }, // overwrite
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }, // trigger trap
        Instruction::Halt,
        Instruction::Label("HANDLER_A".into()),
        Instruction::ILdi { dst: 15, imm: 11 },
        Instruction::Reti,
        Instruction::Label("HANDLER_B".into()),
        Instruction::ILdi { dst: 15, imm: 22 },
        Instruction::Reti,
    ];

    let ctx = run_program(program).unwrap();
    assert_eq!(ctx.iregs.get(15).unwrap(), 22, "Second handler should have run (overwrite)");
    assert!(ctx.psw.trap_halt, "Should halt normally");
}

#[test]
fn test_imod_by_zero_with_handler_resumes() {
    // Like the IDiv handler test, but for IMod
    let program = vec![
        Instruction::SetIV { trap_id: 0, target: "ARITH_HANDLER".into() },
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IMod { dst: 2, lhs: 0, rhs: 1 }, // trap_arith fires
        Instruction::ILdi { dst: 3, imm: 77 },
        Instruction::Halt,
        Instruction::Label("ARITH_HANDLER".into()),
        Instruction::ILdi { dst: 15, imm: 99 },
        Instruction::Reti,
    ];

    let ctx = run_program(program).unwrap();
    assert_eq!(ctx.iregs.get(15).unwrap(), 99, "Handler should have run");
    assert_eq!(ctx.iregs.get(3).unwrap(), 77, "Execution should resume after handler");
}

// --- Pragma qubits integration tests ---

#[test]
fn test_pragma_qubits_applied() {
    use cqam_core::parser::parse_program;
    use cqam_run::runner::run_program_with_config_and_metadata;

    let source = "#! qubits 4\nQPREP Q0, 0\nHALT\n";
    let parsed = parse_program(source).unwrap();

    let config = SimConfig {
        fidelity_threshold: None,
        max_cycles: Some(100),
        enable_interrupts: Some(true),
        default_qubits: None, // no CLI override
        force_density_matrix: false,
        default_threads: None,
    };

    let ctx = run_program_with_config_and_metadata(
        parsed.instructions, &config, &parsed.metadata,
    ).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    let dm = ctx.qregs[0].as_ref().expect("Q0 should be prepared");
    assert_eq!(dm.num_qubits(), 4, "Pragma should set 4 qubits");
    assert_eq!(dm.dimension(), 16, "4 qubits => dimension 16");
}

#[test]
fn test_cli_overrides_pragma() {
    use cqam_core::parser::parse_program;
    use cqam_run::runner::run_program_with_config_and_metadata;

    let source = "#! qubits 4\nQPREP Q0, 0\nHALT\n";
    let parsed = parse_program(source).unwrap();

    let config = SimConfig {
        fidelity_threshold: None,
        max_cycles: Some(100),
        enable_interrupts: Some(true),
        default_qubits: Some(3), // CLI override: 3 qubits
        force_density_matrix: false,
        default_threads: None,
    };

    let ctx = run_program_with_config_and_metadata(
        parsed.instructions, &config, &parsed.metadata,
    ).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    let dm = ctx.qregs[0].as_ref().expect("Q0 should be prepared");
    assert_eq!(dm.num_qubits(), 3, "CLI should override pragma: 3 qubits");
    assert_eq!(dm.dimension(), 8, "3 qubits => dimension 8");
}
