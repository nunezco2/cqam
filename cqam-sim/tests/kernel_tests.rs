// cqam-sim/tests/kernel_tests.rs
//
// Phase 2 (density matrix): Test kernels operating on DensityMatrix.

use cqam_sim::density_matrix::DensityMatrix;
use cqam_sim::kernels::init::Init;
use cqam_sim::kernels::entangle::Entangle;
use cqam_sim::kernels::fourier::Fourier;
use cqam_sim::kernels::diffuse::Diffuse;
use cqam_sim::kernels::grover::GroverIter;
use cqam_sim::kernel::Kernel;

// =============================================================================
// Init kernel tests
// =============================================================================

#[test]
fn test_init_returns_uniform() {
    let init = Init;
    let input = DensityMatrix::new_zero_state(2);
    let output = init.apply(&input);

    // All diagonal entries should be 0.25
    let probs = output.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10, "Init should produce uniform, got p={}", p);
    }
    assert!((output.purity() - 1.0).abs() < 1e-10, "Init should produce pure state");
}

// =============================================================================
// Fourier kernel tests
// =============================================================================

#[test]
fn test_fourier_zero_to_uniform() {
    // QFT on |0><0| should produce uniform superposition
    let input = DensityMatrix::new_zero_state(2);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    let probs = output.diagonal_probabilities();
    for &p in &probs {
        assert!(
            (p - 0.25).abs() < 1e-10,
            "QFT of |0> should be uniform, got p={}",
            p
        );
    }
}

#[test]
fn test_fourier_uniform_to_zero() {
    // QFT on uniform pure state should produce |0><0|
    let input = DensityMatrix::new_uniform(2);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    let probs = output.diagonal_probabilities();
    assert!(
        probs[0] > 0.99,
        "QFT of uniform should concentrate on state 0, got p[0]={}",
        probs[0]
    );
}

#[test]
fn test_fourier_preserves_purity() {
    let input = DensityMatrix::new_zero_state(2);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    assert!(
        (output.purity() - 1.0).abs() < 1e-10,
        "QFT should preserve purity, got {}",
        output.purity()
    );
}

#[test]
fn test_fourier_preserves_trace() {
    let input = DensityMatrix::new_zero_state(2);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    let tr = output.trace();
    assert!(
        (tr.0 - 1.0).abs() < 1e-10,
        "QFT should preserve trace, got ({}, {})",
        tr.0, tr.1
    );
}

// =============================================================================
// Diffuse kernel tests
// =============================================================================

#[test]
fn test_diffuse_on_uniform_is_identity() {
    // Diffusion on uniform superposition should keep it unchanged
    let input = DensityMatrix::new_uniform(2);
    let diffuse = Diffuse;
    let output = diffuse.apply(&input);

    let probs = output.diagonal_probabilities();
    for &p in &probs {
        assert!(
            (p - 0.25).abs() < 1e-10,
            "Diffuse on uniform should stay uniform, got p={}",
            p
        );
    }
}

#[test]
fn test_diffuse_preserves_purity() {
    let input = DensityMatrix::new_zero_state(2);
    let diffuse = Diffuse;
    let output = diffuse.apply(&input);

    assert!(
        (output.purity() - 1.0).abs() < 1e-10,
        "Diffuse should preserve purity, got {}",
        output.purity()
    );
}

// =============================================================================
// GroverIter kernel tests
// =============================================================================

#[test]
fn test_grover_2q_target3_exact() {
    // Key verification: 1 iteration on N=4, target=3 -> probability 1.0
    let input = DensityMatrix::new_uniform(2);
    let grover = GroverIter { target: 3 };
    let output = grover.apply(&input);

    let probs = output.diagonal_probabilities();
    assert!(
        (probs[3] - 1.0).abs() < 1e-10,
        "Grover 2q target=3: expected p[3]=1.0, got {}",
        probs[3]
    );
}

#[test]
fn test_grover_amplifies_target() {
    let input = DensityMatrix::new_uniform(2);
    let grover = GroverIter { target: 2 };
    let output = grover.apply(&input);

    let probs = output.diagonal_probabilities();
    assert!(
        probs[2] > probs[0],
        "Grover should amplify target. target_p={}, other_p={}",
        probs[2], probs[0]
    );
}

#[test]
fn test_grover_preserves_normalization() {
    let input = DensityMatrix::new_uniform(2);
    let grover = GroverIter { target: 1 };
    let output = grover.apply(&input);

    let tr = output.trace();
    assert!(
        (tr.0 - 1.0).abs() < 1e-10,
        "Grover output should have trace 1, got ({}, {})",
        tr.0, tr.1
    );
}

#[test]
fn test_grover_4q_3_iterations() {
    // 3 iterations on 16 states should give high probability for the target
    let mut dm = DensityMatrix::new_uniform(4);
    let grover = GroverIter { target: 7 };

    for _ in 0..3 {
        dm = grover.apply(&dm);
    }

    let probs = dm.diagonal_probabilities();
    assert!(
        probs[7] > 0.9,
        "After 3 Grover iterations on 4-qubit, p[7]={} should be > 0.9",
        probs[7]
    );
}

// =============================================================================
// Entangle kernel tests
// =============================================================================

#[test]
fn test_entangle_creates_bell() {
    // Start with |+>|0> = H|0> tensor |0>
    // Apply CNOT -> Bell state
    // First create |+> tensor |0> as a statevector
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    // |+>|0> = (1/sqrt(2))(|00> + |10>)
    let psi = vec![
        (inv_sqrt2, 0.0), // |00>
        (0.0, 0.0),       // |01>
        (inv_sqrt2, 0.0), // |10>
        (0.0, 0.0),       // |11>
    ];
    let input = DensityMatrix::from_statevector(&psi).unwrap();

    let entangle = Entangle;
    let output = entangle.apply(&input);

    // Should produce Bell state: rho[0][0] = rho[0][3] = rho[3][0] = rho[3][3] = 0.5
    assert!(
        (output.get(0, 0).0 - 0.5).abs() < 1e-10,
        "rho[0][0] should be 0.5, got {}",
        output.get(0, 0).0
    );
    assert!(
        (output.get(0, 3).0 - 0.5).abs() < 1e-10,
        "rho[0][3] should be 0.5, got {}",
        output.get(0, 3).0
    );
    assert!(
        (output.get(3, 0).0 - 0.5).abs() < 1e-10,
        "rho[3][0] should be 0.5, got {}",
        output.get(3, 0).0
    );
    assert!(
        (output.get(3, 3).0 - 0.5).abs() < 1e-10,
        "rho[3][3] should be 0.5, got {}",
        output.get(3, 3).0
    );
}
