//! Comprehensive tests for the `DensityMatrix` type: construction, trace,
//! purity, unitary application, and measurement probabilities.

use cqam_sim::complex::{self, C64};
use cqam_sim::density_matrix::DensityMatrix;

// =============================================================================
// Construction tests
// =============================================================================

#[test]
fn test_zero_state_2q() {
    let dm = DensityMatrix::new_zero_state(2);
    assert_eq!(dm.num_qubits(), 2);
    assert_eq!(dm.dimension(), 4);
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10);
    // All other entries zero
    for i in 0..4 {
        for j in 0..4 {
            if i == 0 && j == 0 { continue; }
            assert!(dm.get(i, j).0.abs() < 1e-10 && dm.get(i, j).1.abs() < 1e-10,
                "rho[{}][{}] should be 0, got {:?}", i, j, dm.get(i, j));
        }
    }
    let tr = dm.trace();
    assert!((tr.0 - 1.0).abs() < 1e-10);
}

#[test]
fn test_zero_state_1q() {
    let dm = DensityMatrix::new_zero_state(1);
    assert_eq!(dm.num_qubits(), 1);
    assert_eq!(dm.dimension(), 2);
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10);
    assert!((dm.get(1, 1).0).abs() < 1e-10);
}

#[test]
fn test_uniform_2q() {
    let dm = DensityMatrix::new_uniform(2);
    // All entries should be 0.25
    for i in 0..4 {
        for j in 0..4 {
            assert!((dm.get(i, j).0 - 0.25).abs() < 1e-10,
                "rho[{}][{}] should be 0.25, got {}", i, j, dm.get(i, j).0);
        }
    }
    let tr = dm.trace();
    assert!((tr.0 - 1.0).abs() < 1e-10);
    assert!((dm.purity() - 1.0).abs() < 1e-10, "Uniform pure state should have purity 1.0");
}

#[test]
fn test_bell_state() {
    let dm = DensityMatrix::new_bell();
    assert_eq!(dm.num_qubits(), 2);
    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(0, 3).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(3, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(3, 3).0 - 0.5).abs() < 1e-10);
    // Other entries zero
    assert!(dm.get(1, 1).0.abs() < 1e-10);
    assert!(dm.get(2, 2).0.abs() < 1e-10);
}

#[test]
fn test_ghz_3q() {
    let dm = DensityMatrix::new_ghz(3);
    assert_eq!(dm.num_qubits(), 3);
    assert_eq!(dm.dimension(), 8);
    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(0, 7).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(7, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(7, 7).0 - 0.5).abs() < 1e-10);
}

#[test]
fn test_from_statevector() {
    // |+> = (|0> + |1>) / sqrt(2)
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    let psi: Vec<C64> = vec![(inv_sqrt2, 0.0), (inv_sqrt2, 0.0)];
    let dm = DensityMatrix::from_statevector(&psi).unwrap();

    assert_eq!(dm.num_qubits(), 1);
    // All entries should be 0.5
    for i in 0..2 {
        for j in 0..2 {
            assert!((dm.get(i, j).0 - 0.5).abs() < 1e-10,
                "rho[{}][{}] should be 0.5, got {}", i, j, dm.get(i, j).0);
        }
    }
}

#[test]
fn test_from_statevector_bad_length() {
    let psi: Vec<C64> = vec![(1.0, 0.0), (0.0, 0.0), (0.0, 0.0)]; // length 3
    assert!(DensityMatrix::from_statevector(&psi).is_err());
}

// =============================================================================
// Unitary application tests
// =============================================================================

#[test]
fn test_apply_unitary_identity() {
    let mut dm = DensityMatrix::new_zero_state(1);
    // Identity matrix for 1 qubit
    let identity: Vec<C64> = vec![
        complex::ONE, complex::ZERO,
        complex::ZERO, complex::ONE,
    ];
    dm.apply_unitary(&identity);
    assert!((dm.get(0, 0).0 - 1.0).abs() < 1e-10);
    assert!(dm.get(1, 1).0.abs() < 1e-10);
}

#[test]
fn test_apply_unitary_not_gate() {
    let mut dm = DensityMatrix::new_zero_state(1);
    // X (NOT) gate
    let x_gate: Vec<C64> = vec![
        complex::ZERO, complex::ONE,
        complex::ONE, complex::ZERO,
    ];
    dm.apply_unitary(&x_gate);
    // |0> -> |1>, so rho becomes |1><1|
    assert!(dm.get(0, 0).0.abs() < 1e-10);
    assert!((dm.get(1, 1).0 - 1.0).abs() < 1e-10);
}

#[test]
fn test_apply_unitary_hadamard() {
    let mut dm = DensityMatrix::new_zero_state(1);
    // Hadamard gate
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    let h_gate: Vec<C64> = vec![
        (inv_sqrt2, 0.0), (inv_sqrt2, 0.0),
        (inv_sqrt2, 0.0), (-inv_sqrt2, 0.0),
    ];
    dm.apply_unitary(&h_gate);
    // H|0> = |+>, so rho = |+><+| = [[0.5, 0.5], [0.5, 0.5]]
    for i in 0..2 {
        for j in 0..2 {
            assert!((dm.get(i, j).0 - 0.5).abs() < 1e-10,
                "H|0> rho[{}][{}] should be 0.5, got {}", i, j, dm.get(i, j).0);
        }
    }
}

// =============================================================================
// Measurement tests
// =============================================================================

#[test]
fn test_measure_zero_state() {
    let dm = DensityMatrix::new_zero_state(2);
    // All measurements should return 0
    for _ in 0..100 {
        let (outcome, collapsed) = dm.measure_all();
        assert_eq!(outcome, 0, "Zero state should always measure 0");
        assert!((collapsed.get(0, 0).0 - 1.0).abs() < 1e-10);
    }
}

#[test]
fn test_measure_deterministic() {
    let dm = DensityMatrix::new_zero_state(2);
    assert_eq!(dm.measure_deterministic(), 0);

    // Bell state: should pick state 0 or 3 (both have p=0.5)
    let bell = DensityMatrix::new_bell();
    let result = bell.measure_deterministic();
    assert!(result == 0 || result == 3, "Bell argmax should be 0 or 3, got {}", result);
}

#[test]
fn test_diagonal_probabilities() {
    let dm = DensityMatrix::new_uniform(2);
    let probs = dm.diagonal_probabilities();
    assert_eq!(probs.len(), 4);
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10);
    }
}

// =============================================================================
// Metric tests
// =============================================================================

#[test]
fn test_purity_pure_state() {
    // All pure states should have purity 1.0
    let dm = DensityMatrix::new_zero_state(2);
    assert!((dm.purity() - 1.0).abs() < 1e-10, "Zero state purity should be 1.0, got {}", dm.purity());

    let dm = DensityMatrix::new_uniform(2);
    assert!((dm.purity() - 1.0).abs() < 1e-10, "Uniform pure state purity should be 1.0, got {}", dm.purity());

    let dm = DensityMatrix::new_bell();
    assert!((dm.purity() - 1.0).abs() < 1e-10, "Bell state purity should be 1.0, got {}", dm.purity());
}

#[test]
fn test_purity_mixed_state() {
    // Maximally mixed 1-qubit state: (1/2) * I
    let mut dm = DensityMatrix::new_zero_state(1);
    dm.set(0, 0, (0.5, 0.0));
    dm.set(1, 1, (0.5, 0.0));
    // purity = Tr(rho^2) = 0.25 + 0.25 = 0.5
    assert!((dm.purity() - 0.5).abs() < 1e-10, "Maximally mixed 1q purity should be 0.5, got {}", dm.purity());
}

#[test]
fn test_entropy_pure() {
    // Zero state: pure state -> true von Neumann entropy = 0
    let dm = DensityMatrix::new_zero_state(2);
    assert!(dm.von_neumann_entropy().abs() < 1e-8,
        "Pure zero state VNE should be 0, got {}", dm.von_neumann_entropy());
}

#[test]
fn test_entropy_uniform_diag() {
    // Uniform pure state: diagonal entropy is 1.0 (spread measurement probs)
    // but true von Neumann entropy is 0 (pure state).
    // Test diagonal_entropy for backward-compatible behavior:
    let dm = DensityMatrix::new_uniform(2);
    assert!(
        (dm.diagonal_entropy() - 1.0).abs() < 1e-10,
        "Uniform pure state diagonal entropy should be 1.0, got {}",
        dm.diagonal_entropy()
    );
    // True von Neumann entropy of any pure state is 0:
    assert!(
        dm.von_neumann_entropy().abs() < 1e-8,
        "Uniform pure state true VNE should be 0, got {}",
        dm.von_neumann_entropy()
    );
}

// =============================================================================
// Eigenvalue tests
// =============================================================================

#[test]
fn test_eigenvalues_pure_state() {
    let dm = DensityMatrix::new_zero_state(2);
    let eigs = dm.eigenvalues();
    assert_eq!(eigs.len(), 4);
    assert!((eigs[0] - 1.0).abs() < 1e-10, "Leading eigenvalue should be 1.0, got {}", eigs[0]);
    for &e in &eigs[1..] {
        assert!(e.abs() < 1e-10, "Non-leading eigenvalue should be 0, got {}", e);
    }
}

#[test]
fn test_eigenvalues_maximally_mixed() {
    let mut dm = DensityMatrix::new_zero_state(2);
    for i in 0..4 {
        for j in 0..4 {
            dm.set(i, j, if i == j { (0.25, 0.0) } else { (0.0, 0.0) });
        }
    }
    let eigs = dm.eigenvalues();
    for &e in &eigs {
        assert!((e - 0.25).abs() < 1e-10, "Eigenvalue should be 0.25, got {}", e);
    }
}

#[test]
fn test_eigenvalues_bell_state() {
    let dm = DensityMatrix::new_bell();
    let eigs = dm.eigenvalues();
    assert!((eigs[0] - 1.0).abs() < 1e-10, "Bell leading eigenvalue = {}", eigs[0]);
    for &e in &eigs[1..] {
        assert!(e.abs() < 1e-10, "Bell non-leading eigenvalue = {}", e);
    }
}

#[test]
fn test_vne_maximally_mixed_is_one() {
    let mut dm = DensityMatrix::new_zero_state(2);
    for i in 0..4 {
        for j in 0..4 {
            dm.set(i, j, if i == j { (0.25, 0.0) } else { (0.0, 0.0) });
        }
    }
    assert!(
        (dm.von_neumann_entropy() - 1.0).abs() < 1e-8,
        "Maximally mixed VNE should be 1.0, got {}", dm.von_neumann_entropy()
    );
}

#[test]
fn test_vne_all_pure_states_zero() {
    for dm in &[
        DensityMatrix::new_zero_state(2),
        DensityMatrix::new_uniform(2),
        DensityMatrix::new_bell(),
        DensityMatrix::new_ghz(3),
    ] {
        assert!(dm.von_neumann_entropy().abs() < 1e-8,
            "Pure state VNE should be 0, got {}", dm.von_neumann_entropy());
    }
}

#[test]
fn test_trace() {
    let dm = DensityMatrix::new_zero_state(2);
    let tr = dm.trace();
    assert!((tr.0 - 1.0).abs() < 1e-10);
    assert!(tr.1.abs() < 1e-10);
}

#[test]
fn test_is_valid() {
    let dm = DensityMatrix::new_zero_state(2);
    assert!(dm.is_valid(1e-10));

    let dm = DensityMatrix::new_uniform(2);
    assert!(dm.is_valid(1e-10));

    let dm = DensityMatrix::new_bell();
    assert!(dm.is_valid(1e-10));
}

// =============================================================================
// CNOT entanglement test
// =============================================================================

#[test]
fn test_cnot_on_plus_zero_gives_bell() {
    // |+>|0> = (1/sqrt(2))(|00> + |10>)
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    let psi: Vec<C64> = vec![
        (inv_sqrt2, 0.0), (0.0, 0.0), (inv_sqrt2, 0.0), (0.0, 0.0),
    ];
    let mut dm = DensityMatrix::from_statevector(&psi).unwrap();

    // CNOT (qubit 0 control, qubit 1 target)
    let cnot: Vec<C64> = vec![
        complex::ONE,  complex::ZERO, complex::ZERO, complex::ZERO,
        complex::ZERO, complex::ONE,  complex::ZERO, complex::ZERO,
        complex::ZERO, complex::ZERO, complex::ZERO, complex::ONE,
        complex::ZERO, complex::ZERO, complex::ONE,  complex::ZERO,
    ];
    dm.apply_unitary(&cnot);

    // Should be Bell state
    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10, "rho[0][0]={}", dm.get(0, 0).0);
    assert!((dm.get(0, 3).0 - 0.5).abs() < 1e-10, "rho[0][3]={}", dm.get(0, 3).0);
    assert!((dm.get(3, 0).0 - 0.5).abs() < 1e-10, "rho[3][0]={}", dm.get(3, 0).0);
    assert!((dm.get(3, 3).0 - 0.5).abs() < 1e-10, "rho[3][3]={}", dm.get(3, 3).0);
}

// =============================================================================
// QFT verification tests
// =============================================================================

#[test]
fn test_qft_zero_produces_uniform() {
    use cqam_sim::kernels::fourier::Fourier;
    use cqam_sim::kernel::Kernel;

    let input = DensityMatrix::new_zero_state(2);
    let fourier = Fourier;
    let output = fourier.apply(&input).unwrap();

    let probs = output.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10, "QFT|0> should be uniform, got p={}", p);
    }
}

#[test]
fn test_qft_uniform_produces_zero() {
    use cqam_sim::kernels::fourier::Fourier;
    use cqam_sim::kernel::Kernel;

    let input = DensityMatrix::new_uniform(2);
    let fourier = Fourier;
    let output = fourier.apply(&input).unwrap();

    let probs = output.diagonal_probabilities();
    assert!(probs[0] > 0.99, "QFT on uniform should give |0>, got p[0]={}", probs[0]);
}

// =============================================================================
// Grover verification tests
// =============================================================================

#[test]
fn test_grover_2q_target3_probability_one() {
    use cqam_sim::kernels::grover::GroverIter;
    use cqam_sim::kernel::Kernel;

    let input = DensityMatrix::new_uniform(2);
    let grover = GroverIter::single(3);
    let output = grover.apply(&input).unwrap();

    let probs = output.diagonal_probabilities();
    assert!(
        (probs[3] - 1.0).abs() < 1e-10,
        "Grover 2q target=3 after 1 iter: p[3]={} should be 1.0",
        probs[3]
    );
}

#[test]
fn test_grover_3q_multi_iteration() {
    use cqam_sim::kernels::grover::GroverIter;
    use cqam_sim::kernel::Kernel;

    let mut dm = DensityMatrix::new_uniform(3);
    let grover = GroverIter::single(5);

    // For N=8, optimal is ~2 iterations
    for _ in 0..2 {
        dm = grover.apply(&dm).unwrap();
    }

    let probs = dm.diagonal_probabilities();
    assert!(
        probs[5] > 0.9,
        "Grover 3q target=5 after 2 iters: p[5]={} should be > 0.9",
        probs[5]
    );
}

// =============================================================================
// Display test
// =============================================================================

// --- Partial trace tests ---

#[test]
fn test_partial_trace_b_bell_gives_maximally_mixed() {
    // Bell state |Phi+> = (|00> + |11>) / sqrt(2)
    // Tracing out qubit B (second qubit) should give the maximally mixed state
    // rho_A = (1/2) I = [[0.5, 0], [0, 0.5]]
    let bell = DensityMatrix::new_bell();
    let rho_a = bell.partial_trace_b(1);

    assert_eq!(rho_a.num_qubits(), 1);
    assert_eq!(rho_a.dimension(), 2);

    // Diagonal: both 0.5
    assert!((rho_a.get(0, 0).0 - 0.5).abs() < 1e-10,
        "rho_A[0][0] should be 0.5, got {}", rho_a.get(0, 0).0);
    assert!((rho_a.get(1, 1).0 - 0.5).abs() < 1e-10,
        "rho_A[1][1] should be 0.5, got {}", rho_a.get(1, 1).0);

    // Off-diagonal: zero (no coherence in maximally mixed state)
    assert!(rho_a.get(0, 1).0.abs() < 1e-10,
        "rho_A[0][1] should be 0, got {}", rho_a.get(0, 1).0);
    assert!(rho_a.get(1, 0).0.abs() < 1e-10,
        "rho_A[1][0] should be 0, got {}", rho_a.get(1, 0).0);

    // Purity of maximally mixed 1-qubit state: Tr(rho^2) = 0.5
    assert!((rho_a.purity() - 0.5).abs() < 1e-10,
        "Maximally mixed 1q purity should be 0.5, got {}", rho_a.purity());

    // Trace should be 1
    let tr = rho_a.trace();
    assert!((tr.0 - 1.0).abs() < 1e-10);
}

#[test]
fn test_partial_trace_b_product_state_gives_pure() {
    // Product state |00> = |0> tensor |0>
    // Tracing out qubit B should give |0><0| = [[1, 0], [0, 0]]
    let zero_state = DensityMatrix::new_zero_state(2);
    let rho_a = zero_state.partial_trace_b(1);

    assert_eq!(rho_a.num_qubits(), 1);
    assert!((rho_a.get(0, 0).0 - 1.0).abs() < 1e-10,
        "rho_A[0][0] should be 1.0, got {}", rho_a.get(0, 0).0);
    assert!(rho_a.get(1, 1).0.abs() < 1e-10,
        "rho_A[1][1] should be 0, got {}", rho_a.get(1, 1).0);

    // Purity of pure state: 1.0
    assert!((rho_a.purity() - 1.0).abs() < 1e-10);
}

#[test]
fn test_partial_trace_b_ghz_3q() {
    // GHZ state: (|000> + |111>) / sqrt(2)
    // Tracing out qubits B (last 2 qubits, partition: 1 qubit A, 2 qubits B)
    // rho_A = Tr_B(|GHZ><GHZ|) = (1/2)(|0><0| + |1><1|) = (1/2)I
    let ghz = DensityMatrix::new_ghz(3);
    let rho_a = ghz.partial_trace_b(1);

    assert_eq!(rho_a.num_qubits(), 1);
    assert!((rho_a.get(0, 0).0 - 0.5).abs() < 1e-10);
    assert!((rho_a.get(1, 1).0 - 0.5).abs() < 1e-10);
    assert!(rho_a.get(0, 1).0.abs() < 1e-10);
    assert!(rho_a.get(1, 0).0.abs() < 1e-10);
}

#[test]
#[should_panic]
fn test_partial_trace_b_panics_on_zero_partition() {
    let bell = DensityMatrix::new_bell();
    bell.partial_trace_b(0);
}

#[test]
#[should_panic]
fn test_partial_trace_b_panics_on_full_partition() {
    let bell = DensityMatrix::new_bell();
    bell.partial_trace_b(2); // num_qubits_a == num_qubits, invalid
}

// --- Entanglement entropy tests ---

#[test]
fn test_entanglement_entropy_bell_is_one() {
    // Bell state |Phi+>: maximally entangled 2-qubit state.
    // Entanglement entropy S(rho_A) = 1.0 bit.
    let bell = DensityMatrix::new_bell();
    let ee = bell.entanglement_entropy(1);
    assert!(
        (ee - 1.0).abs() < 1e-10,
        "Bell state entanglement entropy should be 1.0, got {}", ee
    );
}

#[test]
fn test_entanglement_entropy_product_state_is_zero() {
    // Product state |00>: no entanglement.
    // Entanglement entropy S(rho_A) = 0.0.
    let zero_state = DensityMatrix::new_zero_state(2);
    let ee = zero_state.entanglement_entropy(1);
    assert!(
        ee.abs() < 1e-10,
        "Product state entanglement entropy should be 0.0, got {}", ee
    );
}

#[test]
fn test_entanglement_entropy_ghz_3q_is_one() {
    // GHZ state for 3 qubits: tracing out last 2 gives maximally mixed 1-qubit.
    // S(rho_A) = log2(2) = 1.0 bit.
    let ghz = DensityMatrix::new_ghz(3);
    let ee = ghz.entanglement_entropy(1);
    assert!(
        (ee - 1.0).abs() < 1e-10,
        "GHZ-3 entanglement entropy (1|2 partition) should be 1.0, got {}", ee
    );
}

#[test]
fn test_entanglement_entropy_uniform_pure_state() {
    // Uniform pure state H^2|0> = (1/2)(|00> + |01> + |10> + |11>)
    // This is a product state: (H|0>) tensor (H|0>) = |+>|+>
    // rho_A = |+><+| = [[0.5, 0.5], [0.5, 0.5]], which is pure.
    // Since rho_A is pure, entanglement entropy should be 0.
    //
    // With eigenvalue-based entropy, this is now correctly computed as 0.
    let uniform = DensityMatrix::new_uniform(2);
    let ee = uniform.entanglement_entropy(1);

    assert!(
        ee.abs() < 1e-8,
        "Product state (|+>|+>) entanglement entropy should be 0, got {}", ee
    );
}

// =============================================================================
// Display test
// =============================================================================

// =============================================================================
// Entanglement detection tests (is_any_qubit_entangled)
// =============================================================================

#[test]
fn test_dm_entanglement_product() {
    // |00><00| is a product state -- no entanglement.
    let dm = DensityMatrix::new_zero_state(2);
    assert!(!dm.is_any_qubit_entangled(),
        "|00><00| is a product state, should not be entangled");
}

#[test]
fn test_dm_entanglement_bell() {
    // Bell state DM is maximally entangled.
    let dm = DensityMatrix::new_bell();
    assert!(dm.is_any_qubit_entangled(),
        "Bell state DM should be entangled");
}

// =============================================================================
// Display test
// =============================================================================

#[test]
fn test_display_does_not_panic() {
    let dm = DensityMatrix::new_zero_state(2);
    let s = format!("{}", dm);
    assert!(s.contains("DensityMatrix"));
    assert!(s.contains("2 qubits"));
}
