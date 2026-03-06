//! Tests for quantum operation handlers: QPREP, QKERNEL, QOBSERVE,
//! QLOAD, and QSTORE using the `DensityMatrix` simulation backend.

use cqam_core::instruction::*;
use cqam_core::register::HybridValue;
use cqam_vm::context::ExecutionContext;
use cqam_vm::qop::execute_qop;

// =============================================================================
// QPrep distribution tests
// =============================================================================

#[test]
fn test_qprep_uniform() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    assert!(ctx.qregs[0].is_some());
    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2);
    assert_eq!(dm.dimension(), 4);
    // All diagonal probabilities should be 0.25
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6);
    }
}

#[test]
fn test_qprep_zero() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 1, dist: dist_id::ZERO }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2);
    // rho[0][0] = 1.0, all others 0
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10);
    assert!((dm.get(1, 1).0).abs() < 1e-10);
}

#[test]
fn test_qprep_bell() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 2, dist: dist_id::BELL }).unwrap();

    let dm = ctx.qregs[2].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2);
    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(0, 3).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(3, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(3, 3).0 - 0.5).abs() < 1e-10);
}

#[test]
fn test_qprep_ghz() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 3, dist: dist_id::GHZ }).unwrap();

    let dm = ctx.qregs[3].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2); // default_qubits=2 but GHZ forces n>=2
    let dim = dm.dimension();
    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(0, dim - 1).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(dim - 1, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(dim - 1, dim - 1).0 - 0.5).abs() < 1e-10);
}

// =============================================================================
// QKernel dispatch tests
// =============================================================================

#[test]
fn test_qkernel_entangle() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::ENTANGLE,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    assert!(ctx.qregs[1].is_some());
    let dm = ctx.qregs[1].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2);
    assert!(dm.is_valid(1e-8));
}

#[test]
fn test_qkernel_fourier() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::FOURIER,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    let total: f64 = probs.iter().sum();
    assert!((total - 1.0).abs() < 1e-6, "Fourier output should be normalized");

    // QFT on uniform concentrates on state 0
    assert!(
        probs[0] > 0.99,
        "QFT of uniform should concentrate on state 0, got p[0]={}",
        probs[0]
    );
}

#[test]
fn test_qkernel_diffuse() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::DIFFUSE,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    let total: f64 = probs.iter().sum();
    assert!((total - 1.0).abs() < 1e-6, "Diffuse output should be normalized");

    // Diffusion on uniform stays uniform
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6, "Diffuse on uniform should stay uniform");
    }
}

#[test]
fn test_qkernel_grover_iter() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // Set target state in integer register R0
    ctx.iregs.set(0, 3).unwrap(); // target state = 3
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::GROVER_ITER,
        ctx0: 0,  // reads R0 = 3 as target
        ctx1: 1,
    }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();

    // For N=4, 1 Grover iteration gives p(target) = 1.0
    assert!(
        (probs[3] - 1.0).abs() < 1e-10,
        "Grover should find target with certainty. p[3]={}",
        probs[3]
    );
}

#[test]
fn test_qkernel_updates_psw_with_real_metrics() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::INIT,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    // After applying init kernel (uniform output), quantum flags should be set
    assert!(ctx.psw.qf, "Quantum active flag should be set");
    // Uniform state has von Neumann entropy > 0 (all probs equal)
    assert!(ctx.psw.sf, "Superposition flag should be set for uniform distribution");
    // Purity of pure state = 1.0 > 0, so ef should be set
    assert!(ctx.psw.ef, "Entanglement flag should be set (purity > 0)");
}

// =============================================================================
// QObserve tests
// =============================================================================

#[test]
fn test_qobserve_destructive() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    assert!(ctx.qregs[0].is_some());

    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    assert!(ctx.qregs[0].is_none());
    // After QOBSERVE, the full distribution is preserved (not collapsed)
    if let HybridValue::Dist(d) = ctx.hregs.get(0).unwrap() {
        assert_eq!(d.len(), 4, "Uniform 2-qubit distribution should have 4 entries");
        let total: f64 = d.iter().map(|(_, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-10, "Total probability should be 1.0");
        for &(_, p) in d {
            assert!((p - 0.25).abs() < 1e-10, "Each probability should be ~0.25");
        }
    } else {
        panic!("Expected HybridValue::Dist after QObserve");
    }
}

#[test]
fn test_qobserve_sets_psw_flags() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    assert!(ctx.psw.df);
    assert!(ctx.psw.cf);
}

#[test]
fn test_qobserve_zero_state_single_entry() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    // Zero state has only |0> with p=1.0, rest are near-zero and filtered out
    if let HybridValue::Dist(d) = ctx.hregs.get(0).unwrap() {
        assert_eq!(d.len(), 1, "Zero-state distribution should have exactly 1 entry (others filtered)");
        assert_eq!(d[0].0, 0u16, "Only entry should be state 0");
        assert!((d[0].1 - 1.0).abs() < 1e-10, "Probability should be 1.0");
    } else {
        panic!("Expected HybridValue::Dist after QObserve");
    }
}

// =============================================================================
// QLoad / QStore tests
// =============================================================================

#[test]
fn test_qstore_and_qload() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    execute_qop(&mut ctx, &Instruction::QStore { src_q: 0, addr: 10 }).unwrap();
    assert!(ctx.qmem.is_occupied(10));

    execute_qop(&mut ctx, &Instruction::QLoad { dst_q: 2, addr: 10 }).unwrap();
    assert!(ctx.qregs[2].is_some());

    assert!(ctx.qregs[0].is_some());
}

// ===========================================================================
// Error cases
// ===========================================================================

#[test]
fn test_qkernel_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1, src: 0, kernel: kernel_id::ENTANGLE, ctx0: 0, ctx1: 1,
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Uninitialized register"));
}

#[test]
fn test_qobserve_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let result = execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 });
    assert!(result.is_err());
}

#[test]
fn test_qload_from_empty_slot_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let result = execute_qop(&mut ctx, &Instruction::QLoad { dst_q: 0, addr: 0 });
    assert!(result.is_err());
}

#[test]
fn test_qstore_from_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let result = execute_qop(&mut ctx, &Instruction::QStore { src_q: 0, addr: 0 });
    assert!(result.is_err());
}

#[test]
fn test_unknown_kernel_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1, src: 0, kernel: 99, ctx0: 0, ctx1: 1,
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Unknown kernel"));
}

#[test]
fn test_unknown_distribution_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let result = execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: 99 });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Unknown distribution ID"), "Expected UnknownDistribution error, got: {}", msg);
    assert!(msg.contains("99"), "Error should contain the bad dist ID 99, got: {}", msg);
}

#[test]
fn test_unknown_distribution_boundary_values() {
    let mut ctx = ExecutionContext::new(vec![]);

    // dist_id::GHZ (3) is the last valid ID; 4 should fail
    let result = execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: 4 });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Unknown distribution ID"));

    // Max u8 value
    let result = execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: 255 });
    assert!(result.is_err());
}

// --- End-to-end Bell state example -------------------------------------------

#[test]
fn test_bell_state_example_runs_through_vm() {
    use cqam_core::parser::parse_program;
    use cqam_vm::fork::ForkManager;
    use cqam_vm::executor::run_program;

    let source = r#"
# Bell state example
QPREP Q0, 2
QOBSERVE H0, Q0
HREDUCE H0, R0, 11
HREDUCE H0, F0, 10
HALT
"#;

    let program = parse_program(source).expect("Failed to parse bell_state program");
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();

    run_program(&mut ctx, &mut fm).expect("bell_state program failed");

    // The program should have halted
    assert!(ctx.psw.trap_halt, "Program should have halted");

    // QObserve should have consumed Q0
    assert!(ctx.qregs[0].is_none(), "Q0 should be consumed after QOBSERVE");

    // With the fixed QOBSERVE, H0 now holds the full Bell distribution:
    // [(0, 0.5), (3, 0.5)] -- two entries, not a collapsed delta.
    //
    // HREDUCE with MODE (11) returns the most probable value.
    // Both states (0 and 3) have equal probability, so MODE picks one.
    let r0 = ctx.iregs.get(0).unwrap();
    assert!(r0 == 0 || r0 == 3,
        "Bell state MODE should be 0 or 3, got {}", r0);

    // HREDUCE with MEAN (10) computes expected value: 0*0.5 + 3*0.5 = 1.5
    let f0 = ctx.fregs.get(0).unwrap();
    assert!((f0 - 1.5).abs() < 1e-10,
        "Mean of Bell distribution should be 1.5, got F0={}", f0);

    // Measurement flags should be set
    assert!(ctx.psw.df, "Decoherence flag should be set after QObserve");
    assert!(ctx.psw.cf, "Collapse flag should be set after QObserve");
}

// =============================================================================
// QOBSERVE full-distribution tests (PLAN3 Phase 1)
// =============================================================================

#[test]
fn test_qobserve_preserves_full_distribution() {
    let mut ctx = ExecutionContext::new(vec![]);

    // QPREP with UNIFORM distribution: 2 qubits = 4 basis states, each p=0.25
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    if let HybridValue::Dist(pairs) = ctx.hregs.get(0).unwrap() {
        assert_eq!(pairs.len(), 4, "Uniform 2-qubit distribution should have 4 entries");
        let total: f64 = pairs.iter().map(|(_, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-10, "Total probability should be 1.0, got {}", total);
        for &(_, p) in pairs {
            assert!((p - 0.25).abs() < 1e-10, "Each probability should be ~0.25, got {}", p);
        }
    } else {
        panic!("Expected HybridValue::Dist after QOBSERVE");
    }
}

#[test]
fn test_qobserve_consumes_q_register() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    assert!(ctx.qregs[0].is_some(), "Q[0] should be Some after QPREP");

    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();
    assert!(ctx.qregs[0].is_none(), "Q[0] should be None after QOBSERVE (destructive)");
}

// =============================================================================
// QSAMPLE tests (PLAN3 Phase 1)
// =============================================================================

#[test]
fn test_qsample_preserves_q_register() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    assert!(ctx.qregs[0].is_some());

    execute_qop(&mut ctx, &Instruction::QSample { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be Some after QSAMPLE (non-destructive)");
}

#[test]
fn test_qsample_produces_valid_distribution() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Bell state: |00> and |11> each with p=0.5
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::BELL }).unwrap();
    execute_qop(&mut ctx, &Instruction::QSample { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    if let HybridValue::Dist(pairs) = ctx.hregs.get(0).unwrap() {
        assert_eq!(pairs.len(), 2, "Bell state should have exactly 2 entries, got {}", pairs.len());
        let total: f64 = pairs.iter().map(|(_, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-10, "Total probability should be 1.0");
        for &(_, p) in pairs {
            assert!((p - 0.5).abs() < 1e-10, "Each Bell probability should be ~0.5, got {}", p);
        }
    } else {
        panic!("Expected HybridValue::Dist after QSAMPLE");
    }
}

#[test]
fn test_qsample_then_qkernel() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare Q[0] with UNIFORM distribution
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // QSAMPLE: non-destructive read of Q[0] into H[0]
    execute_qop(&mut ctx, &Instruction::QSample { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    // Verify H[0] has a valid distribution
    if let HybridValue::Dist(pairs) = ctx.hregs.get(0).unwrap() {
        assert_eq!(pairs.len(), 4, "Pre-kernel sample should have 4 entries");
    } else {
        panic!("Expected HybridValue::Dist after QSAMPLE");
    }

    // Q[0] should still be live for QKERNEL
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be live after QSAMPLE");

    // Apply INIT kernel: Q[1] = init(Q[0])
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();
    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::INIT,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    // Q[1] should hold the kernel result
    assert!(ctx.qregs[1].is_some(), "Q[1] should hold INIT kernel result");

    // Q[0] should still be live (QSAMPLE did not consume it, QKERNEL borrows src)
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be live after QKERNEL (src is borrowed)");
}

#[test]
fn test_qsample_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let result = execute_qop(&mut ctx, &Instruction::QSample { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Uninitialized register"), "Expected UninitializedRegister error, got: {}", msg);
}

// =============================================================================
// QSAMPLE does NOT set measured flag (PLAN3 Phase 1 edge case)
// =============================================================================

#[test]
fn test_qsample_does_not_set_measured_flags() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare and sample -- should NOT set df/cf
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    execute_qop(&mut ctx, &Instruction::QSample { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    assert!(!ctx.psw.df, "Decoherence flag (df) should NOT be set after QSAMPLE");
    assert!(!ctx.psw.cf, "Collapse flag (cf) should NOT be set after QSAMPLE");
}

// =============================================================================
// QSAMPLE on single-qubit register (edge case)
// =============================================================================

#[test]
fn test_qsample_single_qubit_register() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.config.default_qubits = 1; // 1 qubit = 2 basis states

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    execute_qop(&mut ctx, &Instruction::QSample { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    // Q[0] should still be live
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be live after QSAMPLE on 1-qubit register");

    if let HybridValue::Dist(pairs) = ctx.hregs.get(0).unwrap() {
        assert_eq!(pairs.len(), 2, "1-qubit uniform should have 2 entries, got {}", pairs.len());
        let total: f64 = pairs.iter().map(|(_, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-10, "Total probability should be 1.0");
        for &(_, p) in pairs {
            assert!((p - 0.5).abs() < 1e-10, "Each probability should be ~0.5, got {}", p);
        }
    } else {
        panic!("Expected HybridValue::Dist after QSAMPLE on 1-qubit register");
    }
}

// =============================================================================
// QOBSERVE on GHZ state (entangled distribution shape)
// =============================================================================

#[test]
fn test_qobserve_ghz_state_distribution_shape() {
    let mut ctx = ExecutionContext::new(vec![]);

    // GHZ state with default 2 qubits: (|00> + |11>)/sqrt(2)
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::GHZ }).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 }).unwrap();

    // Q[0] should be consumed
    assert!(ctx.qregs[0].is_none(), "Q[0] should be consumed after QOBSERVE");

    if let HybridValue::Dist(pairs) = ctx.hregs.get(0).unwrap() {
        // GHZ with 2 qubits has only states 0 (|00>) and 3 (|11>)
        assert_eq!(pairs.len(), 2, "GHZ 2-qubit should have exactly 2 entries, got {}", pairs.len());
        let total: f64 = pairs.iter().map(|(_, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-10, "Total probability should be 1.0");

        // Verify the states are 0 and 3 (or dim-1)
        let states: Vec<u16> = pairs.iter().map(|(s, _)| *s).collect();
        assert!(states.contains(&0), "GHZ distribution should contain state 0");
        assert!(states.contains(&3), "GHZ distribution should contain state 3 (dim-1)");

        // Each should have probability 0.5
        for &(_, p) in pairs {
            assert!((p - 0.5).abs() < 1e-10, "Each GHZ probability should be ~0.5, got {}", p);
        }
    } else {
        panic!("Expected HybridValue::Dist after QOBSERVE on GHZ state");
    }
}

// =============================================================================
// Integration: QSAMPLE -> HREDUCE pipeline (PLAN3 Phase 1)
// =============================================================================

#[test]
fn test_qsample_hreduce_pipeline() {
    use cqam_core::parser::parse_program;
    use cqam_vm::fork::ForkManager;
    use cqam_vm::executor::run_program;

    // QSAMPLE non-destructively reads Q0 into H0, then reduces with MEAN/MODE/VARIANCE
    let source = r#"
# Prepare uniform distribution: 4 states, each p=0.25
QPREP Q0, 0
# Non-destructive sample
QSAMPLE H0, Q0
# MEAN: 0*0.25 + 1*0.25 + 2*0.25 + 3*0.25 = 1.5
HREDUCE H0, F0, 10
# MODE: all equal, so max_by picks one (implementation-defined, but must be 0-3)
HREDUCE H0, R0, 11
# VARIANCE: E[X^2] - E[X]^2 = (0+1+4+9)*0.25 - 1.5^2 = 3.5 - 2.25 = 1.25
HREDUCE H0, F1, 13
HALT
"#;

    let program = parse_program(source).expect("Failed to parse QSAMPLE pipeline program");
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();

    run_program(&mut ctx, &mut fm).expect("QSAMPLE pipeline program failed");

    assert!(ctx.psw.trap_halt, "Program should have halted");

    // Q0 should still be live (QSAMPLE is non-destructive)
    assert!(ctx.qregs[0].is_some(), "Q0 should still be live after QSAMPLE (non-destructive)");

    // MEAN of uniform(0,1,2,3) = 1.5
    let f0 = ctx.fregs.get(0).unwrap();
    assert!((f0 - 1.5).abs() < 1e-10,
        "Mean of uniform(0,1,2,3) should be 1.5, got F0={}", f0);

    // MODE: all probabilities equal, implementation picks one of {0,1,2,3}
    let r0 = ctx.iregs.get(0).unwrap();
    assert!((0..=3).contains(&r0),
        "Mode of uniform should be in 0..=3, got R0={}", r0);

    // VARIANCE of uniform(0,1,2,3) = sum((x - 1.5)^2 * 0.25)
    // = (2.25 + 0.25 + 0.25 + 2.25) * 0.25 = 5.0 * 0.25 = 1.25
    let f1 = ctx.fregs.get(1).unwrap();
    assert!((f1 - 1.25).abs() < 1e-10,
        "Variance of uniform(0,1,2,3) should be 1.25, got F1={}", f1);

    // PSW: df and cf should NOT be set (QSAMPLE does not measure)
    assert!(!ctx.psw.df, "Decoherence flag should NOT be set after QSAMPLE pipeline");
    assert!(!ctx.psw.cf, "Collapse flag should NOT be set after QSAMPLE pipeline");
}

// =============================================================================
// QOBSERVE/QSAMPLE mode dispatch tests (PLAN3 Phase 2)
// =============================================================================

#[test]
fn test_qobserve_mode_prob() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare zero state: |0> with probability 1.0
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();

    // Set ctx0 = 0 (query probability of basis state 0)
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: observe_mode::PROB, ctx0: 0, ctx1: 0,
    }).unwrap();

    // Should be destructive
    assert!(ctx.qregs[0].is_none(), "Q[0] should be consumed after QOBSERVE/PROB");

    // H[0] should hold Float(1.0) -- probability of |0> in zero state
    if let HybridValue::Float(p) = ctx.hregs.get(0).unwrap() {
        assert!((p - 1.0).abs() < 1e-10, "p(|0>) in zero state should be 1.0, got {}", p);
    } else {
        panic!("Expected HybridValue::Float after QOBSERVE/PROB");
    }
}

#[test]
fn test_qobserve_mode_amp() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare zero state: rho[0][0] = 1.0
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();

    // Set ctx0 = 0 (row), ctx1 = 0 (col) -> rho[0][0]
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1,
    }).unwrap();

    // Should be destructive
    assert!(ctx.qregs[0].is_none(), "Q[0] should be consumed after QOBSERVE/AMP");

    // H[0] should hold Complex(1.0, 0.0) -- rho[0][0] of zero state
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((re - 1.0).abs() < 1e-10, "re(rho[0][0]) should be 1.0, got {}", re);
        assert!(im.abs() < 1e-10, "im(rho[0][0]) should be 0.0, got {}", im);
    } else {
        panic!("Expected HybridValue::Complex after QOBSERVE/AMP");
    }
}

#[test]
fn test_qsample_mode_prob() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare uniform state: each |k> has probability 0.25
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // Set ctx0 = 2 (query probability of basis state 2)
    ctx.iregs.set(0, 2).unwrap();

    execute_qop(&mut ctx, &Instruction::QSample {
        dst_h: 0, src_q: 0, mode: observe_mode::PROB, ctx0: 0, ctx1: 0,
    }).unwrap();

    // Should be non-destructive
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be live after QSAMPLE/PROB");

    // H[0] should hold Float(0.25) -- probability of |2> in uniform state
    if let HybridValue::Float(p) = ctx.hregs.get(0).unwrap() {
        assert!((p - 0.25).abs() < 1e-10, "p(|2>) in uniform state should be 0.25, got {}", p);
    } else {
        panic!("Expected HybridValue::Float after QSAMPLE/PROB");
    }
}

#[test]
fn test_qsample_mode_amp() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare Bell state: rho[0][0]=0.5, rho[0][3]=0.5, rho[3][0]=0.5, rho[3][3]=0.5
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::BELL }).unwrap();

    // Query rho[0][3] -> should be 0.5 + 0i
    ctx.iregs.set(0, 0).unwrap(); // row
    ctx.iregs.set(1, 3).unwrap(); // col

    execute_qop(&mut ctx, &Instruction::QSample {
        dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1,
    }).unwrap();

    // Should be non-destructive
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be live after QSAMPLE/AMP");

    // H[0] should hold Complex(0.5, 0.0)
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((re - 0.5).abs() < 1e-10, "re(rho[0][3]) should be 0.5, got {}", re);
        assert!(im.abs() < 1e-10, "im(rho[0][3]) should be 0.0, got {}", im);
    } else {
        panic!("Expected HybridValue::Complex after QSAMPLE/AMP");
    }
}

#[test]
fn test_qobserve_mode_prob_out_of_range() {
    let mut ctx = ExecutionContext::new(vec![]);

    // 2 qubits -> dimension 4; index 4 is out of range
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 4).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: observe_mode::PROB, ctx0: 0, ctx1: 0,
    });
    assert!(result.is_err(), "QOBSERVE/PROB with out-of-range index should error");
}

#[test]
fn test_qobserve_mode_amp_out_of_range() {
    let mut ctx = ExecutionContext::new(vec![]);

    // 2 qubits -> dimension 4; row=5 is out of range
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 5).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1,
    });
    assert!(result.is_err(), "QOBSERVE/AMP with out-of-range row should error");
}

// =============================================================================
// End-to-end pipeline tests (PLAN3 Phase 2)
// =============================================================================

/// QPREP -> QOBSERVE(AMP) -> HREDUCE(CONJ_Z) -> verify Z register.
///
/// Pipeline: prepare a Bell state, extract rho[0][3] as a complex amplitude,
/// then conjugate it into the Z register file.
#[test]
fn test_e2e_observe_amp_then_conj_z() {
    use cqam_vm::fork::ForkManager;
    use cqam_vm::hybrid::execute_hybrid;

    let mut ctx = ExecutionContext::new(vec![]);

    // Bell state: rho[0][3] = 0.5 + 0i
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::BELL }).unwrap();

    // Set ctx0=0 (row), ctx1=R1 where R1=3 (col)
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 3).unwrap();

    // QOBSERVE in AMP mode: H[0] = rho[0][3] = Complex(0.5, 0.0)
    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1,
    }).unwrap();

    // Q[0] consumed
    assert!(ctx.qregs[0].is_none());

    // H[0] should be Complex(0.5, 0.0)
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((re - 0.5).abs() < 1e-10);
        assert!(im.abs() < 1e-10);
    } else {
        panic!("Expected HybridValue::Complex after QOBSERVE/AMP");
    }

    // HREDUCE with CONJ_Z: Z[2] = conj(0.5 + 0i) = (0.5, -0.0)
    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 2, func: reduce_fn::CONJ_Z },
        &mut fm,
    ).unwrap();

    let (re, im) = ctx.zregs.get(2).unwrap();
    assert!((re - 0.5).abs() < 1e-10, "Z[2].re should be 0.5, got {}", re);
    // conj of 0.0 is -0.0 which equals 0.0 in floating point comparison
    assert!(im.abs() < 1e-10, "Z[2].im should be ~0.0, got {}", im);
}

/// QPREP -> QOBSERVE(PROB) -> HREDUCE(ROUND) -> verify R register.
///
/// Pipeline: prepare a uniform state, extract probability of state 2,
/// then round to integer.
#[test]
fn test_e2e_observe_prob_then_round() {
    use cqam_vm::fork::ForkManager;
    use cqam_vm::hybrid::execute_hybrid;

    let mut ctx = ExecutionContext::new(vec![]);

    // Uniform state: p(|2>) = 0.25
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // ctx0 = R0 = 2 (query probability of basis state 2)
    ctx.iregs.set(0, 2).unwrap();

    // QOBSERVE in PROB mode: H[0] = Float(0.25)
    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: observe_mode::PROB, ctx0: 0, ctx1: 0,
    }).unwrap();

    // Q[0] consumed
    assert!(ctx.qregs[0].is_none());

    if let HybridValue::Float(p) = ctx.hregs.get(0).unwrap() {
        assert!((p - 0.25).abs() < 1e-10, "prob should be 0.25, got {}", p);
    } else {
        panic!("Expected HybridValue::Float after QOBSERVE/PROB");
    }

    // HREDUCE with ROUND: R[3] = round(0.25) = 0
    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 3, func: reduce_fn::ROUND },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(3).unwrap(), 0, "round(0.25) should be 0");
}

/// QSAMPLE(AMP) followed by QKERNEL on same register.
///
/// Verifies non-destructive behavior: after QSAMPLE/AMP, the quantum
/// register is still live and can be used by QKERNEL.
#[test]
fn test_qsample_amp_then_qkernel_non_destructive() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare zero state: rho[0][0] = 1.0
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();

    // Set row=0, col=0 for AMP query
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    // QSAMPLE in AMP mode: H[0] = Complex(1.0, 0.0) -- non-destructive
    execute_qop(&mut ctx, &Instruction::QSample {
        dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1,
    }).unwrap();

    // Q[0] should still be alive
    assert!(ctx.qregs[0].is_some(), "Q[0] should be alive after QSAMPLE/AMP");

    // Verify H[0] is correct
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((re - 1.0).abs() < 1e-10);
        assert!(im.abs() < 1e-10);
    } else {
        panic!("Expected HybridValue::Complex after QSAMPLE/AMP");
    }

    // Now apply QKERNEL on the same register -- should succeed
    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1, src: 0, kernel: kernel_id::INIT, ctx0: 0, ctx1: 1,
    }).unwrap();

    // Q[1] should hold the result, Q[0] still alive
    assert!(ctx.qregs[1].is_some(), "Q[1] should hold INIT kernel result");
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be alive after QKERNEL");
}

/// QSAMPLE/PROB with out-of-range index should return error.
#[test]
fn test_qsample_mode_prob_out_of_range() {
    let mut ctx = ExecutionContext::new(vec![]);

    // 2 qubits -> dimension 4; index 4 is out of range
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 4).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QSample {
        dst_h: 0, src_q: 0, mode: observe_mode::PROB, ctx0: 0, ctx1: 0,
    });
    assert!(result.is_err(), "QSAMPLE/PROB with out-of-range index should error");

    // Q[0] should still be alive (QSAMPLE is non-destructive, even on error)
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be alive after failed QSAMPLE");
}

/// QSAMPLE/AMP with out-of-range row should return error.
#[test]
fn test_qsample_mode_amp_out_of_range() {
    let mut ctx = ExecutionContext::new(vec![]);

    // 2 qubits -> dimension 4; row=5 is out of range
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 5).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QSample {
        dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1,
    });
    assert!(result.is_err(), "QSAMPLE/AMP with out-of-range row should error");

    // Q[0] should still be alive (QSAMPLE is non-destructive)
    assert!(ctx.qregs[0].is_some(), "Q[0] should still be alive after failed QSAMPLE");
}

// =============================================================================
// QKERNELF tests (Phase 3)
// =============================================================================

#[test]
fn test_qkernelf_rotate() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare uniform state
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // Load theta = PI/4 into F[0], F[1] = 0.0
    ctx.fregs.set(0, std::f64::consts::FRAC_PI_4).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1,
        src: 0,
        kernel: kernel_id::ROTATE,
        fctx0: 0,
        fctx1: 1,
    }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert!(dm.is_valid(1e-8), "Output should be a valid density matrix");

    // Diagonal probabilities should be preserved (diagonal unitary)
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10,
            "Rotate should preserve diagonal probs, got {}", p);
    }
}

#[test]
fn test_qkernelf_rotate_zero_angle() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // theta = 0 -> identity
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    // Get input state for comparison
    let input_probs = ctx.qregs[0].as_ref().unwrap().diagonal_probabilities();

    execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1,
        src: 0,
        kernel: kernel_id::ROTATE,
        fctx0: 0,
        fctx1: 1,
    }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let output_probs = dm.diagonal_probabilities();
    for (i, (&pi, &po)) in input_probs.iter().zip(output_probs.iter()).enumerate() {
        assert!((pi - po).abs() < 1e-10,
            "Rotate(0) should be identity: p[{}] in={}, out={}", i, pi, po);
    }
}

#[test]
fn test_qkernelz_phase_shift() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // Load complex amplitude (1.0, 0.5) into Z[0], Z[1] = (0,0)
    ctx.zregs.set(0, (1.0, 0.5)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelZ {
        dst: 1,
        src: 0,
        kernel: kernel_id::PHASE_SHIFT,
        zctx0: 0,
        zctx1: 1,
    }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert!(dm.is_valid(1e-8), "Output should be a valid density matrix");

    // Diagonal probabilities should be preserved (diagonal unitary)
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10,
            "PhaseShift should preserve diagonal probs, got {}", p);
    }

    // Purity should be preserved
    assert!((dm.purity() - 1.0).abs() < 1e-10,
        "PhaseShift should preserve purity, got {}", dm.purity());
}

#[test]
fn test_qkernelf_existing_kernels() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Prepare uniform state
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // Use QKERNELF with Init kernel (should work, ignoring float params)
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1,
        src: 0,
        kernel: kernel_id::INIT,
        fctx0: 0,
        fctx1: 1,
    }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert!(dm.is_valid(1e-8));

    // Init on any state produces uniform superposition
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10,
            "Init kernel via QKERNELF should produce uniform, got {}", p);
    }

    // Also test Fourier kernel via QKERNELF
    execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 2,
        src: 1,
        kernel: kernel_id::FOURIER,
        fctx0: 0,
        fctx1: 1,
    }).unwrap();

    let dm2 = ctx.qregs[2].as_ref().unwrap();
    assert!(dm2.is_valid(1e-8));
    // QFT on uniform concentrates on state 0
    let probs2 = dm2.diagonal_probabilities();
    assert!(probs2[0] > 0.99,
        "QFT of uniform via QKERNELF should concentrate on state 0, got p[0]={}", probs2[0]);
}

// =============================================================================
// QKERNELF error cases (Phase 3)
// =============================================================================

#[test]
fn test_qkernelf_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1, src: 0, kernel: kernel_id::ROTATE, fctx0: 0, fctx1: 1,
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Uninitialized register"),
        "Expected UninitializedRegister error, got: {}", msg);
}

#[test]
fn test_qkernelf_unknown_kernel_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1, src: 0, kernel: 99, fctx0: 0, fctx1: 1,
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Unknown kernel"),
        "Expected UnknownKernel error, got: {}", msg);
}

// =============================================================================
// QKERNELZ error cases (Phase 3)
// =============================================================================

#[test]
fn test_qkernelz_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.zregs.set(0, (0.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernelZ {
        dst: 1, src: 0, kernel: kernel_id::PHASE_SHIFT, zctx0: 0, zctx1: 1,
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Uninitialized register"),
        "Expected UninitializedRegister error, got: {}", msg);
}

#[test]
fn test_qkernelz_unknown_kernel_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.zregs.set(0, (0.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernelZ {
        dst: 1, src: 0, kernel: 99, zctx0: 0, zctx1: 1,
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Unknown kernel"),
        "Expected UnknownKernel error, got: {}", msg);
}

// =============================================================================
// End-to-end: QPREP -> QKERNELF(ROTATE) -> QOBSERVE -> HREDUCE(MEAN)
// =============================================================================

#[test]
fn test_e2e_qkernelf_rotate_observe_reduce() {
    use cqam_core::parser::parse_program;
    use cqam_vm::fork::ForkManager;
    use cqam_vm::executor::run_program;

    // Pipeline: prepare uniform state, apply Rotate(0) which is identity,
    // observe the distribution, then reduce to MEAN.
    // Uniform(0,1,2,3) with p=0.25 each -> MEAN = 1.5
    let source = r#"
# Prepare uniform 2-qubit state
QPREP Q0, 0
# Load theta = 0 into F0 (identity rotation)
FLDI F0, 0
FLDI F1, 0
# Apply QKERNELF with ROTATE kernel (kernel_id=5)
QKERNELF Q1, Q0, 5, F0, F1
# Observe Q1 -> H0 (full distribution)
QOBSERVE H0, Q1
# Reduce to mean
HREDUCE H0, F2, 10
HALT
"#;

    let program = parse_program(source).expect("Failed to parse QKERNELF pipeline");
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();

    run_program(&mut ctx, &mut fm).expect("QKERNELF pipeline failed");

    assert!(ctx.psw.trap_halt, "Program should have halted");

    // Q1 was consumed by QOBSERVE
    assert!(ctx.qregs[1].is_none(), "Q1 should be consumed after QOBSERVE");

    // MEAN of uniform(0,1,2,3) = 1.5
    let f2 = ctx.fregs.get(2).unwrap();
    assert!((f2 - 1.5).abs() < 1e-10,
        "Mean of uniform after identity Rotate should be 1.5, got F2={}", f2);
}

// =============================================================================
// End-to-end: QPREP -> QKERNELZ(PHASE_SHIFT) -> QSAMPLE -> verify
// =============================================================================

#[test]
fn test_e2e_qkernelz_phase_shift_sample() {
    use cqam_core::parser::parse_program;
    use cqam_vm::fork::ForkManager;
    use cqam_vm::executor::run_program;

    // Pipeline: prepare uniform state, apply PhaseShift with zero amplitude
    // (which is identity), then sample the distribution.
    // Diagonal probabilities should be 0.25 each.
    let source = r#"
# Prepare uniform 2-qubit state
QPREP Q0, 0
# Load amplitude = (0, 0) into Z0 (zero amplitude -> identity)
ZLDI Z0, 0, 0
ZLDI Z1, 0, 0
# Apply QKERNELZ with PHASE_SHIFT kernel (kernel_id=6)
QKERNELZ Q1, Q0, 6, Z0, Z1
# Non-destructive sample Q1 -> H0
QSAMPLE H0, Q1
# Reduce to mean
HREDUCE H0, F0, 10
HALT
"#;

    let program = parse_program(source).expect("Failed to parse QKERNELZ pipeline");
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();

    run_program(&mut ctx, &mut fm).expect("QKERNELZ pipeline failed");

    assert!(ctx.psw.trap_halt, "Program should have halted");

    // Q1 should still be live (QSAMPLE is non-destructive)
    assert!(ctx.qregs[1].is_some(), "Q1 should be live after QSAMPLE");

    // MEAN of uniform(0,1,2,3) = 1.5
    let f0 = ctx.fregs.get(0).unwrap();
    assert!((f0 - 1.5).abs() < 1e-10,
        "Mean of uniform after identity PhaseShift should be 1.5, got F0={}", f0);

    // Verify the distribution in H0
    if let HybridValue::Dist(pairs) = ctx.hregs.get(0).unwrap() {
        assert_eq!(pairs.len(), 4,
            "Uniform 2-qubit distribution should have 4 entries, got {}", pairs.len());
        for &(_, p) in pairs {
            assert!((p - 0.25).abs() < 1e-10,
                "Each probability should be ~0.25 after identity PhaseShift, got {}", p);
        }
    } else {
        panic!("Expected HybridValue::Dist after QSAMPLE");
    }
}

// =============================================================================
// QKERNELZ with non-zero complex amplitude: verify diagonal preservation
// =============================================================================

#[test]
fn test_qkernelz_phase_shift_nonzero_preserves_diag() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // amplitude = (0.0, 2.0) -> |z| = 2.0, purely imaginary
    ctx.zregs.set(0, (0.0, 2.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelZ {
        dst: 1,
        src: 0,
        kernel: kernel_id::PHASE_SHIFT,
        zctx0: 0,
        zctx1: 1,
    }).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert!(dm.is_valid(1e-8), "Output should be a valid density matrix");

    // Diagonal probabilities should be preserved (diagonal unitary)
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10,
            "PhaseShift with purely imaginary amplitude should preserve diagonal probs, got {}", p);
    }

    assert!((dm.purity() - 1.0).abs() < 1e-10,
        "PhaseShift should preserve purity, got {}", dm.purity());
}

// =============================================================================
// QPrepR tests (Phase 4)
// =============================================================================

#[test]
fn test_qprepr_uniform() {
    let mut ctx = ExecutionContext::new(vec![]);
    // Set R[0] = 0 (UNIFORM)
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 }).unwrap();

    assert!(ctx.qregs[0].is_some());
    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6, "Expected uniform 0.25, got {}", p);
    }
}

#[test]
fn test_qprepr_bell() {
    let mut ctx = ExecutionContext::new(vec![]);
    // Set R[0] = 2 (BELL)
    ctx.iregs.set(0, 2).unwrap();

    execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2);
    // Bell state: |00> and |11> with equal probability
    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(3, 3).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(1, 1).0).abs() < 1e-10);
    assert!((dm.get(2, 2).0).abs() < 1e-10);
}

#[test]
fn test_qprepr_invalid_dist() {
    let mut ctx = ExecutionContext::new(vec![]);
    // Set R[0] = 99 (invalid)
    ctx.iregs.set(0, 99).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 });
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("distribution") || err_msg.contains("99"),
        "Error should mention unknown distribution, got: {}", err_msg);
}

// =============================================================================
// QEncode tests (Phase 4)
// =============================================================================

#[test]
fn test_qencode_from_int() {
    let mut ctx = ExecutionContext::new(vec![]);
    // Load R[0..3] with [1, 1, 1, 1]
    for i in 0..4u8 {
        ctx.iregs.set(i, 1).unwrap();
    }

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 4, file_sel: 0,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2);
    // All equal amplitudes -> uniform-ish state
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6, "Expected ~0.25, got {}", p);
    }
}

#[test]
fn test_qencode_from_float() {
    let mut ctx = ExecutionContext::new(vec![]);
    // Load F[0..3] with [0.5, 0.5, 0.5, 0.5]
    for i in 0..4u8 {
        ctx.fregs.set(i, 0.5).unwrap();
    }

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 4, file_sel: 1,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2);
    // All equal amplitudes -> uniform state after normalization
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6, "Expected ~0.25, got {}", p);
    }
}

#[test]
fn test_qencode_from_complex() {
    let mut ctx = ExecutionContext::new(vec![]);
    // Z[0] = (1, 0), Z[1] = (0, 0) -> |0> state after normalization
    ctx.zregs.set(0, (1.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: 2,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 1);
    // |0> state: prob(0) = 1, prob(1) = 0
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10);
    assert!((dm.get(1, 1).0).abs() < 1e-10);
}

#[test]
fn test_qencode_non_power_of_2() {
    let mut ctx = ExecutionContext::new(vec![]);
    for i in 0..3u8 {
        ctx.fregs.set(i, 1.0).unwrap();
    }

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 3, file_sel: 1,
    });
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("power of 2"), "Error should mention power of 2, got: {}", err_msg);
}

#[test]
fn test_qencode_normalizes() {
    let mut ctx = ExecutionContext::new(vec![]);
    // Load unnormalized values: F[0]=3.0, F[1]=4.0
    ctx.fregs.set(0, 3.0).unwrap();
    ctx.fregs.set(1, 4.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: 1,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    // Trace should be 1 (normalized)
    let trace = dm.get(0, 0).0 + dm.get(1, 1).0;
    assert!((trace - 1.0).abs() < 1e-10,
        "Trace should be 1.0 after normalization, got {}", trace);
    // Specific probabilities: |3/5|^2 = 9/25, |4/5|^2 = 16/25
    assert!((dm.get(0, 0).0 - 9.0/25.0).abs() < 1e-10);
    assert!((dm.get(1, 1).0 - 16.0/25.0).abs() < 1e-10);
}

#[test]
fn test_qencode_count_zero() {
    let mut ctx = ExecutionContext::new(vec![]);

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 0, file_sel: 0,
    });
    assert!(result.is_err());
}

#[test]
fn test_qencode_zero_norm() {
    let mut ctx = ExecutionContext::new(vec![]);
    // All-zero statevector
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: 1,
    });
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("zero norm"), "Error should mention zero norm, got: {}", err_msg);
}

// =============================================================================
// Phase 4 additional coverage: QPrepR equivalence with QPrep
// =============================================================================

/// QPrepR with each dist_id must produce the same density matrix as QPrep
/// with the same dist literal. This validates that the register-indirect path
/// dispatches through the same DensityMatrix constructors.
#[test]
fn test_qprepr_matches_qprep_uniform() {
    let mut ctx_lit = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx_lit, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    let mut ctx_reg = ExecutionContext::new(vec![]);
    ctx_reg.iregs.set(0, dist_id::UNIFORM as i64).unwrap();
    execute_qop(&mut ctx_reg, &Instruction::QPrepR { dst: 0, dist_reg: 0 }).unwrap();

    let dm_lit = ctx_lit.qregs[0].as_ref().unwrap();
    let dm_reg = ctx_reg.qregs[0].as_ref().unwrap();
    assert_eq!(dm_lit.num_qubits(), dm_reg.num_qubits());
    assert_eq!(dm_lit.dimension(), dm_reg.dimension());
    for i in 0..dm_lit.dimension() {
        for j in 0..dm_lit.dimension() {
            let (r1, i1) = dm_lit.get(i, j);
            let (r2, i2) = dm_reg.get(i, j);
            assert!((r1 - r2).abs() < 1e-15 && (i1 - i2).abs() < 1e-15,
                "UNIFORM mismatch at ({},{}): ({},{}) vs ({},{})", i, j, r1, i1, r2, i2);
        }
    }
}

#[test]
fn test_qprepr_matches_qprep_zero() {
    let mut ctx_lit = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx_lit, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();

    let mut ctx_reg = ExecutionContext::new(vec![]);
    ctx_reg.iregs.set(0, dist_id::ZERO as i64).unwrap();
    execute_qop(&mut ctx_reg, &Instruction::QPrepR { dst: 0, dist_reg: 0 }).unwrap();

    let dm_lit = ctx_lit.qregs[0].as_ref().unwrap();
    let dm_reg = ctx_reg.qregs[0].as_ref().unwrap();
    for i in 0..dm_lit.dimension() {
        for j in 0..dm_lit.dimension() {
            let (r1, i1) = dm_lit.get(i, j);
            let (r2, i2) = dm_reg.get(i, j);
            assert!((r1 - r2).abs() < 1e-15 && (i1 - i2).abs() < 1e-15,
                "ZERO mismatch at ({},{})", i, j);
        }
    }
}

#[test]
fn test_qprepr_matches_qprep_bell() {
    let mut ctx_lit = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx_lit, &Instruction::QPrep { dst: 0, dist: dist_id::BELL }).unwrap();

    let mut ctx_reg = ExecutionContext::new(vec![]);
    ctx_reg.iregs.set(0, dist_id::BELL as i64).unwrap();
    execute_qop(&mut ctx_reg, &Instruction::QPrepR { dst: 0, dist_reg: 0 }).unwrap();

    let dm_lit = ctx_lit.qregs[0].as_ref().unwrap();
    let dm_reg = ctx_reg.qregs[0].as_ref().unwrap();
    for i in 0..dm_lit.dimension() {
        for j in 0..dm_lit.dimension() {
            let (r1, i1) = dm_lit.get(i, j);
            let (r2, i2) = dm_reg.get(i, j);
            assert!((r1 - r2).abs() < 1e-15 && (i1 - i2).abs() < 1e-15,
                "BELL mismatch at ({},{})", i, j);
        }
    }
}

#[test]
fn test_qprepr_matches_qprep_ghz() {
    let mut ctx_lit = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx_lit, &Instruction::QPrep { dst: 0, dist: dist_id::GHZ }).unwrap();

    let mut ctx_reg = ExecutionContext::new(vec![]);
    ctx_reg.iregs.set(0, dist_id::GHZ as i64).unwrap();
    execute_qop(&mut ctx_reg, &Instruction::QPrepR { dst: 0, dist_reg: 0 }).unwrap();

    let dm_lit = ctx_lit.qregs[0].as_ref().unwrap();
    let dm_reg = ctx_reg.qregs[0].as_ref().unwrap();
    for i in 0..dm_lit.dimension() {
        for j in 0..dm_lit.dimension() {
            let (r1, i1) = dm_lit.get(i, j);
            let (r2, i2) = dm_reg.get(i, j);
            assert!((r1 - r2).abs() < 1e-15 && (i1 - i2).abs() < 1e-15,
                "GHZ mismatch at ({},{})", i, j);
        }
    }
}

// =============================================================================
// Phase 4 additional coverage: QPrepR edge cases
// =============================================================================

/// Negative register value wraps to large u8 -> UnknownDistribution error.
#[test]
fn test_qprepr_negative_dist_id_wraps() {
    let mut ctx = ExecutionContext::new(vec![]);
    // -1i64 as u8 = 255
    ctx.iregs.set(0, -1).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 });
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("distribution") || err_msg.contains("255"),
        "Negative dist wraps to 255, error should reflect this, got: {}", err_msg);
}

/// Large positive value (256) wraps to 0 -> UNIFORM (same as dist_id=0).
#[test]
fn test_qprepr_large_value_wraps_modulo_256() {
    let mut ctx = ExecutionContext::new(vec![]);
    // 256 as u8 = 0 = UNIFORM
    ctx.iregs.set(0, 256).unwrap();

    execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 }).unwrap();
    let dm = ctx.qregs[0].as_ref().unwrap();
    // Should produce uniform (4-state, each prob = 0.25)
    let probs = dm.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6, "256 wraps to 0 (UNIFORM), expected 0.25, got {}", p);
    }
}

// =============================================================================
// Phase 4 additional coverage: QEncode count=1 (single amplitude)
// =============================================================================

/// count=1 produces a 0-qubit 1x1 density matrix. This is a degenerate case
/// where the statevector has a single amplitude, normalized to 1.
#[test]
fn test_qencode_count_1_single_amplitude() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(0, 5.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 1, file_sel: 1,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    // 1 amplitude -> dimension 1, 0 qubits
    assert_eq!(dm.dimension(), 1);
    // The single entry should be (1.0, 0.0) after normalization
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10,
        "Single amplitude DM should have rho[0][0] = 1.0, got {}", dm.get(0, 0).0);
}

// =============================================================================
// Phase 4 additional coverage: QEncode with negative R-file integers
// =============================================================================

/// Negative integers from R-file are cast to f64, producing negative amplitudes.
/// After normalization, the resulting state should have correct probabilities.
#[test]
fn test_qencode_r_file_negative_values() {
    let mut ctx = ExecutionContext::new(vec![]);
    // [-3, 4] -> amplitudes (-3, 0) and (4, 0) -> norm = 5
    // probs: 9/25 and 16/25
    ctx.iregs.set(0, -3).unwrap();
    ctx.iregs.set(1, 4).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: 0,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 1);
    assert!((dm.get(0, 0).0 - 9.0 / 25.0).abs() < 1e-10,
        "Expected prob 9/25, got {}", dm.get(0, 0).0);
    assert!((dm.get(1, 1).0 - 16.0 / 25.0).abs() < 1e-10,
        "Expected prob 16/25, got {}", dm.get(1, 1).0);
    // Off-diagonal should reflect negative sign: rho[0][1] = (-3/5)(4/5) = -12/25
    assert!((dm.get(0, 1).0 - (-12.0 / 25.0)).abs() < 1e-10,
        "Expected off-diag real = -12/25, got {}", dm.get(0, 1).0);
}

// =============================================================================
// Phase 4 additional coverage: QEncode from Z-file with complex amplitudes
// =============================================================================

/// Complex amplitudes from Z-file producing a known state.
/// Z[0] = (1/sqrt(2), 0), Z[1] = (0, 1/sqrt(2)) -> |psi> = (1+i|1>)/sqrt(2)
/// This is already normalized: |1/sqrt(2)|^2 + |i/sqrt(2)|^2 = 0.5 + 0.5 = 1.
#[test]
fn test_qencode_z_file_complex_amplitudes() {
    let mut ctx = ExecutionContext::new(vec![]);
    let s = std::f64::consts::FRAC_1_SQRT_2;
    ctx.zregs.set(0, (s, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, s)).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: 2,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 1);
    // Both diagonal entries should be 0.5
    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10,
        "Expected prob 0.5 for |0>, got {}", dm.get(0, 0).0);
    assert!((dm.get(1, 1).0 - 0.5).abs() < 1e-10,
        "Expected prob 0.5 for |1>, got {}", dm.get(1, 1).0);
    // Off-diagonal: rho[0][1] = (1/sqrt(2))(0-i/sqrt(2))* = (1/sqrt(2))(-i/sqrt(2))* = ... wait
    // rho[0][1] = psi[0] * conj(psi[1]) = (1/sqrt(2))*(0 - i/sqrt(2)) = (0, -1/2)
    assert!((dm.get(0, 1).0).abs() < 1e-10,
        "Expected off-diag real = 0, got {}", dm.get(0, 1).0);
    assert!((dm.get(0, 1).1 - (-0.5)).abs() < 1e-10,
        "Expected off-diag imag = -0.5, got {}", dm.get(0, 1).1);
}

// =============================================================================
// Phase 4 additional coverage: QEncode count=8 (max useful with 16 regs)
// =============================================================================

/// count=8 with 3-qubit state from float registers.
#[test]
fn test_qencode_count_8_from_floats() {
    let mut ctx = ExecutionContext::new(vec![]);
    // Set up |0> state: first amplitude = 1.0, rest = 0.0
    ctx.fregs.set(0, 1.0).unwrap();
    for i in 1..8u8 {
        ctx.fregs.set(i, 0.0).unwrap();
    }

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 8, file_sel: 1,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 3);
    assert_eq!(dm.dimension(), 8);
    // Should be |000> state
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10);
    for i in 1..8 {
        assert!((dm.get(i, i).0).abs() < 1e-10,
            "Non-zero diagonal at ({},{}): {}", i, i, dm.get(i, i).0);
    }
}

// =============================================================================
// Phase 4 additional coverage: QEncode src_base near register limit
// =============================================================================

/// src_base=14, count=2 should work (registers 14 and 15).
#[test]
fn test_qencode_src_base_near_limit() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(14, 1.0).unwrap();
    ctx.fregs.set(15, 0.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 14, count: 2, file_sel: 1,
    }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 1);
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10);
}

/// src_base=15, count=2 should fail: register 16 does not exist.
#[test]
fn test_qencode_src_base_overflow() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(15, 1.0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 15, count: 2, file_sel: 1,
    });
    assert!(result.is_err(),
        "src_base=15 + count=2 accesses F[16], should produce RegisterOutOfBounds");
}

// =============================================================================
// Phase 4 additional coverage: QEncode invalid file_sel at runtime
// =============================================================================

/// file_sel=3 should be caught by the parser, but if it reaches runtime
/// the match arm in qop.rs should produce an error.
#[test]
fn test_qencode_invalid_file_sel_runtime() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 1).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: 3,
    });
    assert!(result.is_err(), "file_sel=3 should produce error at runtime");
}

// =============================================================================
// Phase 4 additional coverage: End-to-end pipeline
// QEncode -> QKERNEL -> QOBSERVE -> HREDUCE
// =============================================================================

/// Full pipeline: encode a known state from F-file, apply init kernel,
/// observe, and reduce to verify the chain works end-to-end.
#[test]
fn test_qencode_observe_reduce_pipeline() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Step 1: Load F[0..3] = [1, 0, 0, 0] -> |00> state
    ctx.fregs.set(0, 1.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();
    ctx.fregs.set(2, 0.0).unwrap();
    ctx.fregs.set(3, 0.0).unwrap();

    // Step 2: QEncode Q0 from F-file
    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 4, file_sel: 1,
    }).unwrap();

    // Verify: Q0 should be |00> state
    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 2);
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10);

    // Step 3: Apply Init kernel Q1 = kernel(Q0)
    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1, src: 0, kernel: kernel_id::INIT, ctx0: 0, ctx1: 0,
    }).unwrap();
    assert!(ctx.qregs[1].is_some());

    // Step 4: Observe Q1 -> H0
    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 1, mode: observe_mode::DIST, ctx0: 0, ctx1: 0,
    }).unwrap();

    // Step 5: HReduce H0 -> R2 (argmax)
    // (We can't easily call execute_hybrid from here, so just verify
    // the observation result is a Dist.)
    match ctx.hregs.get(0).unwrap() {
        HybridValue::Dist(pairs) => {
            let total_prob: f64 = pairs.iter().map(|(_, p)| p).sum();
            assert!((total_prob - 1.0).abs() < 1e-6,
                "Observed distribution probabilities should sum to 1.0, got {}", total_prob);
        }
        other => panic!("Expected Dist after QOBSERVE, got {:?}", other),
    }
}

// =============================================================================
// QHadM / QFlip / QPhase masked gate tests
// =============================================================================

#[test]
fn test_qhadm_all_qubits() {
    // QPREP |00> -> mask=0b11 -> QHADM -> uniform over 4 states
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b11).unwrap(); // mask both qubits

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    for (i, &p) in probs.iter().enumerate() {
        assert!((p - 0.25).abs() < 1e-8, "P({}) should be 0.25, got {}", i, p);
    }
    assert!(ctx.psw.sf, "PSW superposition flag should be set after Hadamard");
}

#[test]
fn test_qhadm_single_qubit() {
    // QPREP |00> -> mask=0b01 (qubit 0 only) -> QHADM
    // P(00)=0.5, P(01)=0, P(10)=0.5, P(11)=0
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 0.5).abs() < 1e-8, "P(00) should be 0.5, got {}", probs[0]);
    assert!(probs[1].abs() < 1e-8, "P(01) should be 0, got {}", probs[1]);
    assert!((probs[2] - 0.5).abs() < 1e-8, "P(10) should be 0.5, got {}", probs[2]);
    assert!(probs[3].abs() < 1e-8, "P(11) should be 0, got {}", probs[3]);
}

#[test]
fn test_qhadm_empty_mask() {
    // mask=0 -> no-op, state unchanged
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 with empty mask");
}

#[test]
fn test_qhadm_involution() {
    // H*H = I: apply twice, state should return to original
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b11).unwrap();

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 after H*H");
}

#[test]
fn test_qhadm_different_src_dst() {
    // src != dst: src unchanged, dst gets result
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 1, src: 0, mask_reg: 0 }).unwrap();

    // Q0 unchanged
    let dm0 = ctx.qregs[0].as_ref().unwrap();
    let probs0 = dm0.diagonal_probabilities();
    assert!((probs0[0] - 1.0).abs() < 1e-8, "Q0 should remain |00>");

    // Q1 has Hadamard on qubit 0
    let dm1 = ctx.qregs[1].as_ref().unwrap();
    let probs1 = dm1.diagonal_probabilities();
    assert!((probs1[0] - 0.5).abs() < 1e-8, "Q1 P(00) should be 0.5");
    assert!((probs1[2] - 0.5).abs() < 1e-8, "Q1 P(10) should be 0.5");
}

#[test]
fn test_qflip_all_qubits() {
    // QPREP |00> -> mask=0b11 -> QFLIP -> |11>
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b11).unwrap();

    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[3] - 1.0).abs() < 1e-8, "P(11) should be 1.0 after flipping both qubits");
}

#[test]
fn test_qflip_single_qubit() {
    // QPREP |00> -> mask=0b10 -> QFLIP -> |01>
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b10).unwrap();

    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[1] - 1.0).abs() < 1e-8, "P(01) should be 1.0 after flipping qubit 1");
}

#[test]
fn test_qflip_involution() {
    // X*X = I: apply twice, state returns to original
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b11).unwrap();

    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 after X*X");
}

#[test]
fn test_qflip_empty_mask() {
    // mask=0 -> no-op
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 with empty mask");
}

#[test]
fn test_qphase_on_superposition() {
    // Prepare |+> via QHADM, then QPHASE -> |->
    // Diagonal probabilities unchanged but off-diagonal signs flip
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b01).unwrap(); // qubit 0

    // Apply H to get |+>
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    // Apply Z to get |->
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    // Diagonal probabilities should still be 50/50
    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 0.5).abs() < 1e-8, "P(00) should be 0.5");
    assert!((probs[2] - 0.5).abs() < 1e-8, "P(10) should be 0.5");

    // Applying H again should give |1> (|-> = H|1>)
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();
    let dm2 = ctx.qregs[0].as_ref().unwrap();
    let probs2 = dm2.diagonal_probabilities();
    assert!((probs2[2] - 1.0).abs() < 1e-8, "After H|-> should get |10>, P(10)=1.0");
}

#[test]
fn test_qphase_on_computational_basis() {
    // Z on |0> -> |0> (unchanged diagonal probabilities)
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 after Z on |0>");
}

#[test]
fn test_qphase_involution() {
    // Z*Z = I
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    // Put in superposition first to make it interesting
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    // Save state before QPHASE
    let probs_before: Vec<f64> = ctx.qregs[0].as_ref().unwrap()
        .diagonal_probabilities().to_vec();

    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }).unwrap();
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let probs_after = ctx.qregs[0].as_ref().unwrap().diagonal_probabilities();
    for i in 0..probs_before.len() {
        assert!((probs_before[i] - probs_after[i]).abs() < 1e-8,
            "Z*Z should be identity, state {} differs", i);
    }
}

#[test]
fn test_hadm_then_flip_then_phase() {
    // Compose all three: QHADM creates superposition, QFLIP flips, QPHASE flips phase
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    // H|0> = |+>
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();
    // X|+> = |+> (X and H commute in prob space, X|+> = |+>)
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();
    // Z|+> = |->
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    // Still 50/50 in computational basis
    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 0.5).abs() < 1e-8, "Should still be 50/50");
    assert!((probs[2] - 0.5).abs() < 1e-8, "Should still be 50/50");
}

#[test]
fn test_masked_gate_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 0b01).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 });
    assert!(result.is_err(), "QHADM on empty register should error");

    let result = execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 });
    assert!(result.is_err(), "QFLIP on empty register should error");

    let result = execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 });
    assert!(result.is_err(), "QPHASE on empty register should error");
}

#[test]
fn test_mask_bits_beyond_num_qubits_ignored() {
    // Set mask with bits beyond num_qubits (2). Extra bits should be ignored.
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    ctx.iregs.set(0, 0xFF).unwrap(); // all bits set, but only 2 qubits

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    // Should be same as mask=0b11 (uniform distribution)
    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    for (i, &p) in probs.iter().enumerate() {
        assert!((p - 0.25).abs() < 1e-8, "P({}) should be 0.25, got {}", i, p);
    }
}

// =============================================================================
// Phase 5 Revised: Additional coverage tests
// =============================================================================

/// End-to-end pipeline: ILDI mask -> QPREP -> QHADM -> QOBSERVE -> HREDUCE
#[test]
fn test_end_to_end_masked_hadamard_observe_reduce_pipeline() {
    use cqam_vm::executor::execute_instruction;
    use cqam_vm::fork::ForkManager;

    let mut ctx = ExecutionContext::new(vec![]);
    let mut fork_mgr = ForkManager::new();

    // Step 1: ILDI R0, 0b11 (mask for 2 qubits)
    execute_instruction(
        &mut ctx,
        &Instruction::ILdi { dst: 0, imm: 0b11 },
        &mut fork_mgr,
    ).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 0b11);

    // Step 2: QPREP Q0 as |00>
    execute_instruction(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: dist_id::ZERO },
        &mut fork_mgr,
    ).unwrap();

    // Step 3: QHADM Q0, Q0, R0 -> uniform superposition
    execute_instruction(
        &mut ctx,
        &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 },
        &mut fork_mgr,
    ).unwrap();

    // Step 4: QOBSERVE H0, Q0 -> distribution
    execute_instruction(
        &mut ctx,
        &Instruction::QObserve {
            dst_h: 0, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0,
        },
        &mut fork_mgr,
    ).unwrap();

    // Verify we got a distribution in H0
    match ctx.hregs.get(0).unwrap() {
        HybridValue::Dist(pairs) => {
            assert!(!pairs.is_empty(), "Distribution should have entries");
            let total: f64 = pairs.iter().map(|(_, p)| p).sum();
            assert!((total - 1.0).abs() < 1e-6, "Distribution should sum to 1.0, got {}", total);
            // All 4 states should have equal probability
            assert_eq!(pairs.len(), 4, "Uniform dist should have 4 entries");
            for (k, p) in pairs {
                assert!((p - 0.25).abs() < 1e-6,
                    "P({}) should be 0.25, got {}", k, p);
            }
        }
        other => panic!("Expected Dist after QOBSERVE, got {:?}", other),
    }

    // Step 5: HREDUCE H0 -> F0 (mean of distribution)
    execute_instruction(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MEAN },
        &mut fork_mgr,
    ).unwrap();

    // Mean of uniform {0,1,2,3} with equal probs should be 1.5
    let mean_val = ctx.fregs.get(0).unwrap();
    assert!((mean_val - 1.5).abs() < 1e-6,
        "Mean of uniform distribution over {{0,1,2,3}} should be 1.5, got {}", mean_val);
}

/// QFLIP on 3-qubit zero state with selective mask produces expected basis state.
#[test]
fn test_qflip_3_qubit_selective_mask() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.config.default_qubits = 3;

    // QPREP |000>
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(dm.num_qubits(), 3);
    assert_eq!(dm.dimension(), 8);

    // mask = 0b101 -> flip qubit 0 and qubit 2 -> |101>
    ctx.iregs.set(0, 0b101).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    // |101> in big-endian is basis state 5
    assert!((probs[5] - 1.0).abs() < 1e-8,
        "P(101) should be 1.0, got {}", probs[5]);
    // All other states zero
    for (i, &p) in probs.iter().enumerate() {
        if i != 5 {
            assert!(p.abs() < 1e-8, "P({:03b}) should be 0, got {}", i, p);
        }
    }
}

/// QFLIP all qubits on 3-qubit zero state produces |111> = basis 7.
#[test]
fn test_qflip_3_qubit_all_flipped() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.config.default_qubits = 3;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();

    // mask = 0b111 -> flip all 3 qubits
    ctx.iregs.set(0, 0b111).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[7] - 1.0).abs() < 1e-8,
        "P(111) should be 1.0 after flipping all 3 qubits, got {}", probs[7]);
}

/// QPHASE on |1> state: diagonal unchanged (Z|1> = -|1>, but prob = |-1|^2 = 1).
#[test]
fn test_qphase_on_one_state_diagonal_unchanged() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.config.default_qubits = 3;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    // Flip all qubits to get |111>
    ctx.iregs.set(0, 0b111).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let probs_before = ctx.qregs[0].as_ref().unwrap().diagonal_probabilities();

    // Apply QPHASE to all qubits
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let probs_after = ctx.qregs[0].as_ref().unwrap().diagonal_probabilities();
    for i in 0..probs_before.len() {
        assert!((probs_before[i] - probs_after[i]).abs() < 1e-8,
            "QPHASE should not change diagonal probabilities on computational basis state |111>, index {}", i);
    }
}

/// Full pipeline: QHADM -> QFLIP -> QPHASE -> QOBSERVE on 3-qubit register.
#[test]
fn test_combined_hadm_flip_phase_observe_3_qubit() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.config.default_qubits = 3;

    // QPREP |000>
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();

    // QHADM on qubit 0 only: mask = 0b001
    ctx.iregs.set(0, 0b001).unwrap();
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    // QFLIP on qubit 1 only: mask = 0b010
    ctx.iregs.set(1, 0b010).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 1 }).unwrap();

    // QPHASE on qubit 2 only: mask = 0b100
    ctx.iregs.set(2, 0b100).unwrap();
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 2 }).unwrap();

    // QOBSERVE
    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: observe_mode::DIST, ctx0: 0, ctx1: 0,
    }).unwrap();

    match ctx.hregs.get(0).unwrap() {
        HybridValue::Dist(pairs) => {
            let total: f64 = pairs.iter().map(|(_, p)| p).sum();
            assert!((total - 1.0).abs() < 1e-6, "Distribution should sum to 1.0");
            // Qubit 0 is in superposition, qubit 1 is flipped, qubit 2 has Z (no prob change)
            // State: (|0>+|1>)/sqrt(2) x |1> x |0>
            // = (|010> + |110>)/sqrt(2) = basis states 2 and 6 with P=0.5 each
            // But Z on qubit 2 (which is in |0>) doesn't change probabilities.
            assert_eq!(pairs.len(), 2, "Should have exactly 2 non-zero states");
        }
        other => panic!("Expected Dist, got {:?}", other),
    }
}

/// Mask bits beyond num_qubits are ignored: 3-qubit register with mask=0xFF.
#[test]
fn test_mask_bits_beyond_num_qubits_ignored_3_qubit() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.config.default_qubits = 3;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();

    // mask = 0xFF has 8 bits set, but only 3 qubits exist
    ctx.iregs.set(0, 0xFF).unwrap();
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    // Should be uniform over 8 states (same as mask=0b111)
    for (i, &p) in probs.iter().enumerate() {
        assert!((p - 0.125).abs() < 1e-8,
            "P({:03b}) should be 0.125 (3-qubit uniform), got {}", i, p);
    }
}

/// QHADM involution on 3-qubit register with selective mask.
#[test]
fn test_qhadm_involution_3_qubit() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.config.default_qubits = 3;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();

    // Apply H to qubits 0 and 2 (mask = 0b101)
    ctx.iregs.set(0, 0b101).unwrap();
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    // Apply H again with same mask -> should return to |000>
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = dm.diagonal_probabilities();
    assert!((probs[0] - 1.0).abs() < 1e-8,
        "H*H should return to |000>, P(000) = {}", probs[0]);
}
