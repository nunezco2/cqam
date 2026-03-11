//! Tests for the Statevector pure-state backend.

use cqam_sim::complex::{self, C64};
use cqam_sim::statevector::Statevector;
use cqam_sim::density_matrix::DensityMatrix;
use cqam_sim::kernel::Kernel;

// =============================================================================
// Construction tests
// =============================================================================

#[test]
fn test_sv_zero_state() {
    let sv = Statevector::new_zero_state(2);
    assert_eq!(sv.num_qubits(), 2);
    assert_eq!(sv.dimension(), 4);
    assert!((sv.amplitude(0).0 - 1.0).abs() < 1e-12);
    for k in 1..4 {
        assert!(sv.amplitude(k).0.abs() < 1e-12 && sv.amplitude(k).1.abs() < 1e-12);
    }
}

#[test]
fn test_sv_uniform() {
    let sv = Statevector::new_uniform(2);
    let expected = 1.0 / 2.0; // 1/sqrt(4) = 0.5
    for k in 0..4 {
        assert!(
            (sv.amplitude(k).0 - expected).abs() < 1e-12,
            "amplitude[{}] = {}, expected {}",
            k, sv.amplitude(k).0, expected
        );
    }
}

#[test]
fn test_sv_bell() {
    let sv = Statevector::new_bell();
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    assert!((sv.amplitude(0).0 - inv_sqrt2).abs() < 1e-12);
    assert!(sv.amplitude(1).0.abs() < 1e-12);
    assert!(sv.amplitude(2).0.abs() < 1e-12);
    assert!((sv.amplitude(3).0 - inv_sqrt2).abs() < 1e-12);
}

#[test]
fn test_sv_ghz() {
    let sv = Statevector::new_ghz(3).unwrap();
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    assert!((sv.amplitude(0).0 - inv_sqrt2).abs() < 1e-12);
    for k in 1..7 {
        assert!(sv.amplitude(k).0.abs() < 1e-12);
    }
    assert!((sv.amplitude(7).0 - inv_sqrt2).abs() < 1e-12);
}

#[test]
fn test_sv_from_amplitudes() {
    let amps = vec![(1.0, 0.0), (0.0, 0.0)];
    let sv = Statevector::from_amplitudes(amps).unwrap();
    assert_eq!(sv.num_qubits(), 1);
    assert!((sv.amplitude(0).0 - 1.0).abs() < 1e-12);
}

#[test]
fn test_sv_from_amplitudes_normalizes() {
    let amps = vec![(2.0, 0.0), (0.0, 0.0)];
    let sv = Statevector::from_amplitudes(amps).unwrap();
    assert!((sv.amplitude(0).0 - 1.0).abs() < 1e-12);
}

#[test]
fn test_sv_from_amplitudes_bad_length() {
    let amps = vec![(1.0, 0.0), (0.0, 0.0), (0.0, 0.0)]; // length 3
    assert!(Statevector::from_amplitudes(amps).is_err());
}

// =============================================================================
// Unitary Application tests
// =============================================================================

#[test]
fn test_sv_hadamard() {
    let mut sv = Statevector::new_zero_state(1);
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    let h_gate: [C64; 4] = [
        (inv_sqrt2, 0.0), (inv_sqrt2, 0.0),
        (inv_sqrt2, 0.0), (-inv_sqrt2, 0.0),
    ];
    sv.apply_single_qubit_gate(0, &h_gate);

    // H|0> = |+>
    assert!((sv.amplitude(0).0 - inv_sqrt2).abs() < 1e-12);
    assert!((sv.amplitude(1).0 - inv_sqrt2).abs() < 1e-12);
}

#[test]
fn test_sv_not_gate() {
    let mut sv = Statevector::new_zero_state(1);
    let x_gate: Vec<C64> = vec![
        complex::ZERO, complex::ONE,
        complex::ONE, complex::ZERO,
    ];
    sv.apply_unitary(&x_gate);
    assert!(sv.amplitude(0).0.abs() < 1e-12);
    assert!((sv.amplitude(1).0 - 1.0).abs() < 1e-12);
}

#[test]
fn test_sv_apply_unitary_matches_dm() {
    // Create the same state as SV and DM, apply same unitary, compare probabilities
    let sv = Statevector::new_uniform(2);
    let dm = DensityMatrix::new_uniform(2);

    // Apply Fourier transform to both
    use cqam_sim::kernels::fourier::Fourier;
    let fourier = Fourier;

    let dm_result = fourier.apply(&dm).unwrap();
    let sv_result = fourier.apply_sv(&sv).unwrap();

    let dm_probs = dm_result.diagonal_probabilities();
    let sv_probs = sv_result.diagonal_probabilities();

    for (k, (&dp, &sp)) in dm_probs.iter().zip(sv_probs.iter()).enumerate() {
        assert!(
            (dp - sp).abs() < 1e-10,
            "Prob mismatch at |{}> : dm={}, sv={}", k, dp, sp
        );
    }
}

// =============================================================================
// Measurement tests
// =============================================================================

#[test]
fn test_sv_measure_deterministic() {
    let sv = Statevector::new_zero_state(2);
    assert_eq!(sv.measure_deterministic(), 0);
}

#[test]
fn test_sv_measure_all_zero_state() {
    let sv = Statevector::new_zero_state(2);
    for _ in 0..20 {
        let (outcome, collapsed) = sv.measure_all();
        assert_eq!(outcome, 0);
        assert!((collapsed.amplitude(0).0 - 1.0).abs() < 1e-12);
    }
}

#[test]
fn test_sv_diagonal_probabilities() {
    let sv = Statevector::new_uniform(2);
    let probs = sv.diagonal_probabilities();
    assert_eq!(probs.len(), 4);
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10);
    }
}

// =============================================================================
// Metrics tests
// =============================================================================

#[test]
fn test_sv_purity_always_one() {
    let states = [
        Statevector::new_zero_state(2),
        Statevector::new_uniform(2),
        Statevector::new_bell(),
        Statevector::new_ghz(3).unwrap(),
    ];
    for (i, sv) in states.iter().enumerate() {
        assert!(
            (sv.purity() - 1.0).abs() < 1e-12,
            "State {} purity should be 1.0, got {}", i, sv.purity()
        );
    }
}

#[test]
fn test_sv_to_density_matrix() {
    let sv = Statevector::new_bell();
    let dm = sv.to_density_matrix();

    assert_eq!(dm.num_qubits(), 2);
    assert!((dm.purity() - 1.0).abs() < 1e-10);
    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(0, 3).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(3, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(3, 3).0 - 0.5).abs() < 1e-10);
}

#[test]
fn test_sv_to_dm_roundtrip_probabilities() {
    let sv = Statevector::new_uniform(3);
    let dm = sv.to_density_matrix();

    let sv_probs = sv.diagonal_probabilities();
    let dm_probs = dm.diagonal_probabilities();

    for (k, (&sp, &dp)) in sv_probs.iter().zip(dm_probs.iter()).enumerate() {
        assert!(
            (sp - dp).abs() < 1e-10,
            "Prob mismatch at |{}> : sv={}, dm={}", k, sp, dp
        );
    }
}

#[test]
fn test_sv_tensor_product() {
    let sv0 = Statevector::new_zero_state(1);
    let sv1 = Statevector::new_zero_state(1);
    let product = sv0.tensor_product(&sv1).unwrap();

    assert_eq!(product.num_qubits(), 2);
    assert!((product.amplitude(0).0 - 1.0).abs() < 1e-12);
    for k in 1..4 {
        assert!(product.amplitude(k).0.abs() < 1e-12);
    }
}

// =============================================================================
// Kernel apply_sv tests
// =============================================================================

#[test]
fn test_init_sv() {
    use cqam_sim::kernels::init::Init;
    let sv = Statevector::new_zero_state(2);
    let result = Init.apply_sv(&sv).unwrap();
    let probs = result.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10);
    }
}

#[test]
fn test_fourier_sv() {
    use cqam_sim::kernels::fourier::Fourier;
    let sv = Statevector::new_zero_state(2);
    let result = Fourier.apply_sv(&sv).unwrap();
    // QFT|0> = uniform superposition
    let probs = result.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10);
    }
}

#[test]
fn test_grover_sv_2q() {
    use cqam_sim::kernels::grover::GroverIter;
    let sv = Statevector::new_uniform(2);
    let grover = GroverIter::single(3);
    let result = grover.apply_sv(&sv).unwrap();

    let probs = result.diagonal_probabilities();
    assert!(
        (probs[3] - 1.0).abs() < 1e-10,
        "Grover 2q SV target=3: p[3]={} should be 1.0", probs[3]
    );
}

#[test]
fn test_grover_sv_matches_dm() {
    use cqam_sim::kernels::grover::GroverIter;
    let sv = Statevector::new_uniform(3);
    let dm = DensityMatrix::new_uniform(3);
    let grover = GroverIter::single(5);

    let sv_result = grover.apply_sv(&sv).unwrap();
    let dm_result = grover.apply(&dm).unwrap();

    let sv_probs = sv_result.diagonal_probabilities();
    let dm_probs = dm_result.diagonal_probabilities();

    for (k, (&sp, &dp)) in sv_probs.iter().zip(dm_probs.iter()).enumerate() {
        assert!(
            (sp - dp).abs() < 1e-8,
            "Grover prob mismatch at |{}> : sv={}, dm={}", k, sp, dp
        );
    }
}

#[test]
fn test_diffuse_sv_matches_dm() {
    use cqam_sim::kernels::diffuse::Diffuse;
    let sv = Statevector::new_zero_state(2);
    let dm = DensityMatrix::new_zero_state(2);

    let sv_result = Diffuse.apply_sv(&sv).unwrap();
    let dm_result = Diffuse.apply(&dm).unwrap();

    let sv_probs = sv_result.diagonal_probabilities();
    let dm_probs = dm_result.diagonal_probabilities();

    for (k, (&sp, &dp)) in sv_probs.iter().zip(dm_probs.iter()).enumerate() {
        assert!(
            (sp - dp).abs() < 1e-10,
            "Diffuse prob mismatch at |{}> : sv={}, dm={}", k, sp, dp
        );
    }
}

// =============================================================================
// QuantumRegister dispatch tests
// =============================================================================

#[test]
fn test_quantum_register_pure() {
    use cqam_sim::quantum_register::QuantumRegister;

    let sv = Statevector::new_uniform(2);
    let qr = QuantumRegister::Pure(sv);

    assert_eq!(qr.num_qubits(), 2);
    assert_eq!(qr.dimension(), 4);
    assert!((qr.purity() - 1.0).abs() < 1e-12);

    let probs = qr.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10);
    }
}

#[test]
fn test_quantum_register_mixed() {
    use cqam_sim::quantum_register::QuantumRegister;

    let dm = DensityMatrix::new_bell();
    let qr = QuantumRegister::Mixed(dm);

    assert_eq!(qr.num_qubits(), 2);
    assert!((qr.purity() - 1.0).abs() < 1e-10);
}

#[test]
fn test_quantum_register_to_density_matrix() {
    use cqam_sim::quantum_register::QuantumRegister;

    let sv = Statevector::new_bell();
    let qr = QuantumRegister::Pure(sv);
    let dm = qr.to_density_matrix();

    assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10);
    assert!((dm.get(3, 3).0 - 0.5).abs() < 1e-10);
}

#[test]
fn test_quantum_register_get_element() {
    use cqam_sim::quantum_register::QuantumRegister;

    let sv = Statevector::new_bell();
    let qr_pure = QuantumRegister::Pure(sv.clone());
    let qr_mixed = QuantumRegister::Mixed(sv.to_density_matrix());

    // Both should give the same element values
    for i in 0..4 {
        for j in 0..4 {
            let pure_elem = qr_pure.get_element(i, j);
            let mixed_elem = qr_mixed.get_element(i, j);
            assert!(
                (pure_elem.0 - mixed_elem.0).abs() < 1e-10 &&
                (pure_elem.1 - mixed_elem.1).abs() < 1e-10,
                "Element mismatch at ({}, {}): pure={:?}, mixed={:?}",
                i, j, pure_elem, mixed_elem
            );
        }
    }
}

// =============================================================================
// Entanglement detection tests
// =============================================================================

#[test]
fn test_entanglement_single_qubit() {
    // A single-qubit statevector cannot be entangled.
    let sv = Statevector::new_zero_state(1);
    assert!(!sv.is_any_qubit_entangled(),
        "single-qubit state should never be entangled");
}

#[test]
fn test_entanglement_product_state() {
    // |00> is a product state -- no entanglement.
    let sv = Statevector::new_zero_state(2);
    assert!(!sv.is_any_qubit_entangled(),
        "|00> is a product state, should not be entangled");
}

#[test]
fn test_entanglement_bell_state() {
    // (|00> + |11>)/sqrt(2) is maximally entangled.
    let sv = Statevector::new_bell();
    assert!(sv.is_any_qubit_entangled(),
        "Bell state should be entangled");
    let min_purity = sv.min_single_qubit_purity();
    assert!(
        (min_purity - 0.5).abs() < 1e-10,
        "Bell state min single-qubit purity should be 0.5, got {}",
        min_purity
    );
}

#[test]
fn test_entanglement_superposition_no_entanglement() {
    // |+> tensor |+> = (|00> + |01> + |10> + |11>)/2 is a product state
    // of two superposed qubits -- NOT entangled.
    let sv = Statevector::new_uniform(2);
    assert!(!sv.is_any_qubit_entangled(),
        "|+>|+> is a product state, should not be entangled");
}

#[test]
fn test_entanglement_ghz_3qubit() {
    // (|000> + |111>)/sqrt(2) is entangled across all 3 qubits.
    let sv = Statevector::new_ghz(3).unwrap();
    assert!(sv.is_any_qubit_entangled(),
        "3-qubit GHZ state should be entangled");
    let min_purity = sv.min_single_qubit_purity();
    assert!(
        (min_purity - 0.5).abs() < 1e-10,
        "GHZ min single-qubit purity should be 0.5, got {}",
        min_purity
    );
}

#[test]
fn test_entanglement_partial() {
    // Bell(q0,q1) tensor |0>(q2): entanglement between q0 and q1, q2 is separable.
    let bell = Statevector::new_bell();
    let zero = Statevector::new_zero_state(1);
    let sv = bell.tensor_product(&zero).unwrap();

    assert_eq!(sv.num_qubits(), 3);
    assert!(sv.is_any_qubit_entangled(),
        "Bell(q0,q1) tensor |0>(q2) should detect entanglement between q0 and q1");

    // min purity should be 0.5 (from the entangled pair), not 1.0
    let min_purity = sv.min_single_qubit_purity();
    assert!(
        (min_purity - 0.5).abs() < 1e-10,
        "Partial entanglement min purity should be 0.5 (from q0 or q1), got {}",
        min_purity
    );
}
