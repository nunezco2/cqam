use cqam_sim::qdist::QDist;
use cqam_sim::kernels::init::InitDist;
use cqam_sim::kernels::fourier::Fourier;
use cqam_sim::kernels::diffuse::Diffuse;
use cqam_sim::kernels::grover::GroverIter;
use cqam_sim::kernel::Kernel;

#[test]
fn test_init_dist_kernel() {
    let init = InitDist { domain: vec![0u16, 1, 2] };
    let dummy = QDist::new("dummy", vec![0u16], vec![1.0]);
    let output = init.apply(&dummy);
    assert_eq!(output.domain.len(), 3);
    assert!((output.probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-6);
}

// =============================================================================
// Fourier kernel tests
// =============================================================================

#[test]
fn test_fourier_preserves_normalization() {
    let input = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    let total: f64 = output.probabilities.iter().sum();
    assert!(
        (total - 1.0).abs() < 1e-6,
        "Fourier output should be normalized, sum = {}",
        total
    );
    assert_eq!(output.domain.len(), 4);
}

#[test]
fn test_fourier_on_uniform_concentrates() {
    // QFT of uniform distribution should concentrate probability on state 0
    // because uniform amplitudes (all equal) DFT to a spike at k=0.
    let input = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    // The first state should have the highest probability
    assert!(
        output.probabilities[0] > 0.9,
        "QFT of uniform should concentrate on state 0, got p[0]={}",
        output.probabilities[0]
    );
}

#[test]
fn test_fourier_on_delta() {
    // QFT of delta at |0> should produce uniform distribution
    let input = QDist::new("q", vec![0u16, 1, 2, 3], vec![1.0, 0.0, 0.0, 0.0]);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    // All states should have approximately equal probability
    for p in &output.probabilities {
        assert!(
            (*p - 0.25).abs() < 1e-6,
            "QFT of delta should be uniform, got p={}",
            p
        );
    }
}

#[test]
fn test_fourier_empty() {
    let input = QDist::new("q", vec![], vec![]);
    let fourier = Fourier;
    let output = fourier.apply(&input);
    assert!(output.domain.is_empty());
}

// =============================================================================
// Diffuse kernel tests
// =============================================================================

#[test]
fn test_diffuse_preserves_normalization() {
    let input = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.1, 0.2, 0.3, 0.4]);
    let diffuse = Diffuse;
    let output = diffuse.apply(&input);

    let total: f64 = output.probabilities.iter().sum();
    assert!(
        (total - 1.0).abs() < 1e-6,
        "Diffuse output should be normalized, sum = {}",
        total
    );
}

#[test]
fn test_diffuse_on_uniform_stays_uniform() {
    // Diffusion on a uniform distribution should keep it uniform
    // because all amplitudes equal the mean.
    let input = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]);
    let diffuse = Diffuse;
    let output = diffuse.apply(&input);

    for p in &output.probabilities {
        assert!(
            (*p - 0.25).abs() < 1e-6,
            "Diffuse on uniform should stay uniform, got p={}",
            p
        );
    }
}

#[test]
fn test_diffuse_empty() {
    let input = QDist::new("q", vec![], vec![]);
    let diffuse = Diffuse;
    let output = diffuse.apply(&input);
    assert!(output.domain.is_empty());
}

// =============================================================================
// GroverIter kernel tests
// =============================================================================

#[test]
fn test_grover_iter_amplifies_target() {
    // Start with uniform distribution over 4 states
    let input = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]);

    // One Grover iteration targeting state 2
    let grover = GroverIter { target: 2 };
    let output = grover.apply(&input);

    // After one iteration, the target state should have higher probability
    let target_prob = output.probabilities[2]; // state 2 is at index 2
    let other_prob = output.probabilities[0];   // state 0 is at index 0

    assert!(
        target_prob > other_prob,
        "Grover iteration should amplify target state. target_p={}, other_p={}",
        target_prob, other_prob
    );
}

#[test]
fn test_grover_iter_preserves_normalization() {
    let input = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]);
    let grover = GroverIter { target: 1 };
    let output = grover.apply(&input);

    let total: f64 = output.probabilities.iter().sum();
    assert!(
        (total - 1.0).abs() < 1e-6,
        "Grover output should be normalized, sum = {}",
        total
    );
}

#[test]
fn test_grover_multiple_iterations_converge() {
    // Multiple Grover iterations should increase the probability of the target
    let mut dist = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]);
    let grover = GroverIter { target: 3 };

    // For N=4 states, optimal number of iterations is ~pi/4 * sqrt(4) ~ 1.57
    // So one iteration should already significantly boost the target.
    dist = grover.apply(&dist);

    let target_prob = dist.probabilities[3]; // state 3 is the target
    assert!(
        target_prob > 0.5,
        "After Grover iteration on 4 states, target should have p > 0.5, got {}",
        target_prob
    );
}

#[test]
fn test_grover_iter_empty() {
    let input = QDist::new("q", vec![], vec![]);
    let grover = GroverIter { target: 0 };
    let output = grover.apply(&input);
    assert!(output.domain.is_empty());
}

#[test]
fn test_grover_iter_target_not_in_domain() {
    // If the target state isn't in the domain, the oracle has no effect.
    // The diffusion still applies but no amplitude is flipped.
    let input = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]);
    let grover = GroverIter { target: 99 }; // not in domain
    let output = grover.apply(&input);

    // Without a target flip, diffusion on uniform stays uniform
    for p in &output.probabilities {
        assert!(
            (*p - 0.25).abs() < 1e-6,
            "No-target Grover on uniform should stay uniform, got p={}",
            p
        );
    }
}
