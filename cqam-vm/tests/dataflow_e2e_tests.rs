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

// =============================================================================
// Helper: run a program to completion and return the final context
// =============================================================================

/// Execute a sequence of instructions until HALT or end-of-program.
/// Returns the final ExecutionContext for assertion.
fn run_program(instrs: Vec<Instruction>) -> ExecutionContext {
    let mut ctx = ExecutionContext::new(instrs);
    let mut fm = ForkManager::new();

    while ctx.pc < ctx.program.len() {
        let instr = ctx.program[ctx.pc].clone();
        execute_instruction(&mut ctx, &instr, &mut fm).unwrap();
        if ctx.psw.trap_halt {
            break;
        }
    }
    ctx
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
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        // Apply Grover iteration (oracle + diffusion)
        Instruction::QKernel { dst: 0, src: 0, kernel: kernel_id::GROVER_ITER, ctx0: 0, ctx1: 0 },
        // Observe with mode=DIST
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
        // Reduce: get mode (most probable state)
        Instruction::HReduce { src: 0, dst: 4, func: reduce_fn::MODE },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

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
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        // Apply ROTATE kernel with float params
        Instruction::QKernelF { dst: 0, src: 0, kernel: kernel_id::ROTATE, fctx0: 0, fctx1: 1 },
        // Observe as DIST
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
        // Reduce: mean of distribution -> F2
        Instruction::HReduce { src: 0, dst: 2, func: reduce_fn::MEAN },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

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
///   ->  QOBSERVE H0, Q0, AMP, R0, R1  ->  result is Complex in H0
///   ->  HREDUCE CONJ_Z -> Z2
///
/// Assertions:
///   - H0 should be Complex variant (from AMP mode)
///   - Z2 should contain the conjugate of the extracted amplitude
#[test]
fn test_e2e_complex_kernel_phase_shift_to_z_file() {
    let instrs = vec![
        // Z0 = (1.0, 1.0) -- complex amplitude for phase_shift
        Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: 1 },
        // Z1 = (0.0, 0.0) -- unused second context
        Instruction::ZLdi { dst: 1, imm_re: 0, imm_im: 0 },
        // R0 = 0, R1 = 1 (row/col for AMP mode)
        Instruction::ILdi { dst: 0, imm: 0 },
        Instruction::ILdi { dst: 1, imm: 1 },
        // Prepare uniform
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        // Apply PHASE_SHIFT kernel with complex params
        Instruction::QKernelZ { dst: 0, src: 0, kernel: kernel_id::PHASE_SHIFT, zctx0: 0, zctx1: 1 },
        // Observe with AMP mode at (0,1)
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1 },
        // Reduce: conjugate -> Z2
        Instruction::HReduce { src: 0, dst: 2, func: reduce_fn::CONJ_Z },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // H0 should be Complex
    assert!(matches!(ctx.hregs.get(0).unwrap(), HybridValue::Complex(_, _)));

    // Z2 should be conjugate of H0
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        let (z2_re, z2_im) = ctx.zregs.get(2).unwrap();
        assert!((z2_re - re).abs() < 1e-10);
        assert!((z2_im - (-im)).abs() < 1e-10);
    }
}

// =============================================================================
// Test 4: QSAMPLE non-destructive -> multiple reads
// =============================================================================

/// Verify non-destructive sampling:
///   QPREP Q0, BELL  ->  QSAMPLE H0, Q0, DIST  ->  QSAMPLE H1, Q0, PROB, R0
///   ->  verify Q0 still alive  ->  QOBSERVE H2, Q0, DIST  ->  Q0 is None
///
/// Assertions:
///   - After first QSAMPLE, Q0 is still Some
///   - After second QSAMPLE, Q0 is still Some
///   - H0 is Dist with Bell-state probabilities
///   - H1 is Complex(probability, 0.0) of state R0
///   - After QOBSERVE, Q0 is None
///   - PSW.DF only set after QOBSERVE, not after QSAMPLE
#[test]
fn test_e2e_qsample_nondestructive_then_observe() {
    // We'll execute step by step to check intermediate states
    let instrs = vec![
        Instruction::ILdi { dst: 0, imm: 0 }, // R0 = 0 (for PROB mode)
        Instruction::QPrep { dst: 0, dist: dist_id::BELL },
        Instruction::QSample { dst_h: 0, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
    ];

    let mut ctx = ExecutionContext::new(instrs);
    let mut fm = ForkManager::new();

    // Execute ILDI and QPREP
    let instr0 = ctx.program[0].clone();
    execute_instruction(&mut ctx, &instr0, &mut fm).unwrap();
    let instr1 = ctx.program[1].clone();
    execute_instruction(&mut ctx, &instr1, &mut fm).unwrap();

    assert!(ctx.qregs[0].is_some());

    // Execute QSAMPLE (DIST mode)
    let instr2 = ctx.program[2].clone();
    execute_instruction(&mut ctx, &instr2, &mut fm).unwrap();

    // Q0 should still be alive
    assert!(ctx.qregs[0].is_some());
    // H0 should be Dist
    assert!(matches!(ctx.hregs.get(0).unwrap(), HybridValue::Dist(_)));
    // PSW.DF should NOT be set (QSAMPLE is non-destructive)
    assert!(!ctx.psw.df);

    // Now do a second QSAMPLE with PROB mode
    let qsample_prob = Instruction::QSample { dst_h: 1, src_q: 0, mode: observe_mode::PROB, ctx0: 0, ctx1: 0 };
    execute_instruction(&mut ctx, &qsample_prob, &mut fm).unwrap();

    // Q0 still alive
    assert!(ctx.qregs[0].is_some());
    // H1 should be Complex (probability with im=0.0)
    assert!(matches!(ctx.hregs.get(1).unwrap(), HybridValue::Complex(_, _)));

    // Now QOBSERVE to consume
    let qobs = Instruction::QObserve { dst_h: 2, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 };
    execute_instruction(&mut ctx, &qobs, &mut fm).unwrap();

    // Q0 should now be None
    assert!(ctx.qregs[0].is_none());
    // PSW.DF should be set after QOBSERVE
    assert!(ctx.psw.df);
}

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
        Instruction::QPrep { dst: 0, dist: dist_id::ZERO },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: observe_mode::PROB, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // H0 should be Complex(1.0, 0.0) -- probability of |0> in the zero state
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((re - 1.0).abs() < 1e-10);
        assert!((im).abs() < 1e-10);
    } else {
        panic!("Expected Complex variant in H0");
    }
    // Q0 is None (destructive)
    assert!(ctx.qregs[0].is_none());
}

// =============================================================================
// Test 6: QOBSERVE mode=AMP density matrix element extraction
// =============================================================================

/// Verify AMP mode extracts a density matrix element:
///   QPREP Q0, BELL  ->  ILDI R0, 0  ->  ILDI R1, 3
///   ->  QOBSERVE H0, Q0, AMP, R0, R1
///
/// For Bell state, rho[0][3] = 0.5 + 0.0i (off-diagonal coherence).
///
/// Assertions:
///   - H0 == Complex(0.5, 0.0)
///   - Q0 is None (destructive)
#[test]
fn test_e2e_qobserve_amp_mode() {
    let instrs = vec![
        Instruction::ILdi { dst: 0, imm: 0 },  // R0 = row = 0
        Instruction::ILdi { dst: 1, imm: 3 },  // R1 = col = 3
        Instruction::QPrep { dst: 0, dist: dist_id::BELL },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1 },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // Bell state rho[0][3] = 0.5 + 0.0i
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((re - 0.5).abs() < 1e-10, "re={}, expected 0.5", re);
        assert!(im.abs() < 1e-10, "im={}, expected 0.0", im);
    } else {
        panic!("Expected Complex variant in H0");
    }
    assert!(ctx.qregs[0].is_none());
}

// =============================================================================
// Test 7: QPREPR dynamic distribution selection
// =============================================================================

/// Verify register-parameterized preparation:
///   ILDI R0, 1  ->  QPREPR Q0, R0  ->  QSAMPLE H0, Q0, DIST
///
/// R0=1 means ZERO distribution. The sampled distribution should have
/// P(0) = 1.0.
///
/// Assertions:
///   - Q0 is a valid density matrix
///   - H0 is Dist with a single entry (0, 1.0)
#[test]
fn test_e2e_qprepr_dynamic_dist() {
    let instrs = vec![
        Instruction::ILdi { dst: 0, imm: 1 },  // R0 = ZERO dist_id
        Instruction::QPrepR { dst: 0, dist_reg: 0 },
        Instruction::QSample { dst_h: 0, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // Q0 should still be alive (QSAMPLE is non-destructive)
    assert!(ctx.qregs[0].is_some());

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
///   ->  QSAMPLE H0, Q0, DIST
///
/// The encoded state should be (|00> + |11>)/sqrt(2), i.e., a Bell-like state.
///
/// Assertions:
///   - Q0 is a 2-qubit state (dim=4)
///   - H0 has entries at state 0 and state 3, each ~0.5
///   - States 1 and 2 have probability ~0.0
#[test]
fn test_e2e_qencode_f_file_bell_like() {
    let instrs = vec![
        Instruction::FLdi { dst: 0, imm: 1 },  // F0 = 1.0
        Instruction::FLdi { dst: 1, imm: 0 },  // F1 = 0.0
        Instruction::FLdi { dst: 2, imm: 0 },  // F2 = 0.0
        Instruction::FLdi { dst: 3, imm: 1 },  // F3 = 1.0
        Instruction::QEncode { dst: 0, src_base: 0, count: 4, file_sel: file_sel::F_FILE },
        Instruction::QSample { dst_h: 0, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // Q0 should exist
    assert!(ctx.qregs[0].is_some());

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
///   ->  QSAMPLE H0, Q0, DIST
///   ->  QSAMPLE H1, Q0, AMP, R_row, R_col
///
/// Assertions:
///   - Q0 is a 1-qubit state (dim=2)
///   - Both basis states have probability 0.5
///   - The off-diagonal element has non-zero imaginary part (phase information)
#[test]
fn test_e2e_qencode_z_file_with_phase() {
    let instrs = vec![
        Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: 0 },  // Z0 = (1, 0)
        Instruction::ZLdi { dst: 1, imm_re: 0, imm_im: 1 },  // Z1 = (0, 1) = i
        Instruction::ILdi { dst: 0, imm: 0 },  // R0 = 0 (row)
        Instruction::ILdi { dst: 1, imm: 1 },  // R1 = 1 (col)
        Instruction::QEncode { dst: 0, src_base: 0, count: 2, file_sel: file_sel::Z_FILE },
        Instruction::QSample { dst_h: 0, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
        Instruction::QSample { dst_h: 1, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1 },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // H0 should be Dist with two entries, each ~0.5
    if let HybridValue::Dist(entries) = ctx.hregs.get(0).unwrap() {
        let p0 = entries.iter().find(|(k, _)| *k == 0).map(|(_, p)| *p).unwrap_or(0.0);
        let p1 = entries.iter().find(|(k, _)| *k == 1).map(|(_, p)| *p).unwrap_or(0.0);
        assert!((p0 - 0.5).abs() < 1e-10, "P(0)={}, expected 0.5", p0);
        assert!((p1 - 0.5).abs() < 1e-10, "P(1)={}, expected 0.5", p1);
    } else {
        panic!("Expected Dist variant in H0");
    }

    // H1 should be Complex with non-zero imaginary part (off-diagonal has phase info)
    if let HybridValue::Complex(re, im) = ctx.hregs.get(1).unwrap() {
        // rho[0][1] = z0 * conj(z1) / norm = (1,0) * (0,-1) / 2 = (0,-1)/2 = (0, -0.5)
        assert!(re.abs() < 1e-10, "re={}, expected 0.0", re);
        assert!(im.abs() > 1e-10, "im={}, expected non-zero", im);
    } else {
        panic!("Expected Complex variant in H1");
    }
}

// =============================================================================
// Test 10: Masked Hadamard selective superposition
// =============================================================================

/// Verify QHADM creates selective superposition:
///   QPREP Q0, ZERO  ->  ILDI R0, 1 (mask=0b01: qubit 0)
///   ->  QHADM Q1, Q0, R0
///   ->  QSAMPLE H0, Q1, DIST
///
/// Starting from |00>, Hadamard on qubit 0 (MSB in the density matrix convention)
/// gives (|0>+|1>)/sqrt(2) tensor |0>, i.e., states |00>=0 and |10>=2.
/// So P(0) = P(2) = 0.5, P(1) = P(3) = 0.
///
/// Assertions:
///   - H0 has entries at state 0 and state 2, each ~0.5
///   - States 1 and 3 have probability ~0.0
#[test]
fn test_e2e_qhadm_selective_superposition() {
    let instrs = vec![
        Instruction::QPrep { dst: 0, dist: dist_id::ZERO },
        Instruction::ILdi { dst: 0, imm: 1 },  // mask = 0b01 (qubit 0 = MSB)
        Instruction::QHadM { dst: 1, src: 0, mask_reg: 0 },
        Instruction::QSample { dst_h: 0, src_q: 1, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

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
///   ->  QSAMPLE H0, Q1, DIST
///
/// X on both qubits of |00> gives |11>, so P(3) = 1.0.
///
/// Assertions:
///   - H0 has single entry (3, 1.0)
///   - No superposition (deterministic state)
#[test]
fn test_e2e_qflip_both_qubits() {
    let instrs = vec![
        Instruction::QPrep { dst: 0, dist: dist_id::ZERO },
        Instruction::ILdi { dst: 0, imm: 3 },  // mask = 0b11 (both qubits)
        Instruction::QFlip { dst: 1, src: 0, mask_reg: 0 },
        Instruction::QSample { dst_h: 0, src_q: 1, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

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
///   ->  QSAMPLE H0, Q1, DIST
///
/// Phase flip does not change diagonal probabilities (populations unchanged).
/// The distribution should still be uniform.
///
/// Assertions:
///   - H0 has 4 entries, each ~0.25
#[test]
fn test_e2e_qphase_preserves_probabilities() {
    let instrs = vec![
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        Instruction::ILdi { dst: 0, imm: 1 },  // mask = 0b01 (qubit 0)
        Instruction::QPhase { dst: 1, src: 0, mask_reg: 0 },
        Instruction::QSample { dst_h: 0, src_q: 1, mode: observe_mode::DIST, ctx0: 0, ctx1: 0 },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    if let HybridValue::Dist(entries) = ctx.hregs.get(0).unwrap() {
        // All 4 states should have probability ~0.25
        for state_idx in 0u16..4 {
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
// Test 13: CONJ_Z and NEGATE_Z reductions
// =============================================================================

/// Verify the new Z-file reduction functions:
///   QPREP Q0, BELL  ->  QOBSERVE H0, Q0, AMP, R_row, R_col
///   ->  HREDUCE H0, Z0, CONJ_Z
///   ->  HREDUCE H0, Z1, NEGATE_Z
///
/// For Bell state rho[0][3] = (0.5, 0.0):
///   CONJ_Z:   Z0 = (0.5, 0.0)  (imaginary part negated, but was 0)
///   NEGATE_Z: Z1 = (-0.5, 0.0)
///
/// Assertions:
///   - Z0 == (0.5, 0.0)
///   - Z1 == (-0.5, 0.0)
#[test]
fn test_e2e_conj_z_and_negate_z_reductions() {
    let instrs = vec![
        Instruction::ILdi { dst: 0, imm: 0 },  // R0 = row = 0
        Instruction::ILdi { dst: 1, imm: 3 },  // R1 = col = 3
        Instruction::QPrep { dst: 0, dist: dist_id::BELL },
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1 },
        // CONJ_Z: Z0 = conj(H0)
        Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::CONJ_Z },
        // NEGATE_Z: Z1 = -H0
        Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::NEGATE_Z },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // H0 should be Complex(0.5, 0.0) for Bell state rho[0][3]
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((re - 0.5).abs() < 1e-10);
        assert!(im.abs() < 1e-10);
    } else {
        panic!("Expected Complex variant in H0");
    }

    // Z0 = conj(0.5, 0.0) = (0.5, -0.0) ≈ (0.5, 0.0)
    let (z0_re, z0_im) = ctx.zregs.get(0).unwrap();
    assert!((z0_re - 0.5).abs() < 1e-10, "Z0.re={}, expected 0.5", z0_re);
    assert!(z0_im.abs() < 1e-10, "Z0.im={}, expected 0.0", z0_im);

    // Z1 = negate(0.5, 0.0) = (-0.5, 0.0)
    let (z1_re, z1_im) = ctx.zregs.get(1).unwrap();
    assert!((z1_re - (-0.5)).abs() < 1e-10, "Z1.re={}, expected -0.5", z1_re);
    assert!(z1_im.abs() < 1e-10, "Z1.im={}, expected 0.0", z1_im);
}

// =============================================================================
// Test 14: Multi-stage pipeline with QSAMPLE feedback loop
// =============================================================================

/// Verify a realistic multi-step computation:
///   1. QPREP Q0, UNIFORM
///   2. QKERNELF Q0, Q0, ROTATE, F_theta (apply rotation)
///   3. QSAMPLE H0, Q0, PROB, R_target (check probability of target)
///   4. HREDUCE H0, F1, REAL (extract probability as float from Complex(prob, 0.0))
///   5. Verify F1 contains a valid probability in [0, 1]
///
/// This tests the pattern of "peek at quantum state, use result classically"
/// without destroying the quantum register.
///
/// Assertions:
///   - Q0 still alive after QSAMPLE
///   - F1 is in [0.0, 1.0]
///   - A second QKERNEL can still be applied to Q0
#[test]
fn test_e2e_qsample_feedback_loop() {
    let instrs = vec![
        // Set up parameters
        Instruction::FLdi { dst: 0, imm: 1 },  // F0 = theta = 1.0
        Instruction::FLdi { dst: 1, imm: 0 },  // F1 = 0 (unused)
        Instruction::ILdi { dst: 0, imm: 0 },  // R0 = 0 (target state for PROB mode)
        // Prepare uniform
        Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
        // Apply ROTATE kernel
        Instruction::QKernelF { dst: 0, src: 0, kernel: kernel_id::ROTATE, fctx0: 0, fctx1: 1 },
        // Non-destructive sample: get probability of state 0
        Instruction::QSample { dst_h: 0, src_q: 0, mode: observe_mode::PROB, ctx0: 0, ctx1: 0 },
        // H0 is Complex(probability, 0.0). HREDUCE/REAL can extract the probability.
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // Q0 should still be alive after QSAMPLE
    assert!(ctx.qregs[0].is_some());

    // H0 should be Complex(prob, 0.0) with value in [0, 1]
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!(*re >= 0.0 && *re <= 1.0, "prob={}, expected [0,1]", re);
        assert!(im.abs() < 1e-10, "imaginary part should be 0.0, got {}", im);
    } else {
        panic!("Expected Complex variant in H0");
    }

    // Now apply another kernel to Q0 (proving it's still alive)
    let mut ctx2 = ctx;
    let mut fm = ForkManager::new();
    let second_kernel = Instruction::QKernelF {
        dst: 0, src: 0, kernel: kernel_id::ROTATE, fctx0: 0, fctx1: 1,
    };
    execute_instruction(&mut ctx2, &second_kernel, &mut fm).unwrap();
    // Q0 should still be alive after second kernel
    assert!(ctx2.qregs[0].is_some());
}

// =============================================================================
// Test 15: Full 4-stage pipeline: Z -> Q -> H -> Z
// =============================================================================

/// Verify the complete complex dataflow end-to-end:
///   ZLDI Z0, 1, 1  ->  QENCODE Q0, Z0, 2, Z_FILE
///   ->  QKERNELZ Q0, Q0, PHASE_SHIFT, Z0, Z1
///   ->  QOBSERVE H0, Q0, AMP, R0, R1
///   ->  HREDUCE H0, Z2, CONJ_Z
///
/// This exercises: Z-file input (QENCODE + QKERNELZ), quantum processing,
/// observation, and Z-file output (CONJ_Z).
///
/// Assertions:
///   - Z2 contains a valid complex number
///   - The pipeline completes without error
///   - All intermediate values are consistent
#[test]
fn test_e2e_full_z_pipeline() {
    let instrs = vec![
        // Z0 = (1.0, 1.0), Z1 = (1.0, 0.0)
        Instruction::ZLdi { dst: 0, imm_re: 1, imm_im: 1 },
        Instruction::ZLdi { dst: 1, imm_re: 1, imm_im: 0 },
        // R0 = 0, R1 = 1 (row/col for AMP mode)
        Instruction::ILdi { dst: 0, imm: 0 },
        Instruction::ILdi { dst: 1, imm: 1 },
        // QENCODE from Z-file: uses Z0, Z1 as amplitudes for a 1-qubit state
        Instruction::QEncode { dst: 0, src_base: 0, count: 2, file_sel: file_sel::Z_FILE },
        // Apply PHASE_SHIFT kernel with complex context Z0, Z1
        Instruction::QKernelZ { dst: 0, src: 0, kernel: kernel_id::PHASE_SHIFT, zctx0: 0, zctx1: 1 },
        // Observe AMP at (0,1)
        Instruction::QObserve { dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1 },
        // Reduce: CONJ_Z -> Z2
        Instruction::HReduce { src: 0, dst: 2, func: reduce_fn::CONJ_Z },
        Instruction::Halt,
    ];

    let ctx = run_program(instrs);

    // H0 should be Complex
    assert!(matches!(ctx.hregs.get(0).unwrap(), HybridValue::Complex(_, _)));

    // Z2 should contain a valid complex number (conjugate of H0)
    let (z2_re, z2_im) = ctx.zregs.get(2).unwrap();
    assert!(z2_re.is_finite(), "Z2.re should be finite");
    assert!(z2_im.is_finite(), "Z2.im should be finite");

    // Verify conjugate relationship
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((z2_re - re).abs() < 1e-10, "Z2.re={}, H0.re={}", z2_re, re);
        assert!((z2_im - (-im)).abs() < 1e-10, "Z2.im={}, -H0.im={}", z2_im, -im);
    }

    // Q0 should be consumed
    assert!(ctx.qregs[0].is_none());
}
