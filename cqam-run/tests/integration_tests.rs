// cqam-run/tests/integration_tests.rs
//
// Phase 9.1: End-to-end integration tests.
//
// Each test loads a `.cqam` example file from disk, parses it, executes
// it through the full runner pipeline, and verifies expected register
// and memory values.

use cqam_run::loader::load_program;
use cqam_run::runner::run_program;

/// Resolve example file path relative to the workspace root.
fn example_path(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/../examples/{name}")
}

#[test]
fn test_e2e_arithmetic() {
    let program = load_program(&example_path("arithmetic.cqam")).unwrap();
    let ctx = run_program(program).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");

    // Integer section
    assert_eq!(ctx.iregs.get(2).unwrap(), 35, "R2 = 10 + 25");
    assert_eq!(ctx.iregs.get(3).unwrap(), 15, "R3 = 25 - 10");
    assert_eq!(ctx.iregs.get(4).unwrap(), 250, "R4 = 10 * 25");
    assert_eq!(ctx.iregs.get(5).unwrap(), 5, "R5 = 25 % 10");
    assert_eq!(ctx.iregs.get(6).unwrap(), 35, "R6 loaded from CMEM[100]");
    assert_eq!(ctx.iregs.get(7).unwrap(), 1, "R7 = (35 == 35)");
    assert_eq!(ctx.iregs.get(8).unwrap(), 1, "R8 = (10 < 25)");
    assert_eq!(ctx.iregs.get(9).unwrap(), 0, "R9 = (10 > 25)");
    assert_eq!(ctx.cmem.load(100), 35, "CMEM[100] = 35");

    // Float section
    assert!((ctx.fregs.get(2).unwrap() - 10.0).abs() < 1e-10, "F2 = 3.0 + 7.0");
    assert!((ctx.fregs.get(3).unwrap() - 4.0).abs() < 1e-10, "F3 = 7.0 - 3.0");
    assert!((ctx.fregs.get(4).unwrap() - 21.0).abs() < 1e-10, "F4 = 3.0 * 7.0");

    // Type conversion
    assert_eq!(ctx.iregs.get(10).unwrap(), 2, "R10 = truncation of 7/3");
}

#[test]
fn test_e2e_grover() {
    let program = load_program(&example_path("grover.cqam")).unwrap();
    let ctx = run_program(program).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    // R4 holds the mode (most probable state) after HREDUCE
    let mode = ctx.iregs.get(4).unwrap();
    assert!(mode >= 0, "Mode should be a valid non-negative index");
    // F0 holds the mean
    let mean = ctx.fregs.get(0).unwrap();
    assert!(mean.is_finite(), "Mean should be finite");
}

#[test]
fn test_e2e_quantum_observe() {
    let program = load_program(&example_path("quantum_observe.cqam")).unwrap();
    let ctx = run_program(program).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    // F0 holds mean of observed distribution
    let mean = ctx.fregs.get(0).unwrap();
    assert!(mean.is_finite(), "Mean should be finite");
    // R2 holds mode
    let mode = ctx.iregs.get(2).unwrap();
    assert!(mode >= 0, "Mode should be non-negative");
}

#[test]
fn test_e2e_hybrid_fork() {
    let program = load_program(&example_path("hybrid_fork.cqam")).unwrap();
    let ctx = run_program(program).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    assert_eq!(ctx.iregs.get(2).unwrap(), 1, "R2 = (42 == 42) = 1");
    let r3 = ctx.iregs.get(3).unwrap();
    assert!(r3 == 100 || r3 == 200, "R3 should be 100 or 200, got {}", r3);
}
