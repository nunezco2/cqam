use cqam_sim::qdist::{QDist, Measurable};

#[test]
fn test_measure_deterministic_returns_max_probability_value() {
    let domain = vec![0u16, 1, 2];
    let probabilities = vec![0.1, 0.7, 0.2];
    let qdist = QDist::new("qX", domain.clone(), probabilities).unwrap();

    let measured = qdist.measure_deterministic();
    assert_eq!(measured, Some(1u16));
}

#[test]
fn test_expected_value_computes_correctly() {
    let domain = vec![0u16, 1, 2];
    let probabilities = vec![0.1, 0.7, 0.2];
    let qdist = QDist::new("qX", domain, probabilities).unwrap();

    let expected = qdist.expected_value().unwrap();
    assert!((expected - 1.1).abs() < 1e-6);
}

#[test]
fn test_stochastic_measure_distribution() {
    // Create a distribution: state 0 has p=0.2, state 1 has p=0.8
    let qdist = QDist::new("qX", vec![0u16, 1], vec![0.2, 0.8]).unwrap();

    let num_samples = 10_000;
    let mut count_0 = 0usize;
    let mut count_1 = 0usize;

    for _ in 0..num_samples {
        match qdist.measure() {
            Some(0) => count_0 += 1,
            Some(1) => count_1 += 1,
            _ => panic!("Unexpected measurement result"),
        }
    }

    let freq_0 = count_0 as f64 / num_samples as f64;
    let freq_1 = count_1 as f64 / num_samples as f64;

    // Allow 5% tolerance for stochastic sampling
    assert!(
        (freq_0 - 0.2).abs() < 0.05,
        "Expected ~0.2 for state 0, got {}",
        freq_0
    );
    assert!(
        (freq_1 - 0.8).abs() < 0.05,
        "Expected ~0.8 for state 1, got {}",
        freq_1
    );
}

#[test]
fn test_stochastic_measure_delta_distribution() {
    // Delta distribution: all probability on state 5
    let qdist = QDist::new("qX", vec![5u16], vec![1.0]).unwrap();

    for _ in 0..100 {
        assert_eq!(qdist.measure(), Some(5u16));
    }
}

#[test]
fn test_measure_empty_distribution() {
    let qdist: QDist<u16> = QDist::new("empty", vec![], vec![]).unwrap();
    assert_eq!(qdist.measure(), None);
    assert_eq!(qdist.measure_deterministic(), None);
}

// =============================================================================
// Fidelity metric tests (Phase 6.4)
// =============================================================================

#[test]
fn test_superposition_metric_uniform() {
    // Uniform distribution should have maximum superposition (1.0)
    let qdist = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]).unwrap();
    let metric = qdist.superposition_metric();
    assert!(
        (metric - 1.0).abs() < 1e-6,
        "Uniform distribution should have superposition_metric ~1.0, got {}",
        metric
    );
}

#[test]
fn test_superposition_metric_delta() {
    // Delta distribution should have zero superposition
    let qdist = QDist::new("q", vec![0u16], vec![1.0]).unwrap();
    let metric = qdist.superposition_metric();
    assert!(
        metric.abs() < 1e-6,
        "Delta distribution should have superposition_metric ~0.0, got {}",
        metric
    );
}

#[test]
fn test_superposition_metric_partial() {
    // Two-state distribution with unequal probabilities
    let qdist = QDist::new("q", vec![0u16, 1], vec![0.9, 0.1]).unwrap();
    let metric = qdist.superposition_metric();
    // Should be between 0 and 1
    assert!(metric > 0.0 && metric < 1.0, "Partial superposition, got {}", metric);
}

#[test]
fn test_concentration_metric_uniform() {
    // Uniform: effective_states = n, so metric = n/n = 1.0
    let qdist = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.25, 0.25, 0.25, 0.25]).unwrap();
    let metric = qdist.concentration_metric();
    assert!(
        (metric - 1.0).abs() < 1e-6,
        "Uniform should have concentration_metric ~1.0, got {}",
        metric
    );
}

#[test]
fn test_concentration_metric_delta() {
    // Delta: effective_states = 1, metric = 1/1 = 1.0
    let qdist = QDist::new("q", vec![0u16], vec![1.0]).unwrap();
    let metric = qdist.concentration_metric();
    assert!(
        (metric - 1.0).abs() < 1e-6,
        "Delta with 1 state should have concentration_metric ~1.0, got {}",
        metric
    );
}

#[test]
fn test_concentration_metric_concentrated() {
    // Concentrated: one state with most probability
    let qdist = QDist::new("q", vec![0u16, 1, 2, 3], vec![0.97, 0.01, 0.01, 0.01]).unwrap();
    let metric = qdist.concentration_metric();
    // effective_states ~ 1.06, metric ~ 1.06/4 ~ 0.27
    assert!(
        metric < 0.5,
        "Concentrated distribution should have low concentration_metric, got {}",
        metric
    );
}
