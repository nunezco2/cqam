//! Tests for QPREPS and QPREPSM instruction execution.
//!
//! Covers: alpha_beta_to_u3 conversion, normalization warning, zero-amplitude
//! trap, basic state preparation correctness, and PSW flag semantics.

use cqam_core::complex::C64;
use cqam_core::instruction::*;
use cqam_core::quantum_backend::QuantumBackend;
use cqam_sim::backend::SimulationBackend;
use cqam_vm::context::ExecutionContext;
use cqam_vm::qop::execute_qop;

// =============================================================================
// Helpers
// =============================================================================

fn test_backend() -> SimulationBackend {
    SimulationBackend::new()
}

/// Prepare a 3-qubit zero-state register in Q0.
fn prep_zero_3q(ctx: &mut ExecutionContext, backend: &mut SimulationBackend) {
    let config = ctx.config.clone();
    let (handle, _) = backend.prep(DistId::Zero, 3, false).unwrap();
    ctx.set_qreg(0, handle, backend);
    ctx.psw.qf = true;
    let _ = config;
}

// =============================================================================
// alpha_beta_to_u3 conversion tests (via SimulationBackend::prep_product_state)
// =============================================================================

/// State |0> means alpha=(1,0), beta=(0,0).
/// U3(0, 0, 0) = Identity. The statevector should remain |000...>.
#[test]
fn test_prep_product_state_zero_state() {
    let mut backend = test_backend();
    let (handle, _) = backend.prep(DistId::Zero, 1, false).unwrap();

    // alpha = 1, beta = 0 → |0>
    let amplitudes = vec![(C64(1.0, 0.0), C64(0.0, 0.0))];
    let (new_handle, _) = backend.prep_product_state(handle, &amplitudes).unwrap();

    let probs = backend.diagonal_probabilities(new_handle).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-10, "|0> should have prob=1 in state 0");
    assert!(probs[1].abs() < 1e-10, "|1> should have prob=0");
}

/// State |1> means alpha=(0,0), beta=(1,0).
/// U3(π, 0, 0) = X gate. Starting from |0>, result should be |1>.
#[test]
fn test_prep_product_state_one_state() {
    let mut backend = test_backend();
    let (handle, _) = backend.prep(DistId::Zero, 1, false).unwrap();

    // alpha = 0, beta = 1 → |1>
    let amplitudes = vec![(C64(0.0, 0.0), C64(1.0, 0.0))];
    let (new_handle, _) = backend.prep_product_state(handle, &amplitudes).unwrap();

    let probs = backend.diagonal_probabilities(new_handle).unwrap();
    assert!(probs[0].abs() < 1e-10, "|0> should have prob=0");
    assert!((probs[1] - 1.0).abs() < 1e-10, "|1> should have prob=1");
}

/// State |+> = (|0>+|1>)/sqrt(2) means alpha=beta=1/sqrt(2).
/// Should give 50% probability for each outcome.
#[test]
fn test_prep_product_state_plus_state() {
    let mut backend = test_backend();
    let (handle, _) = backend.prep(DistId::Zero, 1, false).unwrap();

    let h = std::f64::consts::FRAC_1_SQRT_2;
    let amplitudes = vec![(C64(h, 0.0), C64(h, 0.0))];
    let (new_handle, _) = backend.prep_product_state(handle, &amplitudes).unwrap();

    let probs = backend.diagonal_probabilities(new_handle).unwrap();
    assert!((probs[0] - 0.5).abs() < 1e-10, "|0> should have prob=0.5");
    assert!((probs[1] - 0.5).abs() < 1e-10, "|1> should have prob=0.5");
}

/// State with arbitrary complex phase: alpha=(0.6, 0.0), beta=(0.0, 0.8).
/// |alpha|^2 + |beta|^2 = 0.36 + 0.64 = 1.0.
/// Probability of |0> = |alpha|^2 = 0.36, probability of |1> = |beta|^2 = 0.64.
#[test]
fn test_prep_product_state_complex_phase() {
    let mut backend = test_backend();
    let (handle, _) = backend.prep(DistId::Zero, 1, false).unwrap();

    let amplitudes = vec![(C64(0.6, 0.0), C64(0.0, 0.8))];
    let (new_handle, _) = backend.prep_product_state(handle, &amplitudes).unwrap();

    let probs = backend.diagonal_probabilities(new_handle).unwrap();
    assert!((probs[0] - 0.36).abs() < 1e-10, "|0> prob should be 0.36, got {}", probs[0]);
    assert!((probs[1] - 0.64).abs() < 1e-10, "|1> prob should be 0.64, got {}", probs[1]);
}

// =============================================================================
// QPREPS instruction tests (register-direct)
// =============================================================================

/// QPREPS with all-|0> state should leave the register in |000>.
#[test]
fn test_qpreps_all_zero_state() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    // Z0 = alpha for qubit 0, Z1 = beta for qubit 0
    // Z2 = alpha for qubit 1, Z3 = beta for qubit 1
    // Z4 = alpha for qubit 2, Z5 = beta for qubit 2
    // All in |0>: alpha=1, beta=0
    ctx.zregs.set(0, (1.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();
    ctx.zregs.set(2, (1.0, 0.0)).unwrap();
    ctx.zregs.set(3, (0.0, 0.0)).unwrap();
    ctx.zregs.set(4, (1.0, 0.0)).unwrap();
    ctx.zregs.set(5, (0.0, 0.0)).unwrap();

    let instr = Instruction::QPreps { dst: 0, z_start: 0, count: 3 };
    execute_qop(&mut ctx, &instr, &mut backend).unwrap();

    let handle = ctx.qregs[0].unwrap();
    let probs = backend.diagonal_probabilities(handle).unwrap();
    // |000> state: only index 0 has probability 1
    assert!((probs[0] - 1.0).abs() < 1e-10, "|000> should have prob=1, got {}", probs[0]);
    for i in 1..8 {
        assert!(probs[i].abs() < 1e-10, "State |{:03b}> should have prob=0, got {}", i, probs[i]);
    }

    // PSW: sf=false (no beta nonzero), ef=false
    assert!(!ctx.psw.sf, "sf should be false for all-|0> state");
    assert!(!ctx.psw.ef, "ef should be false");
    assert!(!ctx.psw.norm_warn, "no normalization warning expected");
}

/// QPREPS with qubit 0 in |+> state.
/// Expected: qubit 0 is |+>, qubits 1 and 2 are |0>.
/// Register state: (|000> + |100>) / sqrt(2) (big-endian: qubit0 is MSB)
#[test]
fn test_qpreps_plus_state_qubit0() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    let h = std::f64::consts::FRAC_1_SQRT_2;
    ctx.zregs.set(0, (h, 0.0)).unwrap();  // alpha for qubit 0
    ctx.zregs.set(1, (h, 0.0)).unwrap();  // beta for qubit 0

    let instr = Instruction::QPreps { dst: 0, z_start: 0, count: 1 };
    execute_qop(&mut ctx, &instr, &mut backend).unwrap();

    let handle = ctx.qregs[0].unwrap();
    let probs = backend.diagonal_probabilities(handle).unwrap();
    // With 3 qubits, qubit 0 = MSB. |+> on qubit 0 → |000> and |100> each at 50%
    // |000> = index 0, |100> = index 4 (big-endian: bit2=q0, bit1=q1, bit0=q2)
    assert!((probs[0] - 0.5).abs() < 1e-10, "|000> prob should be 0.5, got {}", probs[0]);
    assert!((probs[4] - 0.5).abs() < 1e-10, "|100> prob should be 0.5, got {}", probs[4]);

    // PSW: sf=true because beta is nonzero
    assert!(ctx.psw.sf, "sf should be true when beta is nonzero");
    assert!(!ctx.psw.ef, "ef should be false (product state)");
}

/// QPREPS should set norm_warn when amplitudes are not normalized.
#[test]
fn test_qpreps_normalization_warning() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    // Unnormalized: |alpha|^2 + |beta|^2 = 1.5 (should be 1.0)
    // alpha = 1.0, beta = 0.7071 → norm_sq ≈ 1.5
    ctx.zregs.set(0, (1.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.7071, 0.0)).unwrap();

    let instr = Instruction::QPreps { dst: 0, z_start: 0, count: 1 };
    execute_qop(&mut ctx, &instr, &mut backend).unwrap();

    assert!(ctx.psw.norm_warn, "norm_warn should be set for unnormalized amplitudes");
}

/// QPREPS should trigger trap_arith for zero (0,0) amplitudes.
#[test]
fn test_qpreps_zero_amplitude_trap() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    // alpha = 0, beta = 0 → arithmetic trap
    ctx.zregs.set(0, (0.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    let instr = Instruction::QPreps { dst: 0, z_start: 0, count: 1 };
    let result = execute_qop(&mut ctx, &instr, &mut backend);
    assert!(result.is_err(), "Should error on zero amplitude");
    assert!(ctx.psw.trap_arith, "trap_arith should be set");
}

/// QPREPS with uninitialized Q register should return an error.
#[test]
fn test_qpreps_uninitialized_register() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Q0 has no handle
    ctx.zregs.set(0, (1.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    let instr = Instruction::QPreps { dst: 0, z_start: 0, count: 1 };
    let result = execute_qop(&mut ctx, &instr, &mut backend);
    assert!(result.is_err(), "Should error when Q register is uninitialized");
}

// =============================================================================
// QPREPSM instruction tests (CMEM-indirect)
// =============================================================================

/// Helper: write a (alpha, beta) pair as 4 CMEM cells starting at addr.
fn write_qstate_to_cmem(ctx: &mut ExecutionContext, base: u16, re_a: f64, im_a: f64, re_b: f64, im_b: f64) {
    ctx.cmem.store(base,     re_a.to_bits() as i64);
    ctx.cmem.store(base + 1, im_a.to_bits() as i64);
    ctx.cmem.store(base + 2, re_b.to_bits() as i64);
    ctx.cmem.store(base + 3, im_b.to_bits() as i64);
}

/// QPREPSM reading from CMEM should produce same result as QPREPS.
#[test]
fn test_qprepsm_zero_state() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    // Write |0> state for 1 qubit at CMEM[100]
    write_qstate_to_cmem(&mut ctx, 100, 1.0, 0.0, 0.0, 0.0);

    // R0 = 100 (base), R1 = 1 (count)
    ctx.iregs.set(0, 100).unwrap();
    ctx.iregs.set(1, 1).unwrap();

    let instr = Instruction::QPrepsm { dst: 0, r_base: 0, r_count: 1 };
    execute_qop(&mut ctx, &instr, &mut backend).unwrap();

    let handle = ctx.qregs[0].unwrap();
    let probs = backend.diagonal_probabilities(handle).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-10, "|000> should have prob=1");
    assert!(!ctx.psw.sf, "sf should be false for |0> state");
}

/// QPREPSM with |+> state from CMEM.
#[test]
fn test_qprepsm_plus_state() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    let h = std::f64::consts::FRAC_1_SQRT_2;
    // Write |+> state for 1 qubit at CMEM[200]
    write_qstate_to_cmem(&mut ctx, 200, h, 0.0, h, 0.0);

    ctx.iregs.set(2, 200).unwrap();
    ctx.iregs.set(3, 1).unwrap();

    let instr = Instruction::QPrepsm { dst: 0, r_base: 2, r_count: 3 };
    execute_qop(&mut ctx, &instr, &mut backend).unwrap();

    let handle = ctx.qregs[0].unwrap();
    let probs = backend.diagonal_probabilities(handle).unwrap();
    // qubit 0 in |+>, qubits 1,2 in |0> → |000> and |100> each at 50%
    assert!((probs[0] - 0.5).abs() < 1e-10, "|000> prob should be 0.5");
    assert!((probs[4] - 0.5).abs() < 1e-10, "|100> prob should be 0.5");
    assert!(ctx.psw.sf, "sf should be true when beta is nonzero");
}

/// QPREPSM should set norm_warn for unnormalized amplitudes from CMEM.
#[test]
fn test_qprepsm_normalization_warning() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    // Unnormalized: alpha=1.0, beta=0.8 → norm_sq = 1.64
    write_qstate_to_cmem(&mut ctx, 300, 1.0, 0.0, 0.8, 0.0);

    ctx.iregs.set(0, 300).unwrap();
    ctx.iregs.set(1, 1).unwrap();

    let instr = Instruction::QPrepsm { dst: 0, r_base: 0, r_count: 1 };
    execute_qop(&mut ctx, &instr, &mut backend).unwrap();

    assert!(ctx.psw.norm_warn, "norm_warn should be set for unnormalized CMEM amplitudes");
}

/// QPREPSM should trigger trap_arith for zero (0,0) amplitudes from CMEM.
#[test]
fn test_qprepsm_zero_amplitude_trap() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    // Both alpha and beta are 0
    write_qstate_to_cmem(&mut ctx, 400, 0.0, 0.0, 0.0, 0.0);

    ctx.iregs.set(0, 400).unwrap();
    ctx.iregs.set(1, 1).unwrap();

    let instr = Instruction::QPrepsm { dst: 0, r_base: 0, r_count: 1 };
    let result = execute_qop(&mut ctx, &instr, &mut backend);
    assert!(result.is_err(), "Should error on zero amplitude");
    assert!(ctx.psw.trap_arith, "trap_arith should be set");
}

// =============================================================================
// PSW flag tests
// =============================================================================

/// QPREPS clears norm_warn before each execution.
#[test]
fn test_qpreps_clears_norm_warn_before_execution() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    // Set norm_warn manually
    ctx.psw.norm_warn = true;

    // Execute with normalized amplitudes (should clear norm_warn)
    ctx.zregs.set(0, (1.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    let instr = Instruction::QPreps { dst: 0, z_start: 0, count: 1 };
    execute_qop(&mut ctx, &instr, &mut backend).unwrap();

    assert!(!ctx.psw.norm_warn, "norm_warn should be cleared when amplitudes are normalized");
}

/// QPREPS sets ef=false always.
#[test]
fn test_qpreps_ef_is_false() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    prep_zero_3q(&mut ctx, &mut backend);

    ctx.psw.ef = true; // set it to verify it gets cleared

    let h = std::f64::consts::FRAC_1_SQRT_2;
    ctx.zregs.set(0, (h, 0.0)).unwrap();
    ctx.zregs.set(1, (h, 0.0)).unwrap();

    let instr = Instruction::QPreps { dst: 0, z_start: 0, count: 1 };
    execute_qop(&mut ctx, &instr, &mut backend).unwrap();

    assert!(!ctx.psw.ef, "ef should be false after QPREPS (product state, no entanglement)");
}
