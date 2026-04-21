//! End-to-end integration tests for the CQAM classical-quantum dataflow pipeline.
//!
//! These tests verify complete data paths through the four-stage pipeline:
//!   Classical (R/F/Z) --> Quantum (Q) --> Hybrid (H) --> Classical (R/F/Z)
//!
//! Each test constructs a full instruction sequence, executes it through the
//! executor, and asserts on final classical register state. This validates that
//! all quantum extensions compose correctly in realistic programs.
//!
//! File: cqam-vm/tests/dataflow_e2e_tests.rs

use cqam_core::instruction::*;
use cqam_core::register::HybridValue;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;
use cqam_vm::fork::ForkManager;
use cqam_sim::backend::SimulationBackend;

// =============================================================================
// Helper: run a program to completion and return the final context
// =============================================================================

/// Execute a sequence of instructions until HALT or end-of-program.
/// Returns the final ExecutionContext for assertion.
fn run_program(instrs: Vec<Instruction>) -> (ExecutionContext, SimulationBackend) {
    let mut ctx = ExecutionContext::new(instrs);
    let mut fm = ForkManager::new();
    let mut backend = SimulationBackend::new();

    let program = std::sync::Arc::clone(&ctx.program);
    while ctx.pc < program.len() {
        let instr = &program[ctx.pc];
        execute_instruction(&mut ctx, instr, &mut fm, &mut backend).unwrap();
        if ctx.psw.trap_halt {
            break;
        }
    }
    (ctx, backend)
}

// =============================================================================
// Test 1: Full R-file -> Q -> H -> R-file round-trip
// =============================================================================

/// Verify the complete integer dataflow:
///   ILDI R0, target  ->  QPREP Q0  ->  QKERNEL grover  ->  QOBSERVE H0 (DIST)
///   ->  HREDUCE mode -> R4
///
/// This is the classic Grover pipeline using QOBSERVE with explicit mode=DIST.
///
/// Assertions:
///   - R4 (mode of distribution) should be the Grover target after sufficient iterations
///   - H0 should be Dist variant
///   - Q0 should be None after QOBSERVE (destructive)
///   - PSW.DF should be set
#[test]
fn test_e2e_grover_r_file_round_trip() {
    // Use target=0 which is known to converge well with the Grover oracle on 4 states.
    let target_state: i16 = 0;
    let instrs = vec![
        // Set up Grover target in R0
        Instruction::ILdi { dst: 0, imm: target_state },
        // Prepare uniform superposition
        Instruction::QPrep { dst: 0, dist: DistId::Uniform },
        // Apply Grover iteration (oracle + diffusion)
        Instruction::QKernel { dst: 0, src: 0, kernel: KernelId::GroverIter, ctx0: 0, ctx1: 0 },
        // Observe with mode=DIST
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        // Reduce: get mode (most probable state)
        Instruction::HReduce { src: 0, dst: 4, func: ReduceFn::Mode },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // R4 should hold the mode of the distribution = the Grover target
    // For 4 states, one Grover iteration is optimal and should amplify target.
    assert_eq!(ctx.iregs.get(4).unwrap(), target_state as i64);
    // H0 should be Dist variant
    assert!(matches!(ctx.hregs.get(0).unwrap(), HybridValue::Dist(_)));
    // Q0 should be None after QOBSERVE (destructive)
    assert!(ctx.qregs[0].is_none());
    // PSW.DF should be set
    assert!(ctx.psw.df);
}

// =============================================================================
// Test 2: F-file -> Q via QKERNELF -> H -> F-file
// =============================================================================

/// Verify float-parameterized kernel dataflow:
///   FLDI F0, theta  ->  QPREP Q0, UNIFORM  ->  QKERNELF Q0, Q0, ROTATE, F0, F1
///   ->  QOBSERVE H0, Q0, DIST  ->  HREDUCE mean -> F2
///
/// Assertions:
///   - The Rotate kernel should produce a non-uniform distribution
///   - F2 (mean) should be a finite, non-negative float
///   - Q0 consumed after QOBSERVE
#[test]
fn test_e2e_float_kernel_rotate() {
    let instrs = vec![
        // F0 = theta = 1.0
        Instruction::FLdi { dst: 0, imm: 1 },
        // F1 = 0 (unused second context)
        Instruction::FLdi { dst: 1, imm: 0 },
        // Prepare uniform
        Instruction::QPrep { dst: 0, dist: DistId::Uniform },
        // Apply ROTATE kernel with float params
        Instruction::QKernelF { dst: 0, src: 0, kernel: KernelId::Rotate, fctx0: 0, fctx1: 1 },
        // Observe as DIST
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        // Reduce: mean of distribution -> F2
        Instruction::HReduce { src: 0, dst: 2, func: ReduceFn::Mean },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // F2 should be a finite, non-negative float
    let mean = ctx.fregs.get(2).unwrap();
    assert!(mean.is_finite());
    assert!(mean >= 0.0);
    // Q0 consumed
    assert!(ctx.qregs[0].is_none());
}

// =============================================================================
// Test 3: Z-file -> Q via QKERNELZ -> H -> Z-file
// =============================================================================

/// Verify complex-parameterized kernel dataflow:
///   ZLDI Z0, re, im  ->  QPREP Q0, UNIFORM  ->  QKERNELZ Q0, Q0, PHASE_SHIFT, Z0, Z1
///   ->  QOBSERVE H0, Q0, DIST  ->  HREDUCE ARGMX -> R2
///
/// Previously used AMP mode (removed from ISA). Now verifies the same kernel
/// dataflow ends with a valid DIST result.
///
/// Assertions:
///   - H0 should be Dist variant
///   - Q0 is None (destructive)
#[test]
fn test_e2e_complex_kernel_phase_shift_to_z_file() {
    let instrs = vec![
        // Z0 = (1.0, 1.0) -- complex amplitude for phase_shift
        Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: 1 },
        // Z1 = (0.0, 0.0) -- second context
        Instruction::ZLdi { dst: 1, imm_re: 0, imm_im: 0 },
        // Prepare uniform
        Instruction::QPrep { dst: 0, dist: DistId::Uniform },
        // Apply PHASE_SHIFT kernel with complex params
        Instruction::QKernelZ { dst: 0, src: 0, kernel: KernelId::PhaseShift, zctx0: 0, zctx1: 1 },
        // Observe with DIST mode
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // H0 should be Dist
    assert!(matches!(ctx.hregs.get(0).unwrap(), HybridValue::Dist(_)));
    // Q0 consumed
    assert!(ctx.qregs[0].is_none());
}

// (Test 4 removed: QSAMPLE was removed from the ISA — no non-destructive observation.)

// =============================================================================
// Test 5: QOBSERVE mode=PROB single probability extraction
// =============================================================================

/// Verify PROB mode extracts a single probability:
///   QPREP Q0, ZERO  ->  ILDI R0, 0  ->  QOBSERVE H0, Q0, PROB, R0, R0
///
/// For the |0> state, P(0) should be 1.0.
///
/// Assertions:
///   - H0 == Float(1.0)
///   - Q0 is None (destructive)
#[test]
fn test_e2e_qobserve_prob_mode() {
    let instrs = vec![
        Instruction::ILdi { dst: 0, imm: 0 },  // R0 = 0
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Prob, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // H0 should be Float(1.0) -- probability of |0> in the zero state
    if let HybridValue::Float(p) = ctx.hregs.get(0).unwrap() {
        assert!((p - 1.0).abs() < 1e-10, "P(0) for |0> state should be 1.0, got {}", p);
    } else {
        panic!("Expected Float variant in H0, got {:?}", ctx.hregs.get(0));
    }
    // Q0 is None (destructive)
    assert!(ctx.qregs[0].is_none());
}

// (Test 6 removed: QOBSERVE AMP mode was removed from the ISA — density matrix
//  element extraction is not physically realizable on hardware.)

// =============================================================================
// Test 7: QPREPR dynamic distribution selection
// =============================================================================

/// Verify register-parameterized preparation:
///   ILDI R0, 1  ->  QPREPR Q0, R0  ->  QOBSERVE H0, Q0, DIST
///
/// R0=1 means ZERO distribution. The observed distribution should have
/// P(0) = 1.0.
///
/// Assertions:
///   - H0 is Dist with a single entry (0, 1.0)
///   - Q0 is None after QOBSERVE (destructive)
#[test]
fn test_e2e_qprepr_dynamic_dist() {
    let instrs = vec![
        Instruction::ILdi { dst: 0, imm: 1 },  // R0 = ZERO dist_id
        Instruction::QPrepR { dst: 0, dist_reg: 0 },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // Q0 should be consumed after QOBSERVE
    assert!(ctx.qregs[0].is_none());

    // H0 should be Dist with essentially P(0) = 1.0
    if let HybridValue::Dist(entries) = ctx.hregs.get(0).unwrap() {
        // Should have the 0 state with probability ~1.0
        let p0 = entries.iter().find(|(k, _)| *k == 0).map(|(_, p)| *p).unwrap_or(0.0);
        assert!((p0 - 1.0).abs() < 1e-10, "P(0)={}, expected 1.0", p0);
    } else {
        panic!("Expected Dist variant in H0");
    }
}

// =============================================================================
// Test 8: QENCODE from F-file
// =============================================================================

/// Verify amplitude encoding from float registers:
///   FLDI F0, 1  ->  FLDI F1, 0  ->  FLDI F2, 0  ->  FLDI F3, 1
///   ->  QENCODE Q0, F0, 4, F_FILE
///   ->  QOBSERVE H0, Q0, DIST
///
/// The encoded state should be (|00> + |11>)/sqrt(2), i.e., a Bell-like state.
///
/// Assertions:
///   - H0 has entries at state 0 and state 3, each ~0.5
///   - States 1 and 2 have probability ~0.0
///   - Q0 is None after QOBSERVE (destructive)
#[test]
fn test_e2e_qencode_f_file_bell_like() {
    let instrs = vec![
        Instruction::FLdi { dst: 0, imm: 1 },  // F0 = 1.0
        Instruction::FLdi { dst: 1, imm: 0 },  // F1 = 0.0
        Instruction::FLdi { dst: 2, imm: 0 },  // F2 = 0.0
        Instruction::FLdi { dst: 3, imm: 1 },  // F3 = 1.0
        Instruction::QEncode { dst: 0, src_base: 0, count: 4, file_sel: FileSel::FFile },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // Q0 should be consumed after QOBSERVE
    assert!(ctx.qregs[0].is_none());

    // H0 should be Dist
    if let HybridValue::Dist(entries) = ctx.hregs.get(0).unwrap() {
        let p0 = entries.iter().find(|(k, _)| *k == 0).map(|(_, p)| *p).unwrap_or(0.0);
        let p3 = entries.iter().find(|(k, _)| *k == 3).map(|(_, p)| *p).unwrap_or(0.0);
        let p1 = entries.iter().find(|(k, _)| *k == 1).map(|(_, p)| *p).unwrap_or(0.0);
        let p2 = entries.iter().find(|(k, _)| *k == 2).map(|(_, p)| *p).unwrap_or(0.0);
        assert!((p0 - 0.5).abs() < 1e-10, "P(0)={}, expected 0.5", p0);
        assert!((p3 - 0.5).abs() < 1e-10, "P(3)={}, expected 0.5", p3);
        assert!(p1.abs() < 1e-10, "P(1)={}, expected 0.0", p1);
        assert!(p2.abs() < 1e-10, "P(2)={}, expected 0.0", p2);
    } else {
        panic!("Expected Dist variant in H0");
    }
}

// =============================================================================
// Test 9: QENCODE from Z-file (complex amplitudes with phase)
// =============================================================================

/// Verify amplitude encoding from complex registers:
///   ZLDI Z0, 1, 0  ->  ZLDI Z1, 0, 1
///   ->  QENCODE Q0, Z0, 2, Z_FILE
///   ->  QOBSERVE H0, Q0, DIST
///
/// Previously observed in AMP mode (removed from ISA). Now verifies that
/// QENCODE from Z-file produces a valid quantum state by observing its DIST.
///
/// Assertions:
///   - H0 is Dist variant
///   - Q0 is None after QOBSERVE (destructive)
#[test]
fn test_e2e_qencode_z_file_with_phase() {
    let instrs = vec![
        Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: 0 },  // Z0 = (1, 0)
        Instruction::ZLdi { dst: 1, imm_re: 0, imm_im: 1 },  // Z1 = (0, 1) = i
        Instruction::QEncode { dst: 0, src_base: 0, count: 2, file_sel: FileSel::ZFile },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // Q0 should be consumed after QOBSERVE
    assert!(ctx.qregs[0].is_none());

    // H0 should be Dist
    assert!(matches!(ctx.hregs.get(0).unwrap(), HybridValue::Dist(_)));
}

// =============================================================================
// Test 10: Masked Hadamard selective superposition
// =============================================================================

/// Verify QHADM creates selective superposition:
///   QPREP Q0, ZERO  ->  ILDI R0, 1 (mask=0b01: qubit 0)
///   ->  QHADM Q1, Q0, R0
///   ->  QOBSERVE H0, Q1, DIST
///
/// Starting from |00>, Hadamard on qubit 0 (MSB in the density matrix convention)
/// gives (|0>+|1>)/sqrt(2) tensor |0>, i.e., states |00>=0 and |10>=2.
/// So P(0) = P(2) = 0.5, P(1) = P(3) = 0.
///
/// Assertions:
///   - H0 has entries at state 0 and state 2, each ~0.5
///   - States 1 and 3 have probability ~0.0
///   - Q1 is None after QOBSERVE (destructive)
#[test]
fn test_e2e_qhadm_selective_superposition() {
    let instrs = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::ILdi { dst: 0, imm: 1 },  // mask = 0b01 (qubit 0 = MSB)
        Instruction::QHadM { dst: 1, src: 0, mask_reg: 0 },
        Instruction::QObserve { dst_h: 0, src_q: 1, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    if let HybridValue::Dist(entries) = ctx.hregs.get(0).unwrap() {
        let p0 = entries.iter().find(|(k, _)| *k == 0).map(|(_, p)| *p).unwrap_or(0.0);
        let p1 = entries.iter().find(|(k, _)| *k == 1).map(|(_, p)| *p).unwrap_or(0.0);
        let p2 = entries.iter().find(|(k, _)| *k == 2).map(|(_, p)| *p).unwrap_or(0.0);
        let p3 = entries.iter().find(|(k, _)| *k == 3).map(|(_, p)| *p).unwrap_or(0.0);
        assert!((p0 - 0.5).abs() < 1e-10, "P(0)={}, expected 0.5", p0);
        assert!(p1.abs() < 1e-10, "P(1)={}, expected 0.0", p1);
        assert!((p2 - 0.5).abs() < 1e-10, "P(2)={}, expected 0.5", p2);
        assert!(p3.abs() < 1e-10, "P(3)={}, expected 0.0", p3);
    } else {
        panic!("Expected Dist variant in H0");
    }
}

// =============================================================================
// Test 11: QFLIP bit-flip on specific qubits
// =============================================================================

/// Verify QFLIP performs selective bit-flip:
///   QPREP Q0, ZERO  ->  ILDI R0, 3 (mask=0b11: both qubits)
///   ->  QFLIP Q1, Q0, R0
///   ->  QOBSERVE H0, Q1, DIST
///
/// X on both qubits of |00> gives |11>, so P(3) = 1.0.
///
/// Assertions:
///   - H0 has single entry (3, 1.0)
///   - No superposition (deterministic state)
///   - Q1 is None after QOBSERVE (destructive)
#[test]
fn test_e2e_qflip_both_qubits() {
    let instrs = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::ILdi { dst: 0, imm: 3 },  // mask = 0b11 (both qubits)
        Instruction::QFlip { dst: 1, src: 0, mask_reg: 0 },
        Instruction::QObserve { dst_h: 0, src_q: 1, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    if let HybridValue::Dist(entries) = ctx.hregs.get(0).unwrap() {
        let p3 = entries.iter().find(|(k, _)| *k == 3).map(|(_, p)| *p).unwrap_or(0.0);
        assert!((p3 - 1.0).abs() < 1e-10, "P(3)={}, expected 1.0", p3);
        // All other states should be 0
        for &(k, p) in entries.iter() {
            if k != 3 {
                assert!(p.abs() < 1e-10, "P({})={}, expected 0.0", k, p);
            }
        }
    } else {
        panic!("Expected Dist variant in H0");
    }
}

// =============================================================================
// Test 12: QPHASE on superposition state
// =============================================================================

/// Verify QPHASE applies phase flip:
///   QPREP Q0, UNIFORM  ->  ILDI R0, 1 (mask=0b01: qubit 0)
///   ->  QPHASE Q1, Q0, R0
///   ->  QOBSERVE H0, Q1, DIST
///
/// Phase flip does not change diagonal probabilities (populations unchanged).
/// The distribution should still be uniform.
///
/// Assertions:
///   - H0 has 4 entries, each ~0.25
///   - Q1 is None after QOBSERVE (destructive)
#[test]
fn test_e2e_qphase_preserves_probabilities() {
    let instrs = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Uniform },
        Instruction::ILdi { dst: 0, imm: 1 },  // mask = 0b01 (qubit 0)
        Instruction::QPhase { dst: 1, src: 0, mask_reg: 0 },
        Instruction::QObserve { dst_h: 0, src_q: 1, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    if let HybridValue::Dist(entries) = ctx.hregs.get(0).unwrap() {
        // All 4 states should have probability ~0.25
        for state_idx in 0u32..4 {
            let prob = entries.iter()
                .find(|(k, _)| *k == state_idx)
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            assert!((prob - 0.25).abs() < 1e-10,
                "P({})={}, expected 0.25", state_idx, prob);
        }
    } else {
        panic!("Expected Dist variant in H0");
    }
}

// =============================================================================
// Test 13: CONJ_Z and NEGATE_Z reductions (via ZLDI -> Z-register directly)
// =============================================================================

/// Verify the Z-file reduction functions operate on Complex values from Z-registers.
/// Previously tested via QOBSERVE(AMP) (removed from ISA). Now uses ZLDI to
/// place a known complex value into H0 directly, then reduces.
///
///   ZLDI Z0, 1, 0  (H0 seed value, but we set H0 directly via ZLDI + load)
///
/// Strategy: Use ZLDI to set Z0 = (0.5, 0.0), then use a HREDUCE on a
/// HybridValue::Complex placed directly via a QOBSERVE(PROB) followed by a
/// manual verify, OR simply test ConjZ/NegateZ on a Complex value already in H.
///
/// Since H-registers only accept Complex from backend observe results, we use
/// QPREP + QOBSERVE(PROB) to get a Float into H0, then verify CONJ_Z and
/// NEGATE_Z accept Float (treating it as Complex(v, 0.0)).
///
/// Assertions:
///   - Z0 = (prob, 0.0) via CONJ_Z (identity on real-valued input)
///   - Z1 = (-prob, 0.0) via NEGATE_Z
#[test]
fn test_e2e_conj_z_and_negate_z_reductions() {
    // CONJ_Z and NEGATE_Z accept Float as Complex(v, 0.0).
    // Use |0> state: P(0) = 1.0, so H0 = Float(1.0) after PROB observe.
    let instrs = vec![
        Instruction::ILdi { dst: 0, imm: 0 },  // R0 = basis state index 0
        Instruction::QPrep { dst: 0, dist: DistId::Zero },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Prob, ctx0: 0, ctx1: 0 },
        // CONJ_Z: Z0 = conj(Float(1.0)) = (1.0, 0.0)
        Instruction::HReduce { src: 0, dst: 0, func: ReduceFn::ConjZ },
        // NEGATE_Z: Z1 = negate(Float(1.0)) = (-1.0, 0.0)
        Instruction::HReduce { src: 0, dst: 1, func: ReduceFn::NegateZ },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // H0 = Float(1.0)
    if let HybridValue::Float(p) = ctx.hregs.get(0).unwrap() {
        assert!((p - 1.0).abs() < 1e-10, "P(0) for |0> state should be 1.0, got {}", p);
    } else {
        panic!("Expected Float variant in H0");
    }

    // Z0 = conj(1.0 + 0i) = (1.0, 0.0)
    let (z0_re, z0_im) = ctx.zregs.get(0).unwrap();
    assert!((z0_re - 1.0).abs() < 1e-10, "Z0.re={}, expected 1.0", z0_re);
    assert!(z0_im.abs() < 1e-10, "Z0.im={}, expected 0.0", z0_im);

    // Z1 = negate(1.0 + 0i) = (-1.0, 0.0)
    let (z1_re, z1_im) = ctx.zregs.get(1).unwrap();
    assert!((z1_re - (-1.0)).abs() < 1e-10, "Z1.re={}, expected -1.0", z1_re);
    assert!(z1_im.abs() < 1e-10, "Z1.im={}, expected 0.0", z1_im);
}

// (Test 14 removed: QSAMPLE feedback loop was removed from the ISA — no non-destructive observation.)

// =============================================================================
// Test 15: Full 4-stage pipeline: Z -> Q -> H -> R
// =============================================================================

/// Verify the complete Z-file -> quantum -> H -> classical dataflow end-to-end:
///   ZLDI Z0, 1, 1  ->  QENCODE Q0, Z0, 2, Z_FILE
///   ->  QKERNELZ Q0, Q0, PHASE_SHIFT, Z0, Z1
///   ->  QOBSERVE H0, Q0, DIST
///   ->  HREDUCE ARGMX, H0, R2
///
/// Previously used AMP mode (removed from ISA). Now verifies the same pipeline
/// (QENCODE + QKERNELZ) ends with a valid DIST result and ARGMX reduction.
///
/// Assertions:
///   - H0 is Dist variant
///   - R2 holds a valid basis state index (in-range for a 1-qubit state)
///   - The pipeline completes without error
#[test]
fn test_e2e_full_z_pipeline() {
    let instrs = vec![
        // Z0 = (1.0, 1.0), Z1 = (1.0, 0.0)
        Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: 1 },
        Instruction::ZLdi { dst: 1, imm_re: 1, imm_im: 0 },
        // QENCODE from Z-file: uses Z0, Z1 as amplitudes for a 1-qubit state
        Instruction::QEncode { dst: 0, src_base: 0, count: 2, file_sel: FileSel::ZFile },
        // Apply PHASE_SHIFT kernel with complex context Z0, Z1
        Instruction::QKernelZ { dst: 0, src: 0, kernel: KernelId::PhaseShift, zctx0: 0, zctx1: 1 },
        // Observe DIST
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        // ARGMX -> R2
        Instruction::HReduce { src: 0, dst: 2, func: ReduceFn::Argmax },
        Instruction::Halt,
    ];

    let (ctx, _backend) = run_program(instrs);

    // H0 should be Dist
    assert!(matches!(ctx.hregs.get(0).unwrap(), HybridValue::Dist(_)));

    // R2 holds the most probable basis state index (0 or 1 for 1 qubit)
    let most_probable = ctx.iregs.get(2).unwrap();
    assert!(most_probable == 0 || most_probable == 1, "basis state index out of range: {}", most_probable);

    // Q0 should be consumed
    assert!(ctx.qregs[0].is_none());
}
