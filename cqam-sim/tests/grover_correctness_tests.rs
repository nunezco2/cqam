// cqam-sim/tests/grover_correctness_tests.rs
//
// Phase 9.2: Multi-iteration Grover correctness tests.

use cqam_sim::density_matrix::DensityMatrix;
use cqam_sim::kernel::Kernel;
use cqam_sim::kernels::grover::GroverIter;

/// Apply Grover iterations and return the probability of the target state.
fn grover_probability(num_qubits: u8, target: u16, iterations: usize) -> f64 {
    let mut rho = DensityMatrix::new_uniform(num_qubits);
    let kernel = GroverIter { target };
    for _ in 0..iterations {
        rho = kernel.apply(&rho);
    }
    rho.diagonal_probabilities()[target as usize]
}

#[test]
fn test_grover_2q_1iter_exact() {
    let prob = grover_probability(2, 2, 1);
    assert!(
        (prob - 1.0).abs() < 1e-10,
        "2-qubit, 1 iter: P(target=2) should be 1.0, got {}", prob
    );
}

#[test]
fn test_grover_3q_2iter_high_prob() {
    let prob = grover_probability(3, 5, 2);
    assert!(
        prob > 0.94,
        "3-qubit, 2 iters: P(target=5) should be > 0.94, got {}", prob
    );
}

#[test]
fn test_grover_4q_3iter_high_prob() {
    let prob = grover_probability(4, 11, 3);
    assert!(
        prob > 0.96,
        "4-qubit, 3 iters: P(target=11) should be > 0.96, got {}", prob
    );
}

#[test]
fn test_grover_preserves_trace_after_iterations() {
    let mut rho = DensityMatrix::new_uniform(3);
    let kernel = GroverIter { target: 5 };
    for _ in 0..2 {
        rho = kernel.apply(&rho);
    }
    let trace = rho.trace();
    assert!(
        (trace.0 - 1.0).abs() < 1e-10,
        "Trace real part should be 1.0, got {}", trace.0
    );
    assert!(
        trace.1.abs() < 1e-10,
        "Trace imaginary part should be 0.0, got {}", trace.1
    );
}

#[test]
fn test_grover_preserves_purity_after_iterations() {
    let mut rho = DensityMatrix::new_uniform(3);
    let kernel = GroverIter { target: 5 };
    for _ in 0..2 {
        rho = kernel.apply(&rho);
    }
    let purity = rho.purity();
    assert!(
        (purity - 1.0).abs() < 1e-10,
        "Purity should be 1.0, got {}", purity
    );
}

// ===========================================================================
// Phase 9 debugger: additional Grover edge cases
// ===========================================================================

#[test]
fn test_grover_2q_target_zero() {
    // Target = 0 (first basis state) should work identically.
    let prob = grover_probability(2, 0, 1);
    assert!(
        (prob - 1.0).abs() < 1e-10,
        "2-qubit, 1 iter, target=0: P should be 1.0, got {}", prob
    );
}

#[test]
fn test_grover_2q_target_max() {
    // Target = 3 (last basis state for 2 qubits).
    let prob = grover_probability(2, 3, 1);
    assert!(
        (prob - 1.0).abs() < 1e-10,
        "2-qubit, 1 iter, target=3: P should be 1.0, got {}", prob
    );
}

#[test]
fn test_grover_2q_overiteration_returns_to_uniform() {
    // After 3 iterations on 2 qubits, the target probability drops back to 1/N.
    // This tests the oscillatory nature of Grover's algorithm.
    let prob = grover_probability(2, 2, 3);
    assert!(
        (prob - 0.25).abs() < 1e-10,
        "2-qubit, 3 iters: P(target) should return to 0.25, got {}", prob
    );
}

#[test]
fn test_grover_3q_probabilities_sum_to_one() {
    // All diagonal probabilities must sum to 1.0 after any number of iterations.
    let mut rho = DensityMatrix::new_uniform(3);
    let kernel = GroverIter { target: 5 };
    for _ in 0..2 {
        rho = kernel.apply(&rho);
    }
    let probs = rho.diagonal_probabilities();
    let sum: f64 = probs.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-10,
        "Probabilities should sum to 1.0, got {}", sum
    );
}

#[test]
fn test_grover_non_target_states_equal() {
    // After Grover iterations, all non-target states should have equal probability
    // (symmetry of the algorithm).
    let mut rho = DensityMatrix::new_uniform(3);
    let kernel = GroverIter { target: 5 };
    for _ in 0..2 {
        rho = kernel.apply(&rho);
    }
    let probs = rho.diagonal_probabilities();
    let non_target: Vec<f64> = probs.iter().enumerate()
        .filter(|&(i, _)| i != 5)
        .map(|(_, &p)| p)
        .collect();
    let first = non_target[0];
    for (i, &p) in non_target.iter().enumerate() {
        assert!(
            (p - first).abs() < 1e-10,
            "Non-target state {} has probability {} != {} (first non-target)",
            i, p, first
        );
    }
}
