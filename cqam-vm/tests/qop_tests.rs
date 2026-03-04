// cqam-vm/tests/qop_tests.rs
//
// Phase 4/6: Test quantum operations with Result-based error handling
// and Phase 6 kernels (Fourier, Diffuse, GroverIter).

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
    let qdist = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(qdist.label, "uniform");
    assert_eq!(qdist.domain.len(), 4);
    // All probabilities should be 0.25
    for &p in &qdist.probabilities {
        assert!((p - 0.25).abs() < 1e-6);
    }
}

#[test]
fn test_qprep_zero() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 1, dist: dist_id::ZERO }).unwrap();

    let qdist = ctx.qregs[1].as_ref().unwrap();
    assert_eq!(qdist.label, "zero");
    assert_eq!(qdist.domain, vec![0u16]);
    assert_eq!(qdist.probabilities, vec![1.0]);
}

#[test]
fn test_qprep_bell() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 2, dist: dist_id::BELL }).unwrap();

    let qdist = ctx.qregs[2].as_ref().unwrap();
    assert_eq!(qdist.label, "bell");
    assert_eq!(qdist.domain, vec![0u16, 3]);
    assert!((qdist.probabilities[0] - 0.5).abs() < 1e-6);
    assert!((qdist.probabilities[1] - 0.5).abs() < 1e-6);
}

#[test]
fn test_qprep_ghz() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 3, dist: dist_id::GHZ }).unwrap();

    let qdist = ctx.qregs[3].as_ref().unwrap();
    assert_eq!(qdist.label, "ghz");
    assert_eq!(qdist.domain, vec![0u16, 15]);
    assert!((qdist.probabilities[0] - 0.5).abs() < 1e-6);
    assert!((qdist.probabilities[1] - 0.5).abs() < 1e-6);
}

// =============================================================================
// QKernel dispatch tests
// =============================================================================

#[test]
fn test_qkernel_entangle() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    ctx.iregs.set(0, 0);
    ctx.iregs.set(1, 0);

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::ENTANGLE,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    assert!(ctx.qregs[1].is_some());
}

#[test]
fn test_qkernel_fourier() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 0);
    ctx.iregs.set(1, 0);

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::FOURIER,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    let result = ctx.qregs[1].as_ref().unwrap();
    let total: f64 = result.probabilities.iter().sum();
    assert!((total - 1.0).abs() < 1e-6, "Fourier output should be normalized");

    // QFT on uniform concentrates on state 0
    assert!(
        result.probabilities[0] > 0.9,
        "QFT of uniform should concentrate on state 0"
    );
}

#[test]
fn test_qkernel_diffuse() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 0);
    ctx.iregs.set(1, 0);

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::DIFFUSE,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    let result = ctx.qregs[1].as_ref().unwrap();
    let total: f64 = result.probabilities.iter().sum();
    assert!((total - 1.0).abs() < 1e-6, "Diffuse output should be normalized");

    // Diffusion on uniform stays uniform
    for &p in &result.probabilities {
        assert!((p - 0.25).abs() < 1e-6, "Diffuse on uniform should stay uniform");
    }
}

#[test]
fn test_qkernel_grover_iter() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();

    // Set target state in integer register R0
    ctx.iregs.set(0, 2); // target state = 2
    ctx.iregs.set(1, 0);

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::GROVER_ITER,
        ctx0: 0,  // reads R0 = 2 as target
        ctx1: 1,
    }).unwrap();

    let result = ctx.qregs[1].as_ref().unwrap();
    let total: f64 = result.probabilities.iter().sum();
    assert!((total - 1.0).abs() < 1e-6, "Grover output should be normalized");

    // Target state (index 2, state value 2) should have higher probability
    let target_prob = result.probabilities[2];
    let other_prob = result.probabilities[0];
    assert!(
        target_prob > other_prob,
        "Grover should amplify target. target_p={}, other_p={}",
        target_prob, other_prob
    );
}

#[test]
fn test_qkernel_updates_psw_with_real_metrics() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    ctx.iregs.set(0, 0);
    ctx.iregs.set(1, 0);

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: kernel_id::INIT,
        ctx0: 0,
        ctx1: 1,
    }).unwrap();

    // After applying init kernel (uniform output), superposition should be active
    assert!(ctx.psw.qf, "Quantum active flag should be set");
    assert!(ctx.psw.sf, "Superposition flag should be set for uniform distribution");
    assert!(ctx.psw.ef, "Entanglement flag should be set for uniform distribution");
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
    assert!(matches!(ctx.hregs.get(0), HybridValue::Dist(_)));
}

#[test]
fn test_qobserve_sets_psw_flags() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM }).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0 }).unwrap();

    assert!(ctx.psw.df);
    assert!(ctx.psw.cf);
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
// Error cases (Phase 4: now return Err instead of panicking)
// ===========================================================================

#[test]
fn test_qkernel_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 0);
    ctx.iregs.set(1, 0);

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
    ctx.iregs.set(0, 0);
    ctx.iregs.set(1, 0);

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
}
