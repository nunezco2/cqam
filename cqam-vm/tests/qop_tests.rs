//! Tests for quantum operation handlers: QPREP, QKERNEL, QOBSERVE,
//! QLOAD, and QSTORE using the `DensityMatrix` simulation backend.

use cqam_core::complex::C64;
use cqam_core::instruction::*;
use cqam_core::quantum_backend::QuantumBackend;
use cqam_core::register::HybridValue;
use cqam_sim::backend::SimulationBackend;
use cqam_vm::context::ExecutionContext;
use cqam_vm::qop::execute_qop;

fn test_backend() -> SimulationBackend {
    SimulationBackend::new()
}

// =============================================================================
// QPrep distribution tests
// =============================================================================

#[test]
fn test_qprep_uniform() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    assert!(ctx.qregs[0].is_some());
    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2);
    assert_eq!(backend.dimension(*dm).unwrap(), 4);
    // All diagonal probabilities should be 0.25
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6);
    }
}

#[test]
fn test_qprep_zero() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 1, dist: DistId::Zero }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2);
    // rho[0][0] = 1.0, all others 0
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 1.0).abs() < 1e-10);
    assert!((backend.get_element(*dm, 1, 1).unwrap().0).abs() < 1e-10);
}

#[test]
fn test_qprep_bell() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 2, dist: DistId::Bell }, &mut backend).unwrap();

    let dm = ctx.qregs[2].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2);
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*dm, 0, 3).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*dm, 3, 0).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*dm, 3, 3).unwrap().0 - 0.5).abs() < 1e-10);
}

#[test]
fn test_qprep_ghz() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 3, dist: DistId::Ghz }, &mut backend).unwrap();

    let dm = ctx.qregs[3].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2); // default_qubits=2 but GHZ forces n>=2
    let dim = backend.dimension(*dm).unwrap();
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*dm, 0, dim - 1).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*dm, dim - 1, 0).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*dm, dim - 1, dim - 1).unwrap().0 - 0.5).abs() < 1e-10);
}

// =============================================================================
// QKernel dispatch tests
// =============================================================================

#[test]
fn test_qkernel_entangle() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: KernelId::Entangle,
        ctx0: 0,
        ctx1: 1,
    }, &mut backend).unwrap();

    assert!(ctx.qregs[1].is_some());
    let dm = ctx.qregs[1].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2);
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");
}

#[test]
fn test_qkernel_fourier() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: KernelId::Fourier,
        ctx0: 0,
        ctx1: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
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
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: KernelId::Diffuse,
        ctx0: 0,
        ctx1: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
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
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    // Set target state in integer register R0
    ctx.iregs.set(0, 3).unwrap(); // target state = 3
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: KernelId::GroverIter,
        ctx0: 0,  // reads R0 = 3 as target
        ctx1: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();

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
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: KernelId::Init,
        ctx0: 0,
        ctx1: 1,
    }, &mut backend).unwrap();

    // After applying init kernel (uniform output), quantum flags should be set
    assert!(ctx.psw.qf, "Quantum active flag should be set");
    // Init kernel produces a uniform superposition over all basis states.
    assert!(ctx.psw.sf, "sf should be true for uniform superposition state");
    assert!(!ctx.psw.ef, "ef should be false for single-register init");
}

// =============================================================================
// QObserve tests
// =============================================================================

#[test]
fn test_qobserve_destructive() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    assert!(ctx.qregs[0].is_some());

    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 }, &mut backend).unwrap();

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
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 }, &mut backend).unwrap();

    assert!(ctx.psw.df);
    assert!(ctx.psw.cf);
}

#[test]
fn test_qobserve_zero_state_single_entry() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 }, &mut backend).unwrap();

    // Zero state has only |0> with p=1.0, rest are near-zero and filtered out
    if let HybridValue::Dist(d) = ctx.hregs.get(0).unwrap() {
        assert_eq!(d.len(), 1, "Zero-state distribution should have exactly 1 entry (others filtered)");
        assert_eq!(d[0].0, 0u32, "Only entry should be state 0");
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
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    // QSTORE: teleportation semantics -- consumes source register
    execute_qop(&mut ctx, &Instruction::QStore { src_q: 0, addr: 10 }, &mut backend).unwrap();
    assert!(ctx.qmem.is_occupied(10));
    assert!(ctx.qregs[0].is_none(), "QSTORE should consume the source register (teleportation)");

    // QLOAD: teleportation semantics -- consumes QMEM slot
    execute_qop(&mut ctx, &Instruction::QLoad { dst_q: 2, addr: 10 }, &mut backend).unwrap();
    assert!(ctx.qregs[2].is_some());
    assert!(!ctx.qmem.is_occupied(10), "QLOAD should consume the QMEM slot (teleportation)");
}

// ===========================================================================
// Error cases
// ===========================================================================

#[test]
fn test_qkernel_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1, src: 0, kernel: KernelId::Entangle, ctx0: 0, ctx1: 1,
    }, &mut backend);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Uninitialized register"));
}

#[test]
fn test_qobserve_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    let result = execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 }, &mut backend);
    assert!(result.is_err());
}

#[test]
fn test_qload_from_empty_slot_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    let result = execute_qop(&mut ctx, &Instruction::QLoad { dst_q: 0, addr: 0 }, &mut backend);
    assert!(result.is_err());
}

#[test]
fn test_qstore_from_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    let result = execute_qop(&mut ctx, &Instruction::QStore { src_q: 0, addr: 0 }, &mut backend);
    assert!(result.is_err());
}

#[test]
fn test_unknown_kernel_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    // With type-safe enums, invalid kernel IDs are caught at TryFrom time
    let result = KernelId::try_from(99u8);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("KernelId"));
}

#[test]
fn test_unknown_distribution_returns_error() {
    // With type-safe enums, invalid dist IDs are caught at TryFrom time
    let result = DistId::try_from(99u8);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("DistId"), "Expected InvalidId error, got: {}", msg);
}

#[test]
fn test_unknown_distribution_boundary_values() {
    // DistId::Ghz (3) is the last valid ID; 4 should fail
    assert!(DistId::try_from(4u8).is_err());

    // Max u8 value
    assert!(DistId::try_from(255u8).is_err());
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
HREDUCE MODEV, H0, R0
HREDUCE MEANT, H0, F0
HALT
"#;

    let program = parse_program(source).expect("Failed to parse bell_state program").instructions;
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    let mut backend = test_backend();

    run_program(&mut ctx, &mut fm, &mut backend).expect("bell_state program failed");

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

    // Measurement flags: DF is sticky, CF is transient (cleared by HREDUCE)
    assert!(ctx.psw.df, "Decoherence flag should be set after QObserve (sticky)");
    assert!(!ctx.psw.cf, "Collapse flag should be cleared after HREDUCE (transient)");
}

// =============================================================================
// QOBSERVE full-distribution tests
// =============================================================================

#[test]
fn test_qobserve_preserves_full_distribution() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // QPREP with UNIFORM distribution: 2 qubits = 4 basis states, each p=0.25
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 }, &mut backend).unwrap();

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
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    assert!(ctx.qregs[0].is_some(), "Q[0] should be Some after QPREP");

    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 }, &mut backend).unwrap();
    assert!(ctx.qregs[0].is_none(), "Q[0] should be None after QOBSERVE (destructive)");
}

// =============================================================================
// QOBSERVE on GHZ state (entangled distribution shape)
// =============================================================================

#[test]
fn test_qobserve_ghz_state_distribution_shape() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // GHZ state with default 2 qubits: (|00> + |11>)/sqrt(2)
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Ghz }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QObserve { dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 }, &mut backend).unwrap();

    // Q[0] should be consumed
    assert!(ctx.qregs[0].is_none(), "Q[0] should be consumed after QOBSERVE");

    if let HybridValue::Dist(pairs) = ctx.hregs.get(0).unwrap() {
        // GHZ with 2 qubits has only states 0 (|00>) and 3 (|11>)
        assert_eq!(pairs.len(), 2, "GHZ 2-qubit should have exactly 2 entries, got {}", pairs.len());
        let total: f64 = pairs.iter().map(|(_, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-10, "Total probability should be 1.0");

        // Verify the states are 0 and 3 (or dim-1)
        let states: Vec<u32> = pairs.iter().map(|(s, _)| *s).collect();
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
// QOBSERVE mode dispatch tests
// =============================================================================

#[test]
fn test_qobserve_mode_prob() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Prepare zero state: |0> with probability 1.0
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // Set ctx0 = 0 (query probability of basis state 0)
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: ObserveMode::Prob, ctx0: 0, ctx1: 0,
    }, &mut backend).unwrap();

    // Should be destructive
    assert!(ctx.qregs[0].is_none(), "Q[0] should be consumed after QOBSERVE/PROB");

    // H[0] should hold Float(1.0) -- probability of |0> in zero state
    if let HybridValue::Float(p) = ctx.hregs.get(0).unwrap() {
        assert!((p - 1.0).abs() < 1e-10, "p(|0>) in zero state should be 1.0, got {}", p);
    } else {
        panic!("Expected HybridValue::Float after QOBSERVE/PROB, got {:?}", ctx.hregs.get(0));
    }
}

// (test_qobserve_mode_amp removed: AMP mode was removed from the ISA.)

// (test_qobserve_mode_amp_out_of_range removed: AMP mode was removed from the ISA.)

#[test]
fn test_qobserve_mode_prob_out_of_range() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // 2 qubits -> dimension 4; index 4 is out of range
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    ctx.iregs.set(0, 4).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: ObserveMode::Prob, ctx0: 0, ctx1: 0,
    }, &mut backend);
    assert!(result.is_err(), "QOBSERVE/PROB with out-of-range index should error");
}

// (test_qobserve_mode_amp_out_of_range removed: AMP mode was removed from the ISA.)

// =============================================================================
// End-to-end pipeline tests (QPREP -> QOBSERVE -> HREDUCE)
// =============================================================================

// (test_e2e_observe_amp_then_conj_z removed: AMP mode was removed from the ISA.)

/// QPREP -> QOBSERVE(PROB) -> HREDUCE(ROUND) -> verify R register.
///
/// Pipeline: prepare a uniform state, extract probability of state 2,
/// then round to integer.
#[test]
fn test_e2e_observe_prob_then_round() {
    use cqam_vm::fork::ForkManager;
    use cqam_vm::hybrid::execute_hybrid;

    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Uniform state: p(|2>) = 0.25
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    // ctx0 = R0 = 2 (query probability of basis state 2)
    ctx.iregs.set(0, 2).unwrap();

    // QOBSERVE in PROB mode: H[0] = Float(0.25)
    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: ObserveMode::Prob, ctx0: 0, ctx1: 0,
    }, &mut backend).unwrap();

    // Q[0] consumed
    assert!(ctx.qregs[0].is_none());

    if let HybridValue::Float(p) = ctx.hregs.get(0).unwrap() {
        assert!((p - 0.25).abs() < 1e-10, "prob should be 0.25, got {}", p);
    } else {
        panic!("Expected HybridValue::Float after QOBSERVE/PROB, got {:?}", ctx.hregs.get(0));
    }

    // HREDUCE with ROUND: R[3] = round(0.25) = 0
    let mut fm = ForkManager::new();
    let mut backend = test_backend();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 3, func: ReduceFn::Round },
        &mut fm, &mut backend).unwrap();

    assert_eq!(ctx.iregs.get(3).unwrap(), 0, "round(0.25) should be 0");
}

// =============================================================================
// QKERNELF tests
// =============================================================================

#[test]
fn test_qkernelf_rotate() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Prepare uniform state
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    // Load theta = PI/4 into F[0], F[1] = 0.0
    ctx.fregs.set(0, std::f64::consts::FRAC_PI_4).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1,
        src: 0,
        kernel: KernelId::Rotate,
        fctx0: 0,
        fctx1: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");

    // Diagonal probabilities should be preserved (diagonal unitary)
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10,
            "Rotate should preserve diagonal probs, got {}", p);
    }
}

#[test]
fn test_qkernelf_rotate_zero_angle() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    // theta = 0 -> identity
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    // Get input state for comparison
    let input_probs = backend.diagonal_probabilities(*ctx.qregs[0].as_ref().unwrap()).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1,
        src: 0,
        kernel: KernelId::Rotate,
        fctx0: 0,
        fctx1: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let output_probs = backend.diagonal_probabilities(*dm).unwrap();
    for (i, (&pi, &po)) in input_probs.iter().zip(output_probs.iter()).enumerate() {
        assert!((pi - po).abs() < 1e-10,
            "Rotate(0) should be identity: p[{}] in={}, out={}", i, pi, po);
    }
}

#[test]
fn test_qkernelz_phase_shift() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    // Load complex amplitude (1.0, 0.5) into Z[0], Z[1] = (0,0)
    ctx.zregs.set(0, (1.0, 0.5)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelZ {
        dst: 1,
        src: 0,
        kernel: KernelId::PhaseShift,
        zctx0: 0,
        zctx1: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");

    // Diagonal probabilities should be preserved (diagonal unitary)
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10,
            "PhaseShift should preserve diagonal probs, got {}", p);
    }

    // Purity should be preserved
    assert!((backend.purity(*dm).unwrap() - 1.0).abs() < 1e-10,
        "PhaseShift should preserve purity, got {}", backend.purity(*dm).unwrap());
}

#[test]
fn test_qkernelf_existing_kernels() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Prepare uniform state
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    // Use QKERNELF with Init kernel (should work, ignoring float params)
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1,
        src: 0,
        kernel: KernelId::Init,
        fctx0: 0,
        fctx1: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");

    // Init on any state produces uniform superposition
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10,
            "Init kernel via QKERNELF should produce uniform, got {}", p);
    }

    // Also test Fourier kernel via QKERNELF
    execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 2,
        src: 1,
        kernel: KernelId::Fourier,
        fctx0: 0,
        fctx1: 1,
    }, &mut backend).unwrap();

    let dm2 = ctx.qregs[2].as_ref().unwrap();
    assert!(backend.purity(*dm2).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");
    // QFT on uniform concentrates on state 0
    let probs2 = backend.diagonal_probabilities(*dm2).unwrap();
    assert!(probs2[0] > 0.99,
        "QFT of uniform via QKERNELF should concentrate on state 0, got p[0]={}", probs2[0]);
}

// =============================================================================
// QKERNELF error cases
// =============================================================================

#[test]
fn test_qkernelf_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernelF {
        dst: 1, src: 0, kernel: KernelId::Rotate, fctx0: 0, fctx1: 1,
    }, &mut backend);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Uninitialized register"),
        "Expected UninitializedRegister error, got: {}", msg);
}

#[test]
fn test_qkernelf_unknown_kernel_returns_error() {
    // With type-safe enums, invalid kernel IDs are caught at TryFrom time
    let result = KernelId::try_from(99u8);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("KernelId"),
        "Expected InvalidId error, got: {}", msg);
}

// =============================================================================
// QKERNELZ error cases
// =============================================================================

#[test]
fn test_qkernelz_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.zregs.set(0, (0.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QKernelZ {
        dst: 1, src: 0, kernel: KernelId::PhaseShift, zctx0: 0, zctx1: 1,
    }, &mut backend);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Uninitialized register"),
        "Expected UninitializedRegister error, got: {}", msg);
}

#[test]
fn test_qkernelz_unknown_kernel_returns_error() {
    // With type-safe enums, invalid kernel IDs are caught at TryFrom time
    let result = KernelId::try_from(99u8);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("KernelId"),
        "Expected InvalidId error, got: {}", msg);
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
QPREP Q0, 1
# Load theta = 0 into F0 (identity rotation)
FLDI F0, 0
FLDI F1, 0
# Apply QKERNELF with ROTATE kernel (kernel_id=5)
QKERNELF DROT, Q1, Q0, F0, F1
# Observe Q1 -> H0 (full distribution)
QOBSERVE H0, Q1
# Reduce to mean
HREDUCE MEANT, H0, F2
HALT
"#;

    let program = parse_program(source).expect("Failed to parse QKERNELF pipeline").instructions;
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    let mut backend = test_backend();

    run_program(&mut ctx, &mut fm, &mut backend).expect("QKERNELF pipeline failed");

    assert!(ctx.psw.trap_halt, "Program should have halted");

    // Q1 was consumed by QOBSERVE
    assert!(ctx.qregs[1].is_none(), "Q1 should be consumed after QOBSERVE");

    // MEAN of uniform(0,1,2,3) = 1.5
    let f2 = ctx.fregs.get(2).unwrap();
    assert!((f2 - 1.5).abs() < 1e-10,
        "Mean of uniform after identity Rotate should be 1.5, got F2={}", f2);
}

// =============================================================================
// End-to-end: QPREP -> QKERNELZ(PHASE_SHIFT) -> QOBSERVE -> verify
// =============================================================================

#[test]
fn test_e2e_qkernelz_phase_shift_observe() {
    use cqam_core::parser::parse_program;
    use cqam_vm::fork::ForkManager;
    use cqam_vm::executor::run_program;

    // Pipeline: prepare uniform state, apply PhaseShift with zero amplitude
    // (which is identity), then observe the distribution.
    // Diagonal probabilities should be 0.25 each.
    let source = r#"
# Prepare uniform 2-qubit state
QPREP Q0, 1
# Load amplitude = (0, 0) into Z0 (zero amplitude -> identity)
ZLDI Z0, 0, 0
ZLDI Z1, 0, 0
# Apply QKERNELZ with PHASE_SHIFT kernel (kernel_id=6)
QKERNELZ PHSH, Q1, Q0, Z0, Z1
# Destructive observe Q1 -> H0
QOBSERVE H0, Q1
# Reduce to mean
HREDUCE MEANT, H0, F0
HALT
"#;

    let program = parse_program(source).expect("Failed to parse QKERNELZ pipeline").instructions;
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    let mut backend = test_backend();

    run_program(&mut ctx, &mut fm, &mut backend).expect("QKERNELZ pipeline failed");

    assert!(ctx.psw.trap_halt, "Program should have halted");

    // Q1 should be consumed (QOBSERVE is destructive)
    assert!(ctx.qregs[1].is_none(), "Q1 should be consumed after QOBSERVE");

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
        panic!("Expected HybridValue::Dist after QOBSERVE");
    }
}

// =============================================================================
// QKERNELZ with non-zero complex amplitude: verify diagonal preservation
// =============================================================================

#[test]
fn test_qkernelz_phase_shift_nonzero_preserves_diag() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    // amplitude = (0.0, 2.0) -> |z| = 2.0, purely imaginary
    ctx.zregs.set(0, (0.0, 2.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    execute_qop(&mut ctx, &Instruction::QKernelZ {
        dst: 1,
        src: 0,
        kernel: KernelId::PhaseShift,
        zctx0: 0,
        zctx1: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");

    // Diagonal probabilities should be preserved (diagonal unitary)
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10,
            "PhaseShift with purely imaginary amplitude should preserve diagonal probs, got {}", p);
    }

    assert!((backend.purity(*dm).unwrap() - 1.0).abs() < 1e-10,
        "PhaseShift should preserve purity, got {}", backend.purity(*dm).unwrap());
}

// =============================================================================
// QPrepR tests
// =============================================================================

#[test]
fn test_qprepr_uniform() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Set R[0] = 1 (UNIFORM)
    ctx.iregs.set(0, 1).unwrap();

    execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend).unwrap();

    assert!(ctx.qregs[0].is_some());
    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6, "Expected uniform 0.25, got {}", p);
    }
}

#[test]
fn test_qprepr_bell() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Set R[0] = 2 (BELL)
    ctx.iregs.set(0, 2).unwrap();

    execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2);
    // Bell state: |00> and |11> with equal probability
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*dm, 3, 3).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*dm, 1, 1).unwrap().0).abs() < 1e-10);
    assert!((backend.get_element(*dm, 2, 2).unwrap().0).abs() < 1e-10);
}

#[test]
fn test_qprepr_invalid_dist() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Set R[0] = 99 (invalid)
    ctx.iregs.set(0, 99).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("distribution") || err_msg.contains("99"),
        "Error should mention unknown distribution, got: {}", err_msg);
}

// =============================================================================
// QEncode tests
// =============================================================================

#[test]
fn test_qencode_from_int() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Load R[0..3] with [1, 1, 1, 1]
    for i in 0..4u8 {
        ctx.iregs.set(i, 1).unwrap();
    }

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 4, file_sel: FileSel::RFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2);
    // All equal amplitudes -> uniform-ish state
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6, "Expected ~0.25, got {}", p);
    }
}

#[test]
fn test_qencode_from_float() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Load F[0..3] with [0.5, 0.5, 0.5, 0.5]
    for i in 0..4u8 {
        ctx.fregs.set(i, 0.5).unwrap();
    }

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 4, file_sel: FileSel::FFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2);
    // All equal amplitudes -> uniform state after normalization
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-6, "Expected ~0.25, got {}", p);
    }
}

#[test]
fn test_qencode_from_complex() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Z[0] = (1, 0), Z[1] = (0, 0) -> |0> state after normalization
    ctx.zregs.set(0, (1.0, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: FileSel::ZFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 1);
    // |0> state: prob(0) = 1, prob(1) = 0
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 1.0).abs() < 1e-10);
    assert!((backend.get_element(*dm, 1, 1).unwrap().0).abs() < 1e-10);
}

#[test]
fn test_qencode_non_power_of_2() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    for i in 0..3u8 {
        ctx.fregs.set(i, 1.0).unwrap();
    }

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 3, file_sel: FileSel::FFile,
    }, &mut backend);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("power of 2"), "Error should mention power of 2, got: {}", err_msg);
}

#[test]
fn test_qencode_normalizes() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Load unnormalized values: F[0]=3.0, F[1]=4.0
    ctx.fregs.set(0, 3.0).unwrap();
    ctx.fregs.set(1, 4.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: FileSel::FFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    // Trace should be 1 (normalized)
    let trace = backend.get_element(*dm, 0, 0).unwrap().0 + backend.get_element(*dm, 1, 1).unwrap().0;
    assert!((trace - 1.0).abs() < 1e-10,
        "Trace should be 1.0 after normalization, got {}", trace);
    // Specific probabilities: |3/5|^2 = 9/25, |4/5|^2 = 16/25
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 9.0/25.0).abs() < 1e-10);
    assert!((backend.get_element(*dm, 1, 1).unwrap().0 - 16.0/25.0).abs() < 1e-10);
}

#[test]
fn test_qencode_count_zero() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 0, file_sel: FileSel::RFile,
    }, &mut backend);
    assert!(result.is_err());
}

#[test]
fn test_qencode_zero_norm() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // All-zero statevector
    ctx.fregs.set(0, 0.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: FileSel::FFile,
    }, &mut backend);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("zero norm"), "Error should mention zero norm, got: {}", err_msg);
}

// =============================================================================
// QPrepR equivalence with QPrep
// =============================================================================

/// QPrepR with each dist_id must produce the same density matrix as QPrep
/// with the same dist literal. This validates that the register-indirect path
/// dispatches through the same DensityMatrix constructors.
#[test]
fn test_qprepr_matches_qprep_uniform() {
    let mut ctx_lit = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx_lit, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    let mut ctx_reg = ExecutionContext::new(vec![]);
    ctx_reg.iregs.set(0, DistId::Uniform as i64).unwrap();
    execute_qop(&mut ctx_reg, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend).unwrap();

    let dm_lit = ctx_lit.qregs[0].as_ref().unwrap();
    let dm_reg = ctx_reg.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm_lit).unwrap(), backend.num_qubits(*dm_reg).unwrap());
    assert_eq!(backend.dimension(*dm_lit).unwrap(), backend.dimension(*dm_reg).unwrap());
    for i in 0..backend.dimension(*dm_lit).unwrap() {
        for j in 0..backend.dimension(*dm_lit).unwrap() {
            let C64(r1, i1) = backend.get_element(*dm_lit, i, j).unwrap();
            let C64(r2, i2) = backend.get_element(*dm_reg, i, j).unwrap();
            assert!((r1 - r2).abs() < 1e-15 && (i1 - i2).abs() < 1e-15,
                "UNIFORM mismatch at ({},{}): ({},{}) vs ({},{})", i, j, r1, i1, r2, i2);
        }
    }
}

#[test]
fn test_qprepr_matches_qprep_zero() {
    let mut ctx_lit = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx_lit, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    let mut ctx_reg = ExecutionContext::new(vec![]);
    ctx_reg.iregs.set(0, DistId::Zero as i64).unwrap();
    execute_qop(&mut ctx_reg, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend).unwrap();

    let dm_lit = ctx_lit.qregs[0].as_ref().unwrap();
    let dm_reg = ctx_reg.qregs[0].as_ref().unwrap();
    for i in 0..backend.dimension(*dm_lit).unwrap() {
        for j in 0..backend.dimension(*dm_lit).unwrap() {
            let C64(r1, i1) = backend.get_element(*dm_lit, i, j).unwrap();
            let C64(r2, i2) = backend.get_element(*dm_reg, i, j).unwrap();
            assert!((r1 - r2).abs() < 1e-15 && (i1 - i2).abs() < 1e-15,
                "ZERO mismatch at ({},{})", i, j);
        }
    }
}

#[test]
fn test_qprepr_matches_qprep_bell() {
    let mut ctx_lit = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx_lit, &Instruction::QPrep { dst: 0, dist: DistId::Bell }, &mut backend).unwrap();

    let mut ctx_reg = ExecutionContext::new(vec![]);
    ctx_reg.iregs.set(0, DistId::Bell as i64).unwrap();
    execute_qop(&mut ctx_reg, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend).unwrap();

    let dm_lit = ctx_lit.qregs[0].as_ref().unwrap();
    let dm_reg = ctx_reg.qregs[0].as_ref().unwrap();
    for i in 0..backend.dimension(*dm_lit).unwrap() {
        for j in 0..backend.dimension(*dm_lit).unwrap() {
            let C64(r1, i1) = backend.get_element(*dm_lit, i, j).unwrap();
            let C64(r2, i2) = backend.get_element(*dm_reg, i, j).unwrap();
            assert!((r1 - r2).abs() < 1e-15 && (i1 - i2).abs() < 1e-15,
                "BELL mismatch at ({},{})", i, j);
        }
    }
}

#[test]
fn test_qprepr_matches_qprep_ghz() {
    let mut ctx_lit = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx_lit, &Instruction::QPrep { dst: 0, dist: DistId::Ghz }, &mut backend).unwrap();

    let mut ctx_reg = ExecutionContext::new(vec![]);
    ctx_reg.iregs.set(0, DistId::Ghz as i64).unwrap();
    execute_qop(&mut ctx_reg, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend).unwrap();

    let dm_lit = ctx_lit.qregs[0].as_ref().unwrap();
    let dm_reg = ctx_reg.qregs[0].as_ref().unwrap();
    for i in 0..backend.dimension(*dm_lit).unwrap() {
        for j in 0..backend.dimension(*dm_lit).unwrap() {
            let C64(r1, i1) = backend.get_element(*dm_lit, i, j).unwrap();
            let C64(r2, i2) = backend.get_element(*dm_reg, i, j).unwrap();
            assert!((r1 - r2).abs() < 1e-15 && (i1 - i2).abs() < 1e-15,
                "GHZ mismatch at ({},{})", i, j);
        }
    }
}

// =============================================================================
// QPrepR edge cases
// =============================================================================

/// Negative register value wraps to large u8 -> UnknownDistribution error.
#[test]
fn test_qprepr_negative_dist_id_wraps() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // -1i64 as u8 = 255
    ctx.iregs.set(0, -1).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("distribution") || err_msg.contains("255"),
        "Negative dist wraps to 255, error should reflect this, got: {}", err_msg);
}

/// Large positive value (256) wraps to 0 -> ZERO (same as dist_id=0).
#[test]
fn test_qprepr_large_value_wraps_modulo_256() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // 256 as u8 = 0 = ZERO
    ctx.iregs.set(0, 256).unwrap();

    execute_qop(&mut ctx, &Instruction::QPrepR { dst: 0, dist_reg: 0 }, &mut backend).unwrap();
    let dm = ctx.qregs[0].as_ref().unwrap();
    // Should produce zero state: P(0) = 1.0, all others = 0.0
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-6,
        "256 wraps to 0 (ZERO), expected P(0)=1.0, got {}", probs[0]);
    for &p in probs.iter().skip(1) {
        assert!(p.abs() < 1e-6, "256 wraps to 0 (ZERO), expected P(k)=0 for k>0, got {}", p);
    }
}

// =============================================================================
// QEncode count=1 (single amplitude)
// =============================================================================

/// count=1 produces a 0-qubit 1x1 density matrix. This is a degenerate case
/// where the statevector has a single amplitude, normalized to 1.
#[test]
fn test_qencode_count_1_single_amplitude() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.fregs.set(0, 5.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 1, file_sel: FileSel::FFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    // 1 amplitude -> dimension 1, 0 qubits
    assert_eq!(backend.dimension(*dm).unwrap(), 1);
    // The single entry should be (1.0, 0.0) after normalization
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 1.0).abs() < 1e-10,
        "Single amplitude DM should have rho[0][0] = 1.0, got {}", backend.get_element(*dm, 0, 0).unwrap().0);
}

// =============================================================================
// QEncode with negative R-file integers
// =============================================================================

/// Negative integers from R-file are cast to f64, producing negative amplitudes.
/// After normalization, the resulting state should have correct probabilities.
#[test]
fn test_qencode_r_file_negative_values() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // [-3, 4] -> amplitudes (-3, 0) and (4, 0) -> norm = 5
    // probs: 9/25 and 16/25
    ctx.iregs.set(0, -3).unwrap();
    ctx.iregs.set(1, 4).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: FileSel::RFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 1);
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 9.0 / 25.0).abs() < 1e-10,
        "Expected prob 9/25, got {}", backend.get_element(*dm, 0, 0).unwrap().0);
    assert!((backend.get_element(*dm, 1, 1).unwrap().0 - 16.0 / 25.0).abs() < 1e-10,
        "Expected prob 16/25, got {}", backend.get_element(*dm, 1, 1).unwrap().0);
    // Off-diagonal should reflect negative sign: rho[0][1] = (-3/5)(4/5) = -12/25
    assert!((backend.get_element(*dm, 0, 1).unwrap().0 - (-12.0 / 25.0)).abs() < 1e-10,
        "Expected off-diag real = -12/25, got {}", backend.get_element(*dm, 0, 1).unwrap().0);
}

// =============================================================================
// QEncode from Z-file with complex amplitudes
// =============================================================================

/// Complex amplitudes from Z-file producing a known state.
/// Z[0] = (1/sqrt(2), 0), Z[1] = (0, 1/sqrt(2)) -> |psi> = (1+i|1>)/sqrt(2)
/// This is already normalized: |1/sqrt(2)|^2 + |i/sqrt(2)|^2 = 0.5 + 0.5 = 1.
#[test]
fn test_qencode_z_file_complex_amplitudes() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    let s = std::f64::consts::FRAC_1_SQRT_2;
    ctx.zregs.set(0, (s, 0.0)).unwrap();
    ctx.zregs.set(1, (0.0, s)).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: FileSel::ZFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 1);
    // Both diagonal entries should be 0.5
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 0.5).abs() < 1e-10,
        "Expected prob 0.5 for |0>, got {}", backend.get_element(*dm, 0, 0).unwrap().0);
    assert!((backend.get_element(*dm, 1, 1).unwrap().0 - 0.5).abs() < 1e-10,
        "Expected prob 0.5 for |1>, got {}", backend.get_element(*dm, 1, 1).unwrap().0);
    // Off-diagonal: rho[0][1] = (1/sqrt(2))(0-i/sqrt(2))* = (1/sqrt(2))(-i/sqrt(2))* = ... wait
    // rho[0][1] = psi[0] * conj(psi[1]) = (1/sqrt(2))*(0 - i/sqrt(2)) = (0, -1/2)
    assert!((backend.get_element(*dm, 0, 1).unwrap().0).abs() < 1e-10,
        "Expected off-diag real = 0, got {}", backend.get_element(*dm, 0, 1).unwrap().0);
    assert!((backend.get_element(*dm, 0, 1).unwrap().1 - (-0.5)).abs() < 1e-10,
        "Expected off-diag imag = -0.5, got {}", backend.get_element(*dm, 0, 1).unwrap().1);
}

// =============================================================================
// QEncode count=8 (max useful with 16 regs)
// =============================================================================

/// count=8 with 3-qubit state from float registers.
#[test]
fn test_qencode_count_8_from_floats() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Set up |0> state: first amplitude = 1.0, rest = 0.0
    ctx.fregs.set(0, 1.0).unwrap();
    for i in 1..8u8 {
        ctx.fregs.set(i, 0.0).unwrap();
    }

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 8, file_sel: FileSel::FFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 3);
    assert_eq!(backend.dimension(*dm).unwrap(), 8);
    // Should be |000> state
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 1.0).abs() < 1e-10);
    for i in 1..8 {
        assert!((backend.get_element(*dm, i, i).unwrap().0).abs() < 1e-10,
            "Non-zero diagonal at ({},{}): {}", i, i, backend.get_element(*dm, i, i).unwrap().0);
    }
}

// =============================================================================
// QEncode src_base near register limit
// =============================================================================

/// src_base=14, count=2 should work (registers 14 and 15).
#[test]
fn test_qencode_src_base_near_limit() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.fregs.set(14, 1.0).unwrap();
    ctx.fregs.set(15, 0.0).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 14, count: 2, file_sel: FileSel::FFile,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 1);
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 1.0).abs() < 1e-10);
}

/// src_base=15, count=2 should fail: register 16 does not exist.
#[test]
fn test_qencode_src_base_overflow() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.fregs.set(15, 1.0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 15, count: 2, file_sel: FileSel::FFile,
    }, &mut backend);
    assert!(result.is_err(),
        "src_base=15 + count=2 accesses F[16], should produce RegisterOutOfBounds");
}

// =============================================================================
// QEncode invalid file_sel at runtime
// =============================================================================

/// file_sel=3 should be caught by the parser, but if it reaches runtime
/// the match arm in qop.rs should produce an error.
#[test]
fn test_qencode_invalid_file_sel_runtime() {
    // With type-safe enums, invalid file_sel values are caught at TryFrom time
    assert!(FileSel::try_from(3u8).is_err(), "file_sel=3 should produce error");
}

// =============================================================================
// End-to-end pipeline: QEncode -> QKERNEL -> QOBSERVE -> HREDUCE
// =============================================================================

/// Full pipeline: encode a known state from F-file, apply init kernel,
/// observe, and reduce to verify the chain works end-to-end.
#[test]
fn test_qencode_observe_reduce_pipeline() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Step 1: Load F[0..3] = [1, 0, 0, 0] -> |00> state
    ctx.fregs.set(0, 1.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();
    ctx.fregs.set(2, 0.0).unwrap();
    ctx.fregs.set(3, 0.0).unwrap();

    // Step 2: QEncode Q0 from F-file
    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 4, file_sel: FileSel::FFile,
    }, &mut backend).unwrap();

    // Verify: Q0 should be |00> state
    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 2);
    assert!((backend.get_element(*dm, 0, 0).unwrap().0 - 1.0).abs() < 1e-10);

    // Step 3: Apply Init kernel Q1 = kernel(Q0)
    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1, src: 0, kernel: KernelId::Init, ctx0: 0, ctx1: 0,
    }, &mut backend).unwrap();
    assert!(ctx.qregs[1].is_some());

    // Step 4: Observe Q1 -> H0
    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 1, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0,
    }, &mut backend).unwrap();

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
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b11).unwrap(); // mask both qubits

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for (i, &p) in probs.iter().enumerate() {
        assert!((p - 0.25).abs() < 1e-8, "P({}) should be 0.25, got {}", i, p);
    }
    // Hadamard on all qubits produces uniform superposition.
    assert!(ctx.psw.sf, "PSW sf should be true for a superposition state");
}

#[test]
fn test_qhadm_single_qubit() {
    // QPREP |00> -> mask=0b01 (qubit 0 only) -> QHADM
    // P(00)=0.5, P(01)=0, P(10)=0.5, P(11)=0
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 0.5).abs() < 1e-8, "P(00) should be 0.5, got {}", probs[0]);
    assert!(probs[1].abs() < 1e-8, "P(01) should be 0, got {}", probs[1]);
    assert!((probs[2] - 0.5).abs() < 1e-8, "P(10) should be 0.5, got {}", probs[2]);
    assert!(probs[3].abs() < 1e-8, "P(11) should be 0, got {}", probs[3]);
}

#[test]
fn test_qhadm_empty_mask() {
    // mask=0 -> no-op, state unchanged
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 with empty mask");
}

#[test]
fn test_qhadm_involution() {
    // H*H = I: apply twice, state should return to original
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b11).unwrap();

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 after H*H");
}

#[test]
fn test_qhadm_different_src_dst() {
    // src != dst: src unchanged, dst gets result
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 1, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Q0 unchanged
    let dm0 = ctx.qregs[0].as_ref().unwrap();
    let probs0 = backend.diagonal_probabilities(*dm0).unwrap();
    assert!((probs0[0] - 1.0).abs() < 1e-8, "Q0 should remain |00>");

    // Q1 has Hadamard on qubit 0
    let dm1 = ctx.qregs[1].as_ref().unwrap();
    let probs1 = backend.diagonal_probabilities(*dm1).unwrap();
    assert!((probs1[0] - 0.5).abs() < 1e-8, "Q1 P(00) should be 0.5");
    assert!((probs1[2] - 0.5).abs() < 1e-8, "Q1 P(10) should be 0.5");
}

#[test]
fn test_qflip_all_qubits() {
    // QPREP |00> -> mask=0b11 -> QFLIP -> |11>
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b11).unwrap();

    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[3] - 1.0).abs() < 1e-8, "P(11) should be 1.0 after flipping both qubits");
}

#[test]
fn test_qflip_single_qubit() {
    // QPREP |00> -> mask=0b10 -> QFLIP -> |01>
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b10).unwrap();

    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[1] - 1.0).abs() < 1e-8, "P(01) should be 1.0 after flipping qubit 1");
}

#[test]
fn test_qflip_involution() {
    // X*X = I: apply twice, state returns to original
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b11).unwrap();

    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 after X*X");
}

#[test]
fn test_qflip_empty_mask() {
    // mask=0 -> no-op
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 with empty mask");
}

#[test]
fn test_qphase_on_superposition() {
    // Prepare |+> via QHADM, then QPHASE -> |->
    // Diagonal probabilities unchanged but off-diagonal signs flip
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b01).unwrap(); // qubit 0

    // Apply H to get |+>
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Apply Z to get |->
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Diagonal probabilities should still be 50/50
    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 0.5).abs() < 1e-8, "P(00) should be 0.5");
    assert!((probs[2] - 0.5).abs() < 1e-8, "P(10) should be 0.5");

    // Applying H again should give |1> (|-> = H|1>)
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();
    let dm2 = ctx.qregs[0].as_ref().unwrap();
    let probs2 = backend.diagonal_probabilities(*dm2).unwrap();
    assert!((probs2[2] - 1.0).abs() < 1e-8, "After H|-> should get |10>, P(10)=1.0");
}

#[test]
fn test_qphase_on_computational_basis() {
    // Z on |0> -> |0> (unchanged diagonal probabilities)
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-8, "P(00) should be 1.0 after Z on |0>");
}

#[test]
fn test_qphase_involution() {
    // Z*Z = I
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    // Put in superposition first to make it interesting
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Save state before QPHASE
    let probs_before: Vec<f64> = backend.diagonal_probabilities(*ctx.qregs[0].as_ref().unwrap()).unwrap().to_vec();

    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let probs_after = backend.diagonal_probabilities(*ctx.qregs[0].as_ref().unwrap()).unwrap();
    for i in 0..probs_before.len() {
        assert!((probs_before[i] - probs_after[i]).abs() < 1e-8,
            "Z*Z should be identity, state {} differs", i);
    }
}

#[test]
fn test_hadm_then_flip_then_phase() {
    // Compose all three: QHADM creates superposition, QFLIP flips, QPHASE flips phase
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();

    // H|0> = |+>
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();
    // X|+> = |+> (X and H commute in prob space, X|+> = |+>)
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();
    // Z|+> = |->
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Still 50/50 in computational basis
    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 0.5).abs() < 1e-8, "Should still be 50/50");
    assert!((probs[2] - 0.5).abs() < 1e-8, "Should still be 50/50");
}

#[test]
fn test_masked_gate_on_empty_register_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 0b01).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend);
    assert!(result.is_err(), "QHADM on empty register should error");

    let result = execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend);
    assert!(result.is_err(), "QFLIP on empty register should error");

    let result = execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }, &mut backend);
    assert!(result.is_err(), "QPHASE on empty register should error");
}

#[test]
fn test_mask_bits_beyond_num_qubits_ignored() {
    // Set mask with bits beyond num_qubits (2). Extra bits should be ignored.
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0xFF).unwrap(); // all bits set, but only 2 qubits

    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Should be same as mask=0b11 (uniform distribution)
    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    for (i, &p) in probs.iter().enumerate() {
        assert!((p - 0.25).abs() < 1e-8, "P({}) should be 0.25, got {}", i, p);
    }
}

// =============================================================================
// Additional coverage: masked register-level gate operations
// =============================================================================

/// End-to-end pipeline: ILDI mask -> QPREP -> QHADM -> QOBSERVE -> HREDUCE
#[test]
fn test_end_to_end_masked_hadamard_observe_reduce_pipeline() {
    use cqam_vm::executor::execute_instruction;
    use cqam_vm::fork::ForkManager;

    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    let mut fork_mgr = ForkManager::new();

    // Step 1: ILDI R0, 0b11 (mask for 2 qubits)
    execute_instruction(
        &mut ctx,
        &Instruction::ILdi { dst: 0, imm: 0b11 },
        &mut fork_mgr, &mut backend).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 0b11);

    // Step 2: QPREP Q0 as |00>
    execute_instruction(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: DistId::Zero },
        &mut fork_mgr, &mut backend).unwrap();

    // Step 3: QHADM Q0, Q0, R0 -> uniform superposition
    execute_instruction(
        &mut ctx,
        &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 },
        &mut fork_mgr, &mut backend).unwrap();

    // Step 4: QOBSERVE H0, Q0 -> distribution
    execute_instruction(
        &mut ctx,
        &Instruction::QObserve {
            dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0,
        },
        &mut fork_mgr, &mut backend).unwrap();

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
        &Instruction::HReduce { src: 0, dst: 0, func: ReduceFn::Mean },
        &mut fork_mgr, &mut backend).unwrap();

    // Mean of uniform {0,1,2,3} with equal probs should be 1.5
    let mean_val = ctx.fregs.get(0).unwrap();
    assert!((mean_val - 1.5).abs() < 1e-6,
        "Mean of uniform distribution over {{0,1,2,3}} should be 1.5, got {}", mean_val);
}

/// QFLIP on 3-qubit zero state with selective mask produces expected basis state.
#[test]
fn test_qflip_3_qubit_selective_mask() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.config.default_qubits = 3;

    // QPREP |000>
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 3);
    assert_eq!(backend.dimension(*dm).unwrap(), 8);

    // mask = 0b101 -> flip qubit 0 and qubit 2 -> |101>
    ctx.iregs.set(0, 0b101).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
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
    let mut backend = test_backend();
    ctx.config.default_qubits = 3;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // mask = 0b111 -> flip all 3 qubits
    ctx.iregs.set(0, 0b111).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[7] - 1.0).abs() < 1e-8,
        "P(111) should be 1.0 after flipping all 3 qubits, got {}", probs[7]);
}

/// QPHASE on |1> state: diagonal unchanged (Z|1> = -|1>, but prob = |-1|^2 = 1).
#[test]
fn test_qphase_on_one_state_diagonal_unchanged() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.config.default_qubits = 3;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    // Flip all qubits to get |111>
    ctx.iregs.set(0, 0b111).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let probs_before = backend.diagonal_probabilities(*ctx.qregs[0].as_ref().unwrap()).unwrap();

    // Apply QPHASE to all qubits
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let probs_after = backend.diagonal_probabilities(*ctx.qregs[0].as_ref().unwrap()).unwrap();
    for i in 0..probs_before.len() {
        assert!((probs_before[i] - probs_after[i]).abs() < 1e-8,
            "QPHASE should not change diagonal probabilities on computational basis state |111>, index {}", i);
    }
}

/// Full pipeline: QHADM -> QFLIP -> QPHASE -> QOBSERVE on 3-qubit register.
#[test]
fn test_combined_hadm_flip_phase_observe_3_qubit() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.config.default_qubits = 3;

    // QPREP |000>
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // QHADM on qubit 0 only: mask = 0b001
    ctx.iregs.set(0, 0b001).unwrap();
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // QFLIP on qubit 1 only: mask = 0b010
    ctx.iregs.set(1, 0b010).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 1 }, &mut backend).unwrap();

    // QPHASE on qubit 2 only: mask = 0b100
    ctx.iregs.set(2, 0b100).unwrap();
    execute_qop(&mut ctx, &Instruction::QPhase { dst: 0, src: 0, mask_reg: 2 }, &mut backend).unwrap();

    // QOBSERVE
    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0,
    }, &mut backend).unwrap();

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
    let mut backend = test_backend();
    ctx.config.default_qubits = 3;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // mask = 0xFF has 8 bits set, but only 3 qubits exist
    ctx.iregs.set(0, 0xFF).unwrap();
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
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
    let mut backend = test_backend();
    ctx.config.default_qubits = 3;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // Apply H to qubits 0 and 2 (mask = 0b101)
    ctx.iregs.set(0, 0b101).unwrap();
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Apply H again with same mask -> should return to |000>
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-8,
        "H*H should return to |000>, P(000) = {}", probs[0]);
}

// =============================================================================
// QCNOT tests
// =============================================================================

/// QCNOT on |00> should leave state unchanged.
#[test]
fn test_qcnot_zero_state_unchanged() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // ctrl=qubit 0, tgt=qubit 1
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 1).unwrap();

    execute_qop(&mut ctx, &Instruction::QCnot {
        dst: 1, src: 0, ctrl_qubit_reg: 0, tgt_qubit_reg: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    // |00> -> |00>: P(00) = 1.0
    assert!((probs[0] - 1.0).abs() < 1e-8,
        "CNOT|00> should stay |00>, got P(00)={}", probs[0]);
}

/// QCNOT on |10> -> |11> (ctrl=qubit 0, bit ordering: qubit 0 is MSB).
/// We prepare |10> by starting from |00> and flipping qubit 0.
#[test]
fn test_qcnot_flips_target() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // Flip qubit 0 using QFLIP with mask=0b01
    ctx.iregs.set(0, 0b01).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Now in |01> (qubit 0 flipped). Apply CNOT ctrl=0, tgt=1
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 1).unwrap();

    execute_qop(&mut ctx, &Instruction::QCnot {
        dst: 0, src: 0, ctrl_qubit_reg: 0, tgt_qubit_reg: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    // |01> with CNOT(0,1) should flip tgt -> |11>
    assert!((probs[3] - 1.0).abs() < 1e-8,
        "CNOT on |01> should give |11>, got probs={:?}", probs);
}

/// QCNOT with ctrl == tgt should return an error.
#[test]
fn test_qcnot_same_qubit_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap(); // same qubit

    let result = execute_qop(&mut ctx, &Instruction::QCnot {
        dst: 1, src: 0, ctrl_qubit_reg: 0, tgt_qubit_reg: 1,
    }, &mut backend);
    assert!(result.is_err(), "QCNOT with ctrl==tgt should fail");
}

/// QCNOT on uninitialized register should return an error.
#[test]
fn test_qcnot_uninit_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 1).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QCnot {
        dst: 1, src: 0, ctrl_qubit_reg: 0, tgt_qubit_reg: 1,
    }, &mut backend);
    assert!(result.is_err(), "QCNOT on uninitialized Q should fail");
}

/// QCNOT with out-of-range qubit index should return an error.
#[test]
fn test_qcnot_out_of_range_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 99).unwrap(); // out of range

    let result = execute_qop(&mut ctx, &Instruction::QCnot {
        dst: 1, src: 0, ctrl_qubit_reg: 0, tgt_qubit_reg: 1,
    }, &mut backend);
    assert!(result.is_err(), "QCNOT with out-of-range qubit should fail");
}

// =============================================================================
// QROT tests
// =============================================================================

/// QROT Rx(pi) on |0> should flip to |1>.
#[test]
fn test_qrot_rx_pi_flips() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // qubit index in R0, angle in F0
    ctx.iregs.set(0, 0).unwrap();
    ctx.fregs.set(0, std::f64::consts::PI).unwrap();

    execute_qop(&mut ctx, &Instruction::QRot {
        dst: 0, src: 0, qubit_reg: 0, axis: RotAxis::X, angle_freg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    // Rx(pi)|00> = -i|10>; qubit 0 is MSB so flipping it gives index 2 (binary 10)
    assert!((probs[2] - 1.0).abs() < 1e-8,
        "Rx(pi)|00> should flip qubit 0, got probs={:?}", probs);
}

/// QROT Rz(2pi) on |0> should be identity (up to global phase).
#[test]
fn test_qrot_rz_2pi_identity() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    ctx.iregs.set(0, 0).unwrap();
    ctx.fregs.set(0, 2.0 * std::f64::consts::PI).unwrap();

    execute_qop(&mut ctx, &Instruction::QRot {
        dst: 0, src: 0, qubit_reg: 0, axis: RotAxis::Z, angle_freg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    // Rz(2pi)|00> = -|00>, density matrix unchanged
    assert!((probs[0] - 1.0).abs() < 1e-8,
        "Rz(2pi) should preserve |00>, got probs={:?}", probs);
}

/// QROT Ry(pi) on |0> should flip to |1>.
#[test]
fn test_qrot_ry_pi_flips() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    ctx.iregs.set(0, 0).unwrap();
    ctx.fregs.set(0, std::f64::consts::PI).unwrap();

    execute_qop(&mut ctx, &Instruction::QRot {
        dst: 0, src: 0, qubit_reg: 0, axis: RotAxis::Y, angle_freg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    // Ry(pi)|00> flips qubit 0; qubit 0 is MSB so result is index 2 (binary 10)
    assert!((probs[2] - 1.0).abs() < 1e-8,
        "Ry(pi)|00> should flip qubit 0, got probs={:?}", probs);
}

/// QROT with invalid axis should return an error.
#[test]
fn test_qrot_invalid_axis_error() {
    // With type-safe enums, invalid axis values are caught at TryFrom time
    assert!(RotAxis::try_from(5u8).is_err(), "QROT with invalid axis should fail");
}

/// QROT on uninitialized register should return an error.
#[test]
fn test_qrot_uninit_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 0).unwrap();
    ctx.fregs.set(0, 1.0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QRot {
        dst: 0, src: 0, qubit_reg: 0, axis: RotAxis::X, angle_freg: 0,
    }, &mut backend);
    assert!(result.is_err(), "QROT on uninitialized Q should fail");
}

// =============================================================================
// QMEAS tests
// =============================================================================

/// QMEAS on |00> should always measure 0 for qubit 0.
#[test]
fn test_qmeas_zero_state_deterministic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // Measure qubit 0
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QMeas {
        dst_r: 1, src_q: 0, qubit_reg: 0,
    }, &mut backend).unwrap();

    let outcome = ctx.iregs.get(1).unwrap();
    assert_eq!(outcome, 0, "Measuring |00> qubit 0 should always give 0");

    // Post-measurement state should still be valid
    let dm = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");
}

/// QMEAS on |11> should always measure 1 for any qubit.
#[test]
fn test_qmeas_one_state_deterministic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // Flip both qubits to get |11>
    ctx.iregs.set(0, 0b11).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Measure qubit 0
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QMeas {
        dst_r: 1, src_q: 0, qubit_reg: 0,
    }, &mut backend).unwrap();

    let outcome = ctx.iregs.get(1).unwrap();
    assert_eq!(outcome, 1, "Measuring |11> qubit 0 should always give 1");
}

/// QMEAS on uninitialized register should return an error.
#[test]
fn test_qmeas_uninit_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QMeas {
        dst_r: 1, src_q: 0, qubit_reg: 0,
    }, &mut backend);
    assert!(result.is_err(), "QMEAS on uninitialized Q should fail");
}

/// QMEAS with out-of-range qubit index should return an error.
#[test]
fn test_qmeas_out_of_range_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    ctx.iregs.set(0, 99).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QMeas {
        dst_r: 1, src_q: 0, qubit_reg: 0,
    }, &mut backend);
    assert!(result.is_err(), "QMEAS with out-of-range qubit should fail");
}

/// QMEAS on Bell state produces valid post-measurement state.
#[test]
fn test_qmeas_bell_state_valid_post() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Bell }, &mut backend).unwrap();

    // Measure qubit 0
    ctx.iregs.set(0, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QMeas {
        dst_r: 1, src_q: 0, qubit_reg: 0,
    }, &mut backend).unwrap();

    let outcome = ctx.iregs.get(1).unwrap();
    assert!(outcome == 0 || outcome == 1, "Outcome should be 0 or 1");

    // Post-measurement state should be valid density matrix
    let dm = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");

    // Post-measurement: Bell state collapses to either |00> or |11>
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    if outcome == 0 {
        assert!((probs[0] - 1.0).abs() < 1e-6, "After measuring 0, should be in |00>");
    } else {
        assert!((probs[3] - 1.0).abs() < 1e-6, "After measuring 1, should be in |11>");
    }
}

// =============================================================================
// QMIXED — mixed state preparation
// =============================================================================

/// QMIXED loads a mixture of pure states from CMEM and produces a density matrix.
/// We prepare a mixture of |0> (weight 0.5) and |1> (weight 0.5), which should
/// give the maximally mixed 1-qubit state.
#[test]
fn test_qmixed_maximally_mixed() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Layout in CMEM:
    // addr 0: weight_0 = 0.5
    // addr 1: dim_0 = 2
    // addr 2: re(|0>[0]) = 1.0
    // addr 3: im(|0>[0]) = 0.0
    // addr 4: re(|0>[1]) = 0.0
    // addr 5: im(|0>[1]) = 0.0
    // addr 6: weight_1 = 0.5
    // addr 7: dim_1 = 2
    // addr 8: re(|1>[0]) = 0.0
    // addr 9: im(|1>[0]) = 0.0
    // addr 10: re(|1>[1]) = 1.0
    // addr 11: im(|1>[1]) = 0.0

    let w = 0.5f64.to_bits() as i64;
    let one = 1.0f64.to_bits() as i64;
    let zero = 0.0f64.to_bits() as i64;

    ctx.cmem.store(0, w);     // weight 0.5
    ctx.cmem.store(1, 2);     // dim 2
    ctx.cmem.store(2, one);   // re(1.0)
    ctx.cmem.store(3, zero);  // im(0.0)
    ctx.cmem.store(4, zero);  // re(0.0)
    ctx.cmem.store(5, zero);  // im(0.0)
    ctx.cmem.store(6, w);     // weight 0.5
    ctx.cmem.store(7, 2);     // dim 2
    ctx.cmem.store(8, zero);  // re(0.0)
    ctx.cmem.store(9, zero);  // im(0.0)
    ctx.cmem.store(10, one);  // re(1.0)
    ctx.cmem.store(11, zero); // im(0.0)

    // R0 = base_addr = 0, R1 = count = 2
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 2).unwrap();

    execute_qop(&mut ctx, &Instruction::QMixed {
        dst: 0, base_addr_reg: 0, count_reg: 1,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 1);
    // Maximally mixed 1-qubit: diagonal probs are [0.5, 0.5]
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 0.5).abs() < 1e-6);
    assert!((probs[1] - 0.5).abs() < 1e-6);
}

// =============================================================================
// QPREPN — variable qubit count
// =============================================================================

#[test]
fn test_qprepn_zero_state_3_qubits() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // R0 = 3 (qubit count)
    ctx.iregs.set(0, 3).unwrap();

    execute_qop(&mut ctx, &Instruction::QPrepN {
        dst: 0, dist: DistId::Zero, qubit_count_reg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 3);
    assert_eq!(backend.dimension(*dm).unwrap(), 8);
    // |000>: prob[0] = 1.0, all others 0
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-6);
    for i in 1..8 {
        assert!(probs[i].abs() < 1e-6);
    }
}

#[test]
fn test_qprepn_uniform_4_qubits() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 4).unwrap();

    execute_qop(&mut ctx, &Instruction::QPrepN {
        dst: 1, dist: DistId::Uniform, qubit_count_reg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 4);
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    let expected = 1.0 / 16.0;
    for &p in &probs {
        assert!((p - expected).abs() < 1e-6);
    }
}

#[test]
fn test_qprepn_zero_count_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 0).unwrap();

    let result = execute_qop(&mut ctx, &Instruction::QPrepN {
        dst: 0, dist: DistId::Zero, qubit_count_reg: 0,
    }, &mut backend);
    assert!(result.is_err(), "QPREPN with 0 qubits should fail");
}

// =============================================================================
// QPTRACE — partial trace
// =============================================================================

#[test]
fn test_qptrace_bell_state() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Prepare Bell state (2 qubits)
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Bell }, &mut backend).unwrap();

    // Trace out subsystem B (qubit 1), keep subsystem A (qubit 0)
    ctx.iregs.set(0, 1).unwrap(); // num_qubits_a = 1

    execute_qop(&mut ctx, &Instruction::QPtrace {
        dst: 1, src: 0, num_qubits_a_reg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 1);
    // Partial trace of Bell state over one qubit gives maximally mixed state
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 0.5).abs() < 1e-6, "Bell partial trace should be maximally mixed");
    assert!((probs[1] - 0.5).abs() < 1e-6, "Bell partial trace should be maximally mixed");
}

#[test]
fn test_qptrace_separable_state() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Prepare |00> (separable 2-qubit state)
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    ctx.iregs.set(0, 1).unwrap(); // num_qubits_a = 1

    execute_qop(&mut ctx, &Instruction::QPtrace {
        dst: 1, src: 0, num_qubits_a_reg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[1].as_ref().unwrap();
    assert_eq!(backend.num_qubits(*dm).unwrap(), 1);
    // Partial trace of |00> should be |0>
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-6, "Partial trace of |00> should be |0>");
}

#[test]
fn test_qptrace_invalid_count_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    // num_qubits_a = 0 (invalid)
    ctx.iregs.set(0, 0).unwrap();
    let result = execute_qop(&mut ctx, &Instruction::QPtrace {
        dst: 1, src: 0, num_qubits_a_reg: 0,
    }, &mut backend);
    assert!(result.is_err(), "QPTRACE with num_qubits_a=0 should fail");
}

#[test]
fn test_qptrace_uninit_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 1).unwrap();
    let result = execute_qop(&mut ctx, &Instruction::QPtrace {
        dst: 1, src: 0, num_qubits_a_reg: 0,
    }, &mut backend);
    assert!(result.is_err(), "QPTRACE on uninitialized Q should fail");
}

// =============================================================================
// QRESET — qubit reset
// =============================================================================

#[test]
fn test_qreset_zero_state_noop() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // |00>: qubit 0 is already 0, reset should be a no-op
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    ctx.iregs.set(0, 0).unwrap(); // qubit 0

    execute_qop(&mut ctx, &Instruction::QReset {
        dst: 0, src: 0, qubit_reg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    // Should still be |00>
    assert!((probs[0] - 1.0).abs() < 1e-6, "Reset on |00> should remain |00>");
}

#[test]
fn test_qreset_flipped_state() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Prepare |00>, flip qubit 0 to get |01>
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    ctx.iregs.set(0, 0b01).unwrap();
    execute_qop(&mut ctx, &Instruction::QFlip { dst: 0, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    // Now state is |01>; reset qubit 0 should bring back to |00>
    ctx.iregs.set(0, 0).unwrap();
    execute_qop(&mut ctx, &Instruction::QReset {
        dst: 0, src: 0, qubit_reg: 0,
    }, &mut backend).unwrap();

    let dm = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.purity(*dm).unwrap() <= 1.0 + 1e-6, "purity should be <= 1.0");
    let probs = backend.diagonal_probabilities(*dm).unwrap();
    assert!((probs[0] - 1.0).abs() < 1e-6, "Reset should bring |01> back to |00>");
}

#[test]
fn test_qreset_out_of_range_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();

    ctx.iregs.set(0, 99).unwrap(); // invalid qubit
    let result = execute_qop(&mut ctx, &Instruction::QReset {
        dst: 0, src: 0, qubit_reg: 0,
    }, &mut backend);
    assert!(result.is_err(), "QRESET with out-of-range qubit should fail");
}

#[test]
fn test_qreset_uninit_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.iregs.set(0, 0).unwrap();
    let result = execute_qop(&mut ctx, &Instruction::QReset {
        dst: 0, src: 0, qubit_reg: 0,
    }, &mut backend);
    assert!(result.is_err(), "QRESET on uninitialized Q should fail");
}

// =============================================================================
// QuantumRegister integration tests: Pure/Mixed variants
// =============================================================================


#[test]
fn test_qprep_default_uses_statevector() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    // Default: force_density_matrix = false
    assert!(!ctx.config.force_density_matrix);

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    let qr = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.is_pure(*qr).unwrap(),
        "Default QPREP should produce Pure(Statevector)");
    assert_eq!(backend.num_qubits(*qr).unwrap(), 2);
    assert!((backend.purity(*qr).unwrap() - 1.0).abs() < 1e-12);
}

#[test]
fn test_qprep_force_dm_uses_density_matrix() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.config.force_density_matrix = true;

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();

    let qr = ctx.qregs[0].as_ref().unwrap();
    assert!(!backend.is_pure(*qr).unwrap(),
        "QPREP with force_density_matrix should produce Mixed(DensityMatrix)");
    assert_eq!(backend.num_qubits(*qr).unwrap(), 2);
}

#[test]
fn test_qprep_zero_state_variant() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    let qr = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.is_pure(*qr).unwrap());
    assert!((backend.get_element(*qr, 0, 0).unwrap().0 - 1.0).abs() < 1e-10);
}

#[test]
fn test_qprep_bell_variant() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Bell }, &mut backend).unwrap();
    let qr = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.is_pure(*qr).unwrap());
    assert!((backend.get_element(*qr, 0, 0).unwrap().0 - 0.5).abs() < 1e-10);
    assert!((backend.get_element(*qr, 0, 3).unwrap().0 - 0.5).abs() < 1e-10);
}

#[test]
fn test_qkernel_preserves_pure_with_sv_fast_path() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    assert!(backend.is_pure(*ctx.qregs[0].as_ref().unwrap()).unwrap());

    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();
    execute_qop(&mut ctx, &Instruction::QKernel {
        dst: 1, src: 0, kernel: KernelId::Fourier, ctx0: 0, ctx1: 1,
    }, &mut backend).unwrap();

    let qr = ctx.qregs[1].as_ref().unwrap();
    // If the kernel supports apply_sv, result should stay Pure
    // (all built-in kernels support apply_sv)
    assert!(backend.is_pure(*qr).unwrap(),
        "Kernel result should be Pure when SV fast path is available");
}

#[test]
fn test_qmixed_produces_mixed_variant() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Set up a simple mixture in CMEM: 2 states, each 1 qubit (dim=2)
    // State 1: weight=0.5, |0> = (1,0)
    // State 2: weight=0.5, |1> = (0,0, 1,0)
    let mut addr = 0u16;
    // State 1
    ctx.cmem.store(addr, f64::to_bits(0.5) as i64); addr += 1; // weight
    ctx.cmem.store(addr, 2); addr += 1; // dim
    ctx.cmem.store(addr, f64::to_bits(1.0) as i64); addr += 1; // re
    ctx.cmem.store(addr, f64::to_bits(0.0) as i64); addr += 1; // im
    ctx.cmem.store(addr, f64::to_bits(0.0) as i64); addr += 1; // re
    ctx.cmem.store(addr, f64::to_bits(0.0) as i64); addr += 1; // im
    // State 2
    ctx.cmem.store(addr, f64::to_bits(0.5) as i64); addr += 1; // weight
    ctx.cmem.store(addr, 2); addr += 1; // dim
    ctx.cmem.store(addr, f64::to_bits(0.0) as i64); addr += 1; // re
    ctx.cmem.store(addr, f64::to_bits(0.0) as i64); addr += 1; // im
    ctx.cmem.store(addr, f64::to_bits(1.0) as i64); addr += 1; // re
    ctx.cmem.store(addr, f64::to_bits(0.0) as i64);           // im

    ctx.iregs.set(0, 0).unwrap(); // base_addr
    ctx.iregs.set(1, 2).unwrap(); // count

    execute_qop(&mut ctx, &Instruction::QMixed { dst: 0, base_addr_reg: 0, count_reg: 1 }, &mut backend).unwrap();

    let qr = ctx.qregs[0].as_ref().unwrap();
    assert!(!backend.is_pure(*qr).unwrap(),
        "QMIXED should always produce Mixed variant");
    // Maximally mixed 1-qubit state: purity = 0.5
    assert!((backend.purity(*qr).unwrap() - 0.5).abs() < 1e-10,
        "Equal mixture of |0> and |1> should have purity 0.5, got {}", backend.purity(*qr).unwrap());
}

#[test]
fn test_qptrace_auto_promotes_to_mixed() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Create a 2-qubit Bell state (Pure)
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Bell }, &mut backend).unwrap();
    assert!(backend.is_pure(*ctx.qregs[0].as_ref().unwrap()).unwrap());

    // Partial trace over qubit B -> should produce Mixed
    ctx.iregs.set(0, 1).unwrap(); // num_qubits_a = 1
    execute_qop(&mut ctx, &Instruction::QPtrace { dst: 1, src: 0, num_qubits_a_reg: 0 }, &mut backend).unwrap();

    let qr = ctx.qregs[1].as_ref().unwrap();
    assert!(!backend.is_pure(*qr).unwrap(),
        "QPTRACE should always produce Mixed variant (partial trace of Bell state)");
    assert_eq!(backend.num_qubits(*qr).unwrap(), 1);
    // Partial trace of Bell state gives maximally mixed 1-qubit state
    assert!((backend.purity(*qr).unwrap() - 0.5).abs() < 1e-10,
        "Partial trace of Bell state should give purity 0.5, got {}", backend.purity(*qr).unwrap());
}

#[test]
fn test_qstore_qload_preserves_variant() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Store a Pure register
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Uniform }, &mut backend).unwrap();
    assert!(backend.is_pure(*ctx.qregs[0].as_ref().unwrap()).unwrap());

    execute_qop(&mut ctx, &Instruction::QStore { src_q: 0, addr: 42 }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QLoad { dst_q: 1, addr: 42 }, &mut backend).unwrap();

    let qr = ctx.qregs[1].as_ref().unwrap();
    assert!(backend.is_pure(*qr).unwrap(),
        "Pure register should remain Pure after QSTORE/QLOAD roundtrip");
}

#[test]
fn test_qencode_produces_pure() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    // Encode 2 amplitudes (3, 4) from integer registers
    ctx.iregs.set(0, 3).unwrap();
    ctx.iregs.set(1, 4).unwrap();

    execute_qop(&mut ctx, &Instruction::QEncode {
        dst: 0, src_base: 0, count: 2, file_sel: FileSel::RFile, // R_FILE
    }, &mut backend).unwrap();

    let qr = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.is_pure(*qr).unwrap(),
        "QENCODE should always produce Pure variant");
    assert_eq!(backend.num_qubits(*qr).unwrap(), 1);
}

#[test]
fn test_qprepn_uses_force_dm_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();
    ctx.config.force_density_matrix = true;

    ctx.iregs.set(0, 3).unwrap(); // 3 qubits
    execute_qop(&mut ctx, &Instruction::QPrepN { dst: 0, dist: DistId::Zero, qubit_count_reg: 0 }, &mut backend).unwrap();

    let qr = ctx.qregs[0].as_ref().unwrap();
    assert!(!backend.is_pure(*qr).unwrap(),
        "QPREPN with force_density_matrix should produce Mixed variant");
    assert_eq!(backend.num_qubits(*qr).unwrap(), 3);
}

#[test]
fn test_gate_operations_preserve_pure_variant() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    assert!(backend.is_pure(*ctx.qregs[0].as_ref().unwrap()).unwrap());

    // Apply Hadamard via masked gate
    ctx.iregs.set(0, 0b11).unwrap(); // mask: both qubits
    execute_qop(&mut ctx, &Instruction::QHadM { dst: 1, src: 0, mask_reg: 0 }, &mut backend).unwrap();

    let qr = ctx.qregs[1].as_ref().unwrap();
    assert!(backend.is_pure(*qr).unwrap(),
        "Gate operations on Pure should stay Pure");
}

#[test]
fn test_qmeas_preserves_pure_variant() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    assert!(backend.is_pure(*ctx.qregs[0].as_ref().unwrap()).unwrap());

    ctx.iregs.set(0, 0).unwrap(); // measure qubit 0
    execute_qop(&mut ctx, &Instruction::QMeas { dst_r: 1, src_q: 0, qubit_reg: 0 }, &mut backend).unwrap();

    let qr = ctx.qregs[0].as_ref().unwrap();
    assert!(backend.is_pure(*qr).unwrap(),
        "Measurement of Pure should stay Pure");
}

#[test]
fn test_qtensor_pure_pure_stays_pure() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut backend = test_backend();

    ctx.config.default_qubits = 1;
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: DistId::Zero }, &mut backend).unwrap();
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 1, dist: DistId::Zero }, &mut backend).unwrap();

    execute_qop(&mut ctx, &Instruction::QTensor { dst: 2, src0: 0, src1: 1 }, &mut backend).unwrap();

    let qr = ctx.qregs[2].as_ref().unwrap();
    assert!(backend.is_pure(*qr).unwrap(),
        "Tensor of Pure x Pure should be Pure");
    assert_eq!(backend.num_qubits(*qr).unwrap(), 2);
}
