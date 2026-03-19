//! Integration tests for the mock QPU backend via `cqam-run`.
//!
//! These tests verify that the full execution pipeline works correctly when
//! using `BackendChoice::Qpu { provider: "mock", .. }`. Each test constructs
//! a short CQAM program, runs it via the runner with the mock backend, and
//! asserts quantum-mechanical correctness of the output distribution.

use cqam_core::instruction::{DistId, Instruction, ObserveMode};
use cqam_core::register::HybridValue;
use cqam_run::runner::run_program_with_config;
use cqam_run::simconfig::{BackendChoice, SimConfig};

// =============================================================================
// Helpers
// =============================================================================

/// Build a SimConfig that routes execution through the mock QPU backend.
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

/// Build a SimConfig that uses the default simulation backend.
/// Used to verify the default path is unchanged by Phase 4.
fn sim_config() -> SimConfig {
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
        backend: None,
        ibm_token: None,
        ibm_optimization_level: None,
    }
}

// =============================================================================
// BackendChoice unit tests
// =============================================================================

#[test]
fn test_backend_choice_default_is_simulation() {
    let config = SimConfig::default();
    assert!(matches!(config.backend_choice(), BackendChoice::Simulation));
}

#[test]
fn test_backend_choice_mock_constructed() {
    let choice = BackendChoice::Qpu {
        provider: "mock".to_string(),
        device: None,
        shot_budget: 1024,
        confidence: 0.99,
    };
    assert!(matches!(choice, BackendChoice::Qpu { .. }));
}

#[test]
fn test_backend_choice_via_config() {
    let config = mock_qpu_config();
    assert!(matches!(config.backend_choice(), BackendChoice::Qpu { .. }));
}

#[test]
fn test_unknown_provider_returns_config_error() {
    let mut config = mock_qpu_config();
    config.backend = Some(BackendChoice::Qpu {
        provider: "nonexistent".to_string(),
        device: None,
        shot_budget: 1000,
        confidence: 0.95,
    });
    let program = vec![Instruction::Halt];
    let result = run_program_with_config(program, &config);
    assert!(result.is_err(), "Unknown provider must return ConfigError");
    let err = format!("{:?}", result.err().unwrap());
    assert!(err.contains("nonexistent") || err.contains("unknown QPU provider"),
        "Error must mention the unknown provider; got: {}", err);
}

// =============================================================================
// Quantum correctness tests (mock QPU)
// =============================================================================

/// QPREP + QOBSERVE(Dist) on |0> state.
///
/// After preparing the zero state and observing, H0 must be a Dist with
/// a single outcome (bitstring 0) at probability ≈ 1.0.
#[test]
fn test_zero_state_mock_qpu() {
    let program = vec![
        // QPREP Q0 with DistId::Zero → |0> state
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        // QOBSERVE H0 = Dist over Q0
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let result = run_program_with_config(program, &mock_qpu_config()).unwrap();
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt");

    let h0 = ctx.hregs.get(0).unwrap();
    if let HybridValue::Dist(entries) = h0 {
        assert!(!entries.is_empty(), "Dist must have at least one entry");
        // All probability mass should be on bitstring 0 (the |0> state)
        let total_prob: f64 = entries.iter().map(|(_, p)| p).sum();
        assert!((total_prob - 1.0).abs() < 0.05,
            "Total probability must sum to ~1.0; got {:.4}", total_prob);
        let p0: f64 = entries.iter()
            .filter(|(bs, _)| *bs == 0)
            .map(|(_, p)| *p)
            .sum();
        assert!(p0 > 0.9, "Zero state should produce |0> with probability > 0.9; got {:.4}", p0);
    } else {
        panic!("H0 should be HybridValue::Dist after QPU observe; got {:?}", h0);
    }
}

/// Single-qubit Hadamard + QOBSERVE(Dist) → ~50/50 distribution.
///
/// With a 2-qubit register (config: default_qubits=2), applying H to qubit 0
/// (mask=1, bit 0 set) on |00> gives (|00> + |10>) / sqrt(2).
/// In the 2-qubit computational basis, bitstring 0 = |00> and bitstring 2 = |10>.
/// Both should appear with probability ≈ 0.5.
#[test]
fn test_hadamard_mock_qpu() {
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        // R1 = 1 (bitmask: bit 0 set → apply H to qubit 0 only)
        Instruction::ILdi { dst: 1, imm: 1 },
        // QHadM: apply H to each qubit where the corresponding mask bit is 1
        // On |00>, H on qubit 0 gives (|00> + |10>) / sqrt(2)
        Instruction::QHadM { dst: 1, src: 0, mask_reg: 1 },
        Instruction::QObserve { dst_h: 0, src_q: 1, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let result = run_program_with_config(program, &mock_qpu_config()).unwrap();
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt");

    let h0 = ctx.hregs.get(0).unwrap();
    if let HybridValue::Dist(entries) = h0 {
        assert!(!entries.is_empty(), "Dist must have entries");
        // For 2-qubit register: H on qubit 0 gives |00>(bs=0) and |10>(bs=2)
        let p0: f64 = entries.iter().filter(|(bs, _)| *bs == 0).map(|(_, p)| *p).sum();
        let p2: f64 = entries.iter().filter(|(bs, _)| *bs == 2).map(|(_, p)| *p).sum();
        // With 8192 shots and Bayesian estimator, expect both within [0.35, 0.65]
        assert!(p0 > 0.35 && p0 < 0.65,
            "H on qubit 0 of |00>: P(|00|)=bs0 should be ≈0.5; got P(bs0)={:.4}, P(bs2)={:.4}", p0, p2);
        assert!(p2 > 0.35 && p2 < 0.65,
            "H on qubit 0 of |00>: P(|10|)=bs2 should be ≈0.5; got P(bs0)={:.4}, P(bs2)={:.4}", p0, p2);
    } else {
        panic!("H0 should be HybridValue::Dist after QPU observe; got {:?}", h0);
    }
}

/// Bell state (H + CNOT) via mock QPU → only |00> and |11> appear.
///
/// QPREP(2-qubit zero) + H(qubit 0) + CNOT(ctrl=0, tgt=1) creates
/// the Bell state |Φ+> = (|00> + |11>) / sqrt(2).
#[test]
fn test_bell_state_mock_qpu() {
    let program = vec![
        // Prepare 2-qubit zero state
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        // R1 = 1 (bitmask for qubit 0 only)
        Instruction::ILdi { dst: 1, imm: 1 },
        // Apply H to qubit 0
        Instruction::QHadM { dst: 1, src: 0, mask_reg: 1 },
        // R2 = 0 (ctrl qubit), R3 = 1 (tgt qubit)
        Instruction::ILdi { dst: 2, imm: 0 },
        Instruction::ILdi { dst: 3, imm: 1 },
        // Apply CNOT(ctrl=Q[R2], tgt=Q[R3])
        Instruction::QCnot { dst: 2, src: 1, ctrl_qubit_reg: 2, tgt_qubit_reg: 3 },
        // Observe
        Instruction::QObserve { dst_h: 0, src_q: 2, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let result = run_program_with_config(program, &mock_qpu_config()).unwrap();
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt");

    let h0 = ctx.hregs.get(0).unwrap();
    if let HybridValue::Dist(entries) = h0 {
        assert!(!entries.is_empty(), "Bell state Dist must have entries");
        // Only bitstrings 0 (|00>) and 3 (|11>) should appear
        for (bs, prob) in entries {
            assert!(
                *bs == 0 || *bs == 3,
                "Bell state should only produce |00>=0 or |11>=3; got bitstring {} with prob {:.4}",
                bs, prob
            );
        }
        let p00: f64 = entries.iter().filter(|(bs, _)| *bs == 0).map(|(_, p)| *p).sum();
        let p11: f64 = entries.iter().filter(|(bs, _)| *bs == 3).map(|(_, p)| *p).sum();
        assert!(p00 > 0.35 && p00 < 0.65,
            "Bell state: P(|00>) should be ≈0.5; got {:.4}", p00);
        assert!(p11 > 0.35 && p11 < 0.65,
            "Bell state: P(|11>) should be ≈0.5; got {:.4}", p11);
    } else {
        panic!("H0 should be HybridValue::Dist after QPU observe; got {:?}", h0);
    }
}

/// Multi-section: two independent QPREP→QOBSERVE blocks.
///
/// Each block creates its own circuit submission. Both H registers must
/// contain valid Dist results after execution.
#[test]
fn test_multi_section_mock_qpu() {
    let program = vec![
        // Section 1: zero state
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        // Section 2: uniform state
        Instruction::QPrep { dst: 0, dist: DistId::Uniform },
        Instruction::QObserve { dst_h: 1, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let result = run_program_with_config(program, &mock_qpu_config()).unwrap();
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt");

    // H0: zero state measurement
    let h0 = ctx.hregs.get(0).unwrap();
    assert!(matches!(h0, HybridValue::Dist(_)), "H0 must be Dist from section 1");

    // H1: uniform state measurement (should show both |0> and |1>)
    let h1 = ctx.hregs.get(1).unwrap();
    if let HybridValue::Dist(entries) = h1 {
        assert!(!entries.is_empty(), "H1 (uniform state) Dist must have entries");
        // Uniform state on n qubits should produce multiple bitstrings
        let total_prob: f64 = entries.iter().map(|(_, p)| p).sum();
        assert!((total_prob - 1.0).abs() < 0.05,
            "H1 probabilities should sum to ~1.0; got {:.4}", total_prob);
    } else {
        panic!("H1 should be HybridValue::Dist; got {:?}", h1);
    }
}

/// QOBSERVE with SAMPLE mode via mock QPU.
///
/// After observing in Sample mode, H0 should contain an Int (a single
/// collapsed measurement outcome), not a Dist.
#[test]
fn test_sample_mode_mock_qpu() {
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Sample, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let result = run_program_with_config(program, &mock_qpu_config()).unwrap();
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt");

    let h0 = ctx.hregs.get(0).unwrap();
    assert!(matches!(h0, HybridValue::Int(_)),
        "SAMPLE mode should produce HybridValue::Int; got {:?}", h0);

    // Zero state: sampled value should be 0
    if let HybridValue::Int(v) = h0 {
        assert_eq!(*v, 0, "Zero state sampled in SAMPLE mode must yield 0");
    }
}

// =============================================================================
// Simulation path unchanged tests
// =============================================================================

/// The default Simulation backend path must produce identical results to
/// pre-Phase-4 behavior (regression guard).
#[test]
fn test_simulation_path_unchanged() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 7 },
        Instruction::ILdi { dst: 1, imm: 3 },
        Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 },
        Instruction::Halt,
    ];

    let result = run_program_with_config(program, &sim_config()).unwrap();
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt");
    assert_eq!(ctx.iregs.get(2).unwrap(), 10, "R2 should be 7+3=10");
}

/// Simulation path with QPREP+QOBSERVE must still work unchanged.
#[test]
fn test_simulation_quantum_path_unchanged() {
    let program = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let result = run_program_with_config(program, &sim_config()).unwrap();
    let ctx = result.ctx();

    assert!(ctx.psw.trap_halt, "Program should halt");
    let h0 = ctx.hregs.get(0).unwrap();
    assert!(matches!(h0, HybridValue::Dist(_)),
        "Simulation path should produce Dist; got {:?}", h0);
}

// =============================================================================
// CLI flag parsing tests (via SimConfig construction)
// =============================================================================

#[test]
fn test_backend_flag_simulation_is_default() {
    let config = SimConfig {
        backend: None,
        ..SimConfig::default()
    };
    assert!(matches!(config.backend_choice(), BackendChoice::Simulation));
}

#[test]
fn test_backend_flag_explicit_simulation() {
    let config = SimConfig {
        backend: Some(BackendChoice::Simulation),
        ..SimConfig::default()
    };
    assert!(matches!(config.backend_choice(), BackendChoice::Simulation));
}

#[test]
fn test_qpu_shots_and_confidence_stored() {
    let choice = BackendChoice::Qpu {
        provider: "mock".to_string(),
        device: Some("test_device".to_string()),
        shot_budget: 4096,
        confidence: 0.99,
    };
    if let BackendChoice::Qpu { shot_budget, confidence, ref device, .. } = choice {
        assert_eq!(shot_budget, 4096);
        assert!((confidence - 0.99).abs() < 1e-10);
        assert_eq!(device.as_deref(), Some("test_device"));
    }
}

#[test]
fn test_noise_model_with_simulation_still_works() {
    // Noise model should work fine when using the Simulation backend
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::Halt,
    ];
    let config = SimConfig {
        noise_model: None, // No noise, just verifying config path
        backend: None,     // Default Simulation
        ..SimConfig::default()
    };
    let result = run_program_with_config(program, &config).unwrap();
    assert_eq!(result.ctx().iregs.get(0).unwrap(), 42);
}
