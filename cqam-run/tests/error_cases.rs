//! Error handling and edge case tests for the QPU pipeline.
//!
//! Tests verify that `CircuitBackend<MockQpuBackend>` correctly rejects
//! unsupported operations, enforces resource limits, and handles edge cases.
//!
//! Tests are placed here (cqam-run) because they require `CircuitBackend`
//! (from cqam-sim), `MockQpuBackend` (from cqam-qpu), and the runner
//! (from cqam-run), all of which are dependencies of this crate.

use cqam_core::complex::C64;
use cqam_core::error::CqamError;
use cqam_core::instruction::{DistId, Instruction, ObserveMode};
use cqam_core::quantum_backend::{ObserveResult, QuantumBackend};
use cqam_core::register::HybridValue;
use cqam_core::native_ir::NativeGateSet;

use cqam_qpu::mock::{MockCalibrationData, MockQpuBackend};
use cqam_qpu::traits::{ConnectivityGraph, ConvergenceCriterion};

use cqam_sim::circuit_backend::CircuitBackend;

use cqam_run::runner::run_program_with_config;
use cqam_run::simconfig::{BackendChoice, SimConfig};

// =============================================================================
// Helpers
// =============================================================================

/// Build a CircuitBackend backed by MockQpuBackend with all-to-all connectivity
/// and the given max_qubits limit.
fn make_test_backend(max_qubits: u32) -> CircuitBackend<MockQpuBackend> {
    let qpu = MockQpuBackend::with_config(
        ConnectivityGraph::all_to_all(max_qubits),
        NativeGateSet::Superconducting,
        max_qubits,
        MockCalibrationData::default(),
        Some(42),
    );
    CircuitBackend::new(qpu, ConvergenceCriterion::default(), 8192)
}

/// Build a CircuitBackend with a linear chain topology of `n` qubits.
fn make_linear_backend(n: u32) -> CircuitBackend<MockQpuBackend> {
    let qpu = MockQpuBackend::with_config(
        ConnectivityGraph::linear(n),
        NativeGateSet::Superconducting,
        n,
        MockCalibrationData::default(),
        Some(42),
    );
    CircuitBackend::new(qpu, ConvergenceCriterion::default(), 8192)
}

/// Build a SimConfig using the mock QPU backend.
fn mock_qpu_config() -> SimConfig {
    SimConfig {
        fidelity_threshold: None,
        max_cycles: Some(5000),
        enable_interrupts: Some(true),
        default_qubits: Some(2),
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
        ibm_token: None,
        ibm_optimization_level: None,
    }
}

/// Build a SimConfig with a finite Bell pair budget.
fn mock_qpu_config_with_bell_budget(budget: u32) -> SimConfig {
    SimConfig {
        bell_pair_budget: Some(budget),
        ..mock_qpu_config()
    }
}

/// Hadamard matrix (2x2, row-major, C64).
fn h_gate() -> [C64; 4] {
    let s = std::f64::consts::FRAC_1_SQRT_2;
    [C64(s, 0.0), C64(s, 0.0), C64(s, 0.0), C64(-s, 0.0)]
}

/// CNOT / CX matrix (4x4, row-major, C64).
fn cx_gate() -> [C64; 16] {
    [
        C64(1.0, 0.0), C64(0.0, 0.0), C64(0.0, 0.0), C64(0.0, 0.0),
        C64(0.0, 0.0), C64(1.0, 0.0), C64(0.0, 0.0), C64(0.0, 0.0),
        C64(0.0, 0.0), C64(0.0, 0.0), C64(0.0, 0.0), C64(1.0, 0.0),
        C64(0.0, 0.0), C64(0.0, 0.0), C64(1.0, 0.0), C64(0.0, 0.0),
    ]
}

// =============================================================================
// Test 1: AMP observe mode rejected by CircuitBackend
// =============================================================================

/// `CircuitBackend::observe` with `ObserveMode::Amp` returns
/// `CqamError::QpuUnsupportedOperation { operation: "QOBSERVE/AMP", .. }`.
#[test]
fn test_amp_mode_returns_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let err = cb.observe(h, ObserveMode::Amp, 0, 0);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { ref operation, .. })
            if operation == "QOBSERVE/AMP"),
        "Expected QpuUnsupportedOperation(QOBSERVE/AMP), got: {:?}", err
    );
}

/// Runner-level: QPREP followed by QOBSERVE(AMP) returns
/// `CqamError::QpuUnsupportedOperation`.
#[test]
fn test_amp_mode_via_runner_returns_unsupported() {
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Amp, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];
    let err = run_program_with_config(program, &mock_qpu_config());
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { .. })),
        "Runner: expected QpuUnsupportedOperation for AMP mode, got: {:?}",
        err.err()
    );
}

// =============================================================================
// Test 2: PROB observe mode rejected by CircuitBackend
// =============================================================================

/// `CircuitBackend::observe` with `ObserveMode::Prob` returns
/// `CqamError::QpuUnsupportedOperation { operation: "QOBSERVE/PROB", .. }`.
#[test]
fn test_prob_mode_returns_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let err = cb.observe(h, ObserveMode::Prob, 0, 0);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { ref operation, .. })
            if operation == "QOBSERVE/PROB"),
        "Expected QpuUnsupportedOperation(QOBSERVE/PROB), got: {:?}", err
    );
}

/// Runner-level: QPREP followed by QOBSERVE(PROB) returns
/// `CqamError::QpuUnsupportedOperation`.
#[test]
fn test_prob_mode_via_runner_returns_unsupported() {
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Prob, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];
    let err = run_program_with_config(program, &mock_qpu_config());
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { .. })),
        "Runner: expected QpuUnsupportedOperation for PROB mode, got: {:?}",
        err.err()
    );
}

// =============================================================================
// Test 3: Qubit limit exceeded at submit time
// =============================================================================

/// Preparing 4 qubits on a backend with max_qubits=3 succeeds at prep time
/// but fails at `observe()` with `CqamError::QpuQubitAllocationFailed`.
///
/// The limit is enforced in `MockQpuBackend::compile()` during `qpu.submit()`,
/// not at `prep()` time. This is the expected and documented behavior.
#[test]
fn test_qubit_limit_exceeded_at_submit() {
    let mut cb = make_test_backend(3);
    // prep() allocates 4 wires unconditionally — no limit check here
    let (h, _) = cb.prep(DistId::Zero, 4, false).unwrap();
    // observe() flushes buffer -> synthesize -> submit -> compile -> ERROR
    let err = cb.observe(h, ObserveMode::Dist, 0, 0);
    assert!(
        matches!(err, Err(CqamError::QpuQubitAllocationFailed { required: 4, available: 3 })),
        "Expected QpuQubitAllocationFailed{{required:4, available:3}}, got: {:?}", err
    );
}

// =============================================================================
// Test 4: Qubit limit exceeded via QPREPN (VM-level check)
// =============================================================================

/// QPREPN with a count exceeding max_qubits returns `CqamError::QubitLimitExceeded`
/// at the VM level, before the circuit is even assembled.
///
/// The mock QPU backend has 27 qubits. Using 30 exceeds the limit.
#[test]
fn test_qubit_limit_via_qprepn() {
    let program = vec![
        // R0 = 30 (exceeds MockQpuBackend default max of 27)
        Instruction::ILdi { dst: 0, imm: 30 },
        Instruction::QPrepN { dst: 0, dist: DistId::Zero, qubit_count_reg: 0 },
        Instruction::Halt,
    ];
    let err = run_program_with_config(program, &mock_qpu_config());
    assert!(
        matches!(err, Err(CqamError::QubitLimitExceeded { ref instruction, .. })
            if instruction == "QPREPN"),
        "Expected QubitLimitExceeded(QPREPN), got: {:?}", err.err()
    );
}

// =============================================================================
// Test 5: Non-adjacent CX on linear topology inserts SWAPs (success case)
// =============================================================================

/// A CX gate on non-adjacent qubits (0 and 3) in a linear-4 topology is
/// handled by the routing pass, which inserts SWAP gates. The circuit
/// compiles and executes successfully — this is NOT an error.
#[test]
fn test_connectivity_violation_inserts_swaps() {
    let mut cb = make_linear_backend(4);
    // Prepare 4-qubit zero state
    let (h, _) = cb.prep(DistId::Zero, 4, false).unwrap();
    // Apply CX(0, 3) — qubits 0 and 3 are not adjacent on linear(4)
    // The routing pass will insert SWAPs along the path 0->1->2->3
    let (h2, _) = cb.apply_two_qubit_gate(h, 0, 3, &cx_gate()).unwrap();
    // observe() triggers routing + compilation + submission
    let result = cb.observe(h2, ObserveMode::Dist, 0, 0);
    assert!(
        result.is_ok(),
        "CX on non-adjacent qubits should succeed via SWAP insertion; got: {:?}", result
    );
    assert!(
        matches!(result.unwrap(), ObserveResult::Dist(_)),
        "Result should be a Dist distribution"
    );
}

// =============================================================================
// Test 6: Empty circuit (QPREP -> QOBSERVE, no gates) works correctly
// =============================================================================

/// A QPREP immediately followed by QOBSERVE (no gates applied) should
/// produce a valid Dist with all probability mass on bitstring 0 (|0...0>).
///
/// This exercises the minimal circuit path and verifies the zero state is
/// correctly propagated through the compile/submit pipeline.
#[test]
fn test_empty_circuit_qprep_qobserve() {
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];
    let result = run_program_with_config(program, &mock_qpu_config()).unwrap();
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt cleanly");

    let h0 = ctx.hregs.get(0).unwrap();
    if let HybridValue::Dist(entries) = h0 {
        assert!(!entries.is_empty(), "Empty-circuit Dist must have at least one entry");
        let p0: f64 = entries.iter()
            .filter(|(bs, _)| *bs == 0)
            .map(|(_, p)| *p)
            .sum();
        assert!(
            p0 > 0.95,
            "Zero state (no gates) should have P(|0>)>0.95; got {:.4}", p0
        );
    } else {
        panic!("H0 should be HybridValue::Dist; got {:?}", h0);
    }
}

// =============================================================================
// Test 7: QMEAS mid-circuit inserts op and returns dummy outcome 0
// =============================================================================

/// `CircuitBackend::measure_qubit()` inserts a `MeasQubit` op into the circuit
/// IR and returns a dummy measurement outcome of 0 (the real measurement is
/// deferred to the QPU at observe time).
///
/// The key behaviors verified:
/// - `measure_qubit` does not return an error
/// - The dummy outcome is 0
/// - `observe()` can still be called after a mid-circuit measurement
#[test]
fn test_qmeas_mid_circuit_returns_dummy_zero() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let (h2, meas_result) = cb.measure_qubit(h, 0).unwrap();
    // Dummy outcome must be 0 (see circuit_backend.rs known limitations)
    assert_eq!(
        meas_result.outcome, 0,
        "mid-circuit measurement must return dummy outcome 0; got {}",
        meas_result.outcome
    );
    // Can still observe after a mid-circuit measurement
    let observe_result = cb.observe(h2, ObserveMode::Dist, 0, 0);
    assert!(
        observe_result.is_ok(),
        "observe() after mid-circuit QMEAS should succeed; got: {:?}", observe_result
    );
}

/// Mid-circuit measurement does not panic and the result is still a Dist.
#[test]
fn test_qmeas_mid_circuit_observe_is_dist() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let gate = h_gate();
    let (h2, _) = cb.apply_single_gate(h, 0, &gate).unwrap();
    let (h3, _) = cb.measure_qubit(h2, 0).unwrap();
    let result = cb.observe(h3, ObserveMode::Dist, 0, 0).unwrap();
    assert!(
        matches!(result, ObserveResult::Dist(_)),
        "observe() after mid-circuit QMEAS must return Dist; got: {:?}", result
    );
}

// =============================================================================
// Test 8: clone_state on evolved handle fails (no-cloning theorem)
// =============================================================================

/// `CircuitBackend::clone_state()` on a handle that has had a gate applied
/// returns `CqamError::QpuUnsupportedOperation { operation: "clone_state", .. }`.
///
/// The no-cloning restriction is enforced because an evolved quantum state
/// cannot be duplicated without running the full circuit again.
#[test]
fn test_clone_state_evolved_fails() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let gate = h_gate();
    // apply_single_gate marks `h` as evolved
    let (_h2, _) = cb.apply_single_gate(h, 0, &gate).unwrap();
    // Attempt to clone the original (evolved) handle
    let err = cb.clone_state(h);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { ref operation, .. })
            if operation == "clone_state"),
        "Expected QpuUnsupportedOperation(clone_state) for evolved handle; got: {:?}", err
    );
}

/// `clone_state()` on an unevolved (freshly prepped) handle succeeds.
#[test]
fn test_clone_state_unevolved_succeeds() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let h2 = cb.clone_state(h);
    assert!(
        h2.is_ok(),
        "clone_state on unevolved handle should succeed; got: {:?}", h2
    );
}

// =============================================================================
// Test 9: Bell pair budget exhaustion
// =============================================================================

/// With `bell_pair_budget = 1`, the first QSTORE consumes the only available
/// Bell pair (budget goes to 0). The subsequent QLOAD finds budget = 0 and
/// returns `CqamError::BellPairExhausted { instruction: "QLOAD" }`.
///
/// The VM logic in qop.rs is:
///   - QSTORE: check `ctx.bell_pair_budget == 0 && ctx.config.bell_pair_budget != 0`
///   - If false, decrement and proceed.
///   - QLOAD: same check.
/// With budget=1: QSTORE decrements to 0 (succeeds), QLOAD sees 0 → error.
#[test]
fn test_bell_pair_budget_exhaustion_at_qload() {
    // budget=1: QSTORE uses it (success), QLOAD has none left (error)
    let config = mock_qpu_config_with_bell_budget(1);
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QStore { src_q: 0, addr: 0 },  // uses 1 bell pair (0 remaining)
        Instruction::QLoad { dst_q: 1, addr: 0 },   // budget exhausted -> error
        Instruction::Halt,
    ];
    let err = run_program_with_config(program, &config);
    assert!(
        matches!(err, Err(CqamError::BellPairExhausted { ref instruction })
            if instruction == "QLOAD"),
        "Expected BellPairExhausted(QLOAD), got: {:?}", err.err()
    );
}

/// With `bell_pair_budget = 0` (unlimited), QSTORE and QLOAD both succeed
/// without exhausting any budget.
#[test]
fn test_bell_pair_unlimited_budget_succeeds() {
    // budget=0 means unlimited (0 = no limit in VmConfig semantics)
    let config = mock_qpu_config_with_bell_budget(0);
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QStore { src_q: 0, addr: 0 },  // no budget check when budget=0
        Instruction::QLoad { dst_q: 0, addr: 0 },   // no budget check
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];
    let result = run_program_with_config(program, &config);
    assert!(
        result.is_ok(),
        "Unlimited budget (0) should not exhaust; got: {:?}", result.err()
    );
}

// =============================================================================
// Test 10: QpuShotBudgetExhausted error type formatting
// =============================================================================

/// `CqamError::QpuShotBudgetExhausted` formats its `budget` and `used` fields
/// correctly in the Display representation.
///
/// NOTE: The mock backend does NOT raise this error at runtime — it exits its
/// shot-sampling loop gracefully when the budget runs out. This test validates
/// that the error type is well-formed and displays correctly, so that future
/// QPU backends that do raise it will integrate correctly.
#[test]
fn test_shot_budget_exhausted_error_type_display() {
    let err = CqamError::QpuShotBudgetExhausted { budget: 100, used: 100 };
    let msg = format!("{}", err);
    assert!(msg.contains("100"), "Display must include the budget value");
    assert!(
        msg.contains("budget") || msg.contains("exhausted"),
        "Display must indicate budget exhaustion; got: {}", msg
    );
}

/// Debug representation of `QpuShotBudgetExhausted` is well-formed.
#[test]
fn test_shot_budget_exhausted_error_type_debug() {
    let err = CqamError::QpuShotBudgetExhausted { budget: 8192, used: 8192 };
    let msg = format!("{:?}", err);
    assert!(msg.contains("8192"), "Debug must include budget value");
}

// =============================================================================
// Test 11: prep_from_amplitudes unsupported (QENCODE)
// =============================================================================

/// `CircuitBackend::prep_from_amplitudes()` returns
/// `CqamError::QpuUnsupportedOperation { operation: "QENCODE", .. }`.
///
/// Arbitrary amplitude encoding requires a statevector preparation step that
/// cannot be expressed as a circuit from first principles in the current
/// compilation pipeline.
#[test]
fn test_prep_from_amplitudes_unsupported() {
    let mut cb = make_test_backend(8);
    let amps = [C64(1.0, 0.0), C64(0.0, 0.0)];
    let err = cb.prep_from_amplitudes(&amps);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { ref operation, .. })
            if operation == "QENCODE"),
        "Expected QpuUnsupportedOperation(QENCODE); got: {:?}", err
    );
}

// =============================================================================
// Test 12: partial_trace unsupported (QPTRACE)
// =============================================================================

/// `CircuitBackend::partial_trace()` returns
/// `CqamError::QpuUnsupportedOperation { operation: "QPTRACE", .. }`.
///
/// Partial trace requires density-matrix operations that are not available
/// in the circuit IR. Tracing out subsystems is not expressible as a gate.
#[test]
fn test_partial_trace_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let err = cb.partial_trace(h, 1);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { ref operation, .. })
            if operation == "QPTRACE"),
        "Expected QpuUnsupportedOperation(QPTRACE); got: {:?}", err
    );
}

// =============================================================================
// Test 13: State inspection methods unsupported
// =============================================================================

/// All five state inspection methods on `CircuitBackend` return
/// `CqamError::QpuUnsupportedOperation`.
///
/// In circuit mode, the quantum state is implicit in the circuit buffer and
/// is only materialized at observe time by the QPU backend. Direct state
/// inspection is not possible.
#[test]
fn test_purity_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let err = cb.purity(h);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { .. })),
        "purity() must return QpuUnsupportedOperation; got: {:?}", err
    );
}

#[test]
fn test_is_pure_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let err = cb.is_pure(h);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { .. })),
        "is_pure() must return QpuUnsupportedOperation; got: {:?}", err
    );
}

#[test]
fn test_diagonal_probabilities_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let err = cb.diagonal_probabilities(h);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { .. })),
        "diagonal_probabilities() must return QpuUnsupportedOperation; got: {:?}", err
    );
}

#[test]
fn test_get_element_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let err = cb.get_element(h, 0, 0);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { .. })),
        "get_element() must return QpuUnsupportedOperation; got: {:?}", err
    );
}

#[test]
fn test_amplitude_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    let err = cb.amplitude(h, 0);
    assert!(
        matches!(err, Err(CqamError::QpuUnsupportedOperation { .. })),
        "amplitude() must return QpuUnsupportedOperation; got: {:?}", err
    );
}

/// All five state inspection methods return errors in a single combined test.
#[test]
fn test_all_state_inspection_methods_unsupported() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    assert!(cb.purity(h).is_err(), "purity() must be unsupported");
    assert!(cb.is_pure(h).is_err(), "is_pure() must be unsupported");
    assert!(cb.diagonal_probabilities(h).is_err(), "diagonal_probabilities() must be unsupported");
    assert!(cb.get_element(h, 0, 0).is_err(), "get_element() must be unsupported");
    assert!(cb.amplitude(h, 0).is_err(), "amplitude() must be unsupported");
}

// =============================================================================
// Additional edge cases found during implementation
// =============================================================================

/// An invalid handle (never allocated) returns `UninitializedRegister` on
/// observe, not a panic.
///
/// BUG CHECK: ensure error paths don't panic when given stale handles.
#[test]
fn test_stale_handle_returns_error_not_panic() {
    use cqam_core::quantum_backend::QRegHandle;
    let mut cb = make_test_backend(8);
    // Construct a handle ID that was never allocated
    let stale_handle = QRegHandle(9999);
    let err = cb.observe(stale_handle, ObserveMode::Dist, 0, 0);
    assert!(
        err.is_err(),
        "observe() on a stale handle must return Err, not panic; got: {:?}", err
    );
}

/// A handle consumed by `observe()` cannot be re-observed.
///
/// BUG CHECK: ensure the second observe returns an error, not a panic.
#[test]
fn test_double_observe_returns_error_not_panic() {
    let mut cb = make_test_backend(8);
    let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
    // First observe consumes the handle
    let _ = cb.observe(h, ObserveMode::Dist, 0, 0).unwrap();
    // Second observe on the same (now released) handle
    let err = cb.observe(h, ObserveMode::Dist, 0, 0);
    assert!(
        err.is_err(),
        "Second observe on consumed handle must return Err; got: {:?}", err
    );
}

/// Qubit index out of range returns `QuantumIndexOutOfRange`, not a panic.
///
/// BUG CHECK: gate applied to qubit index >= register size should be an error.
#[test]
fn test_out_of_range_qubit_returns_error() {
    let mut cb = make_test_backend(8);
    // Prepare a 2-qubit register (valid qubits: 0, 1)
    let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
    // Attempt to apply a gate to qubit 5 (out of range)
    let gate = h_gate();
    let err = cb.apply_single_gate(h, 5, &gate);
    assert!(
        matches!(err, Err(CqamError::QuantumIndexOutOfRange { .. })),
        "Out-of-range qubit must return QuantumIndexOutOfRange; got: {:?}", err
    );
}

/// BellPairExhausted at QSTORE (budget=0 after being set to finite, then
/// immediately reached): with budget=0 as the configured limit, QSTORE fails.
///
/// NOTE: `bell_pair_budget = 0` in SimConfig means *unlimited* (no budget).
/// To get BellPairExhausted at QSTORE, we need the runtime counter to have
/// decremented to 0, which requires a finite budget that is already used up.
///
/// This tests with budget=1 and verifies QSTORE itself succeeds (it is QLOAD
/// that fails — see test_bell_pair_budget_exhaustion_at_qload above).
/// A separate test verifies the QSTORE path by using budget=0 at the VM level.
///
/// Bug identified: the VM budget check uses `ctx.bell_pair_budget == 0 &&
/// ctx.config.bell_pair_budget != 0`. To get BellPairExhausted at QSTORE, we
/// would need `ctx.bell_pair_budget` to already be 0 before the QSTORE call.
/// This is impossible via QSTORE alone when starting from a finite budget
/// because QSTORE itself decrements *after* the check. Therefore, the only way
/// to exhaust the budget at QSTORE is if a prior QSTORE or QLOAD used it up.
///
/// The following test confirms this two-step exhaustion scenario.
#[test]
fn test_bell_pair_exhaustion_at_second_qstore() {
    // budget=1: first QSTORE succeeds (1->0), second QSTORE sees budget=0 -> error
    let config = mock_qpu_config_with_bell_budget(1);
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QPrep { dst: 1, dist: DistId::Zero },
        Instruction::QStore { src_q: 0, addr: 0 },  // uses 1 bell pair (0 remaining)
        Instruction::QStore { src_q: 1, addr: 1 },  // budget=0 -> BellPairExhausted
        Instruction::Halt,
    ];
    let err = run_program_with_config(program, &config);
    assert!(
        matches!(err, Err(CqamError::BellPairExhausted { ref instruction })
            if instruction == "QSTORE"),
        "Expected BellPairExhausted(QSTORE) on second store; got: {:?}", err.err()
    );
}
