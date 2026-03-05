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

    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0 }).unwrap();

    assert!(ctx.qregs[0].is_none());
    // After collapse, should be a delta distribution with exactly 1 entry
    if let HybridValue::Dist(d) = ctx.hregs.get(0).unwrap() {
        assert_eq!(d.len(), 1, "Collapsed distribution should have exactly 1 entry");
        assert!((d[0].1 - 1.0).abs() < 1e-10, "Collapsed probability should be 1.0");
    } else {
        panic!("Expected HybridValue::Dist after QObserve");
    }
}

#[test]
fn test_qobserve_sets_psw_flags() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0 }).unwrap();

    assert!(ctx.psw.df);
    assert!(ctx.psw.cf);
}

#[test]
fn test_qobserve_collapses_to_delta() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::ZERO }).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0 }).unwrap();

    // Zero state has only |0> with p=1.0, so measurement must yield 0
    if let HybridValue::Dist(d) = ctx.hregs.get(0).unwrap() {
        assert_eq!(d.len(), 1, "Collapsed distribution should have exactly 1 entry");
        assert_eq!(d[0].0, 0u16, "Measured value should be 0 for zero-state");
        assert!((d[0].1 - 1.0).abs() < 1e-10, "Collapsed probability should be 1.0");
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
    let result = execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0 });
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

    // HREDUCE with MODE (11) should have written the measured state to R0
    let r0 = ctx.iregs.get(0).unwrap();
    // Bell state collapses to either |00> (0) or |11> (3)
    assert!(r0 == 0 || r0 == 3,
        "Bell state MODE should be 0 or 3, got {}", r0);

    // HREDUCE with MEAN (10) should have written the mean to F0
    let f0 = ctx.fregs.get(0).unwrap();
    // After collapse, the distribution is a delta at r0 with p=1.0
    // So mean = r0 as f64
    assert!((f0 - r0 as f64).abs() < 1e-10,
        "Mean should equal the collapsed state value, got F0={}, R0={}", f0, r0);

    // Measurement flags should be set
    assert!(ctx.psw.df, "Decoherence flag should be set after QObserve");
    assert!(ctx.psw.cf, "Collapse flag should be set after QObserve");
}
