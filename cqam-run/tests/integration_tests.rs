//! End-to-end integration tests for the CQAM runner.
//!
//! Each test loads a `.cqam` example file from disk, parses it, executes
//! it through the full runner pipeline, and verifies expected behavior.
//!
//! Note: Examples are written for 16-qubit registers but the default
//! config uses 2 qubits (4 states). Tests verify structural correctness
//! (halting, register population) rather than algorithm-specific outcomes
//! that require larger qubit counts.

use cqam_run::loader::load_program;
use cqam_run::runner::run_program_with_config;
use cqam_run::simconfig::SimConfig;

/// Resolve example file path relative to the workspace root.
fn example_path(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/../examples/{name}")
}

/// Config with enough cycles and small qubit count for testing.
fn test_config() -> SimConfig {
    SimConfig {
        fidelity_threshold: None,
        max_cycles: Some(5000),
        enable_interrupts: Some(true),
        default_qubits: Some(2),
        force_density_matrix: false,
    }
}

#[test]
fn test_e2e_qrng() {
    let parsed = load_program(&example_path("qrng.cqam")).unwrap();
    let ctx = run_program_with_config(parsed.instructions, &test_config()).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    // F5 = empirical mean (should be finite)
    let mean = ctx.fregs.get(5).unwrap();
    assert!(mean.is_finite(), "Empirical mean should be finite");
    // R2 = sample count = 8
    assert_eq!(ctx.iregs.get(2).unwrap(), 8, "Should have generated 8 samples");
}

#[test]
fn test_e2e_qaoa() {
    let parsed = load_program(&example_path("qaoa.cqam")).unwrap();
    let ctx = run_program_with_config(parsed.instructions, &test_config()).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    // F7 = expected cost (mean), should be finite
    let cost = ctx.fregs.get(7).unwrap();
    assert!(cost.is_finite(), "Expected cost should be finite");
    // R3 = optimal solution (mode)
    let mode = ctx.iregs.get(3).unwrap();
    assert!(mode >= 0, "Mode should be non-negative");
}

#[test]
fn test_e2e_phase_estimation() {
    let parsed = load_program(&example_path("phase_estimation.cqam")).unwrap();
    let ctx = run_program_with_config(parsed.instructions, &test_config()).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    // F4 = mean phase index, should be finite
    let mean = ctx.fregs.get(4).unwrap();
    assert!(mean.is_finite(), "Mean phase index should be finite");
}

#[test]
fn test_e2e_vqe_loop() {
    let parsed = load_program(&example_path("vqe_loop.cqam")).unwrap();
    let ctx = run_program_with_config(parsed.instructions, &test_config()).unwrap();

    assert!(ctx.psw.trap_halt, "Program should halt");
    // R2 = iteration count (should be > 0)
    let iters = ctx.iregs.get(2).unwrap();
    assert!(iters > 0, "Should have performed at least 1 iteration");
}
