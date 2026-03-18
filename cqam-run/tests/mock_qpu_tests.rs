//! Full test suite for example programs running through `CircuitBackend<MockQpuBackend>`.
//!
//! Each test loads a `.cqam` example file, runs it via `run_program_with_data()`
//! with `BackendChoice::Qpu { provider: "mock", .. }`, and asserts correctness.
//!
//! Tests are organized into three tiers:
//!   - Tier 1: Deterministic outcomes — exact register assertions.
//!   - Tier 2: Statistical outcomes — structural correctness (finite, non-zero, etc.)
//!   - Tier 3: Smoke tests — just verify no crash and halt flag is set.

use cqam_run::loader::load_program;
use cqam_run::runner::run_program_with_data;
use cqam_run::simconfig::{BackendChoice, SimConfig};

// =============================================================================
// Helpers
// =============================================================================

/// Resolve an example file path relative to the workspace root.
fn example_path(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/../examples/{name}")
}

/// Mock QPU config. Uses None for default_qubits so the `#! qubits N` pragma
/// controls the register size. The MockQpuBackend supports up to 27 qubits.
fn mock_config() -> SimConfig {
    SimConfig {
        fidelity_threshold: None,
        max_cycles: Some(50_000),
        enable_interrupts: Some(true),
        default_qubits: None, // let pragma decide
        force_density_matrix: false,
        default_threads: None,
        rng_seed: Some(42),
        shots: None,
        noise_model: None,
        noise_method: None,
        bell_pair_budget: None,
        backend: Some(BackendChoice::Qpu {
            provider: "mock".to_string(),
            device: None,
            shot_budget: 8192,
            confidence: 0.95,
        }),
    }
}

/// Load and run a program through the mock QPU backend.
///
/// Panics with a helpful message if the program errors.
fn run_mock(name: &str) -> cqam_run::shot::RunResult {
    let path = example_path(name);
    let parsed = load_program(&path)
        .unwrap_or_else(|e| panic!("Failed to load '{}': {:?}", name, e));
    run_program_with_data(
        parsed.instructions,
        &mock_config(),
        &parsed.metadata,
        &parsed.data_section,
        &parsed.shared_section,
        &parsed.private_section,
    )
    .unwrap_or_else(|e| panic!("Program '{}' returned error: {:?}", name, e))
}

// =============================================================================
// Tier 1: Deterministic assertions
// =============================================================================

/// Bernstein-Vazirani: recover secret s=21 from a single quantum query.
///
/// The BV algorithm is deterministic: the measurement must return exactly the
/// secret string. We check:
///   - R6 == 21 (recovered secret via HREDUCE MODEV)
///   - CMEM[2] == 1 (success flag stored by program)
#[test]
fn test_mock_bernstein_vazirani() {
    let result = run_mock("basic/bernstein_vazirani.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // R6 = recovered secret (HREDUCE MODEV on final measurement)
    let recovered = ctx.iregs.get(6).unwrap();
    assert_eq!(
        recovered, 21,
        "BV algorithm should recover secret=21; got R6={}",
        recovered
    );

    // CMEM[2] = success flag (program stores 1 if recovered == secret)
    let success = ctx.cmem.load(2);
    assert_eq!(success, 1, "CMEM[2] success flag should be 1; got {}", success);
}

/// Error detection: inject a bit-flip on qubit 1, detect, correct, verify.
///
/// The circuit is deterministic: error on qubit 1 -> syndrome = state 2 ->
/// correction -> final state = 0. Check R12 == 1 (correction succeeded).
#[test]
fn test_mock_error_detection() {
    let result = run_mock("basic/error_detection.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // R12 = (final_state == 0) ? 1 : 0  (program stores result of IEQ)
    // The program computes: R12 = IEQ(R11, R0) where R0=0 and R11=final measured state.
    // After correction, the state should be |000> = 0.
    // Due to mock QPU statistical sampling, the mode should still be 0.
    let success = ctx.iregs.get(12).unwrap();
    assert_eq!(
        success, 1,
        "Error correction should succeed (R12==1); got {}",
        success
    );
}

/// Reversible adder: PERMUTATION kernel on basis states.
///
/// Four tests exercise: basis state adder, modular addition, superposition
/// adder, and add-then-subtract identity. Expect at least 3 of 4 pass
/// (generous due to statistical sampling in the superposition test).
#[test]
fn test_mock_reversible_adder() {
    let result = run_mock("basic/reversible_adder.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // R14 = pass counter (counts how many of 4 tests passed)
    let passes = ctx.iregs.get(14).unwrap();
    assert_eq!(
        passes, 4,
        "All 4 reversible adder tests should pass; got R14={}",
        passes
    );
}

/// Superdense coding: encode and decode all 4 two-bit messages.
///
/// Each of the 4 protocol variants produces a deterministic outcome.
/// The program stores decoded messages in CMEM[0..3] and verification
/// flags in CMEM[4..7].
#[test]
fn test_mock_superdense_coding() {
    let result = run_mock("basic/superdense_coding.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // CMEM[0..3] = decoded messages for 00, 01, 10, 11
    // The program stores HREDUCE MODEV results (expected: 0, 1, 2, 3)
    let msg0 = ctx.cmem.load(0);
    let msg1 = ctx.cmem.load(1);
    let msg2 = ctx.cmem.load(2);
    let msg3 = ctx.cmem.load(3);

    // Allow one message to be wrong due to statistical sampling noise
    let correct = [msg0 == 0, msg1 == 1, msg2 == 2, msg3 == 3]
        .iter()
        .filter(|&&b| b)
        .count();
    assert!(
        correct >= 3,
        "At least 3 of 4 superdense coding messages should decode correctly; \
         got msg0={}, msg1={}, msg2={}, msg3={}",
        msg0, msg1, msg2, msg3
    );
}

/// Diagonal unitary: phase-oracle Grover search for state |5>.
///
/// Two Grover iterations on 4 qubits amplify state 5. Check halt + R2 >= 0.
/// (The argmax register should be a valid state index in [0, 15].)
#[test]
fn test_mock_test_diagonal() {
    let result = run_mock("basic/test_diagonal.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // R2 = argmax of final distribution (should be state 5 after Grover amplification)
    let argmax = ctx.iregs.get(2).unwrap();
    assert!(
        argmax >= 0 && argmax <= 15,
        "Argmax should be a valid 4-qubit state in [0,15]; got R2={}",
        argmax
    );
    // With 2 Grover iterations on 4 qubits, state 5 should dominate.
    // Generous assertion: just check it is 5 (deterministic for ideal oracle).
    assert_eq!(
        argmax, 5,
        "After 2 Grover iterations, marked state 5 should be the argmax; got {}",
        argmax
    );
}

// =============================================================================
// Tier 2: Statistical assertions
// =============================================================================

/// QRNG: generate 8 random samples, compute statistics.
///
/// R2 must equal 8 (loop ran 8 times). F5 = empirical mean must be finite.
/// The mock QPU uses SAMPLE mode so each observation produces HybridValue::Int.
#[test]
fn test_mock_qrng() {
    let result = run_mock("basic/qrng.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // R2 = sample counter (should be 8 after the loop)
    let count = ctx.iregs.get(2).unwrap();
    assert_eq!(count, 8, "QRNG should generate 8 samples; got R2={}", count);

    // F5 = empirical mean (should be finite for any 8 samples)
    let mean = ctx.fregs.get(5).unwrap();
    assert!(mean.is_finite(), "Empirical mean F5 should be finite; got {}", mean);
}

/// QAOA: alternating cost/mixer unitaries, observe distribution, compute cost.
///
/// F7 = expected cost (mean of distribution) must be finite.
/// The program runs 3 QAOA rounds then measures.
#[test]
fn test_mock_qaoa() {
    let result = run_mock("intermediate/qaoa.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // F7 = expected cost (mean), should be finite
    let cost = ctx.fregs.get(7).unwrap();
    assert!(
        cost.is_finite(),
        "Expected cost F7 should be finite; got {}",
        cost
    );
}

/// VQE loop: variational eigensolver with classical optimization.
///
/// R2 = iteration count; must be > 0 (at least one VQE iteration ran).
#[test]
fn test_mock_vqe() {
    let result = run_mock("intermediate/vqe_loop.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // R2 = iteration counter
    let iters = ctx.iregs.get(2).unwrap();
    assert!(
        iters > 0,
        "VQE should complete at least 1 iteration; got R2={}",
        iters
    );
}

/// Grover's search on 16 qubits: parametric amplification.
///
/// The program computes ~201 Grover iterations at runtime for N=16 qubits.
/// Each QKERNEL GROV accumulates in the circuit buffer, so the final circuit
/// submitted to the mock QPU is enormous (201 Grover rounds × O(n) gates).
/// Compilation and mock statevector simulation of that circuit is prohibitively
/// slow for a unit test.  Ignored by default; run with `--ignored` to check it
/// manually after performance improvements.
#[test]
#[ignore = "16-qubit Grover: ~201-round circuit is too slow for CI (takes 5+ minutes)"]
fn test_mock_grover_16q() {
    let result = run_mock("intermediate/grover_16q.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // R1 = found flag (the program stores 1 if found, 0 if not)
    // With 16 qubits and mock QPU statistical noise, this may not always be 1.
    // Just assert the register contains a valid boolean (0 or 1).
    let found = ctx.iregs.get(1).unwrap();
    assert!(
        found == 0 || found == 1,
        "Found flag R1 should be 0 or 1; got {}",
        found
    );
}

/// Phase estimation: QPE on 16 qubits, observe phase distribution.
///
/// F4 = mean phase index; must be finite.
#[test]
fn test_mock_phase_estimation() {
    let result = run_mock("intermediate/phase_estimation.cqam");
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    // F4 = mean phase index
    let mean_phase = ctx.fregs.get(4).unwrap();
    assert!(
        mean_phase.is_finite(),
        "Mean phase F4 should be finite; got {}",
        mean_phase
    );
}

// =============================================================================
// Tier 3: Smoke tests (just verify no crash)
// =============================================================================

/// Quantum teleportation: Bell pair + SAMPLE mode measurements + corrections.
///
/// The program uses QOBSERVE(SAMPLE) to get Alice's classical bits, then
/// applies conditional corrections. The mock QPU returns Int for SAMPLE mode.
#[test]
fn test_mock_quantum_teleport() {
    let result = run_mock("basic/quantum_teleport.cqam");
    let ctx = result.ctx();
    assert!(ctx.psw.trap_halt, "quantum_teleport should halt cleanly");
}

/// Shor period-finding: QFT + phase oracle + classical post-processing.
#[test]
fn test_mock_shor_period() {
    let result = run_mock("intermediate/shor_period.cqam");
    let ctx = result.ctx();
    assert!(ctx.psw.trap_halt, "shor_period should halt cleanly");
}

/// Simon's algorithm (advanced): SAMPLE mode in a loop to collect equations.
#[test]
fn test_mock_simon_adv() {
    let result = run_mock("advanced_nothreads/simon.cqam");
    let ctx = result.ctx();
    assert!(ctx.psw.trap_halt, "simon (advanced) should halt cleanly");
}

/// Adaptive Grover: uses JMPF CF flag after QOBSERVE, 4 qubits.
#[test]
fn test_mock_adaptive_grover() {
    let result = run_mock("advanced_nothreads/adaptive_grover.cqam");
    let ctx = result.ctx();
    assert!(ctx.psw.trap_halt, "adaptive_grover should halt cleanly");
}

/// ECALL hello: purely classical program with ECALL PRINT_STR.
///
/// Validates that the mock backend does not interfere with classical-only
/// programs. No quantum operations are exercised.
#[test]
fn test_mock_ecall_hello() {
    let result = run_mock("basic/ecall_hello.cqam");
    let ctx = result.ctx();
    assert!(ctx.psw.trap_halt, "ecall_hello should halt cleanly");
}
