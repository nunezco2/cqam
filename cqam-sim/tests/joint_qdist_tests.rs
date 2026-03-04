use cqam_sim::qdist::QDist;
use cqam_sim::joint_qdist::JointQDist;

#[test]
fn test_joint_qdist_from_independent() {
    let a = QDist::new("A", vec![0u16, 1], vec![0.5, 0.5]);
    let b = QDist::new("B", vec![0u16, 1], vec![0.5, 0.5]);

    let joint = JointQDist::from_independent(&a, &b);

    assert_eq!(joint.domain.len(), 4);
    // All four combinations should have equal probability 0.25
    for &p in &joint.probabilities {
        assert!(
            (p - 0.25).abs() < 1e-6,
            "Independent uniform joint should have p=0.25, got {}",
            p
        );
    }
}

#[test]
fn test_joint_qdist_normalization() {
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![(0, 0), (0, 1), (1, 0), (1, 1)],
        vec![2.0, 2.0, 2.0, 2.0],
    );

    let total: f64 = joint.probabilities.iter().sum();
    assert!(
        (total - 1.0).abs() < 1e-6,
        "JointQDist should normalize, sum = {}",
        total
    );
}

#[test]
fn test_marginal_a() {
    // Bell-like: only |00> and |11> with equal probability
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![(0, 0), (1, 1)],
        vec![0.5, 0.5],
    );

    let marginal = joint.marginal_a();
    assert_eq!(marginal.domain, vec![0, 1]);
    assert!((marginal.probabilities[0] - 0.5).abs() < 1e-6);
    assert!((marginal.probabilities[1] - 0.5).abs() < 1e-6);
}

#[test]
fn test_marginal_b() {
    // Bell-like: only |00> and |11> with equal probability
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![(0, 0), (1, 1)],
        vec![0.5, 0.5],
    );

    let marginal = joint.marginal_b();
    assert_eq!(marginal.domain, vec![0, 1]);
    assert!((marginal.probabilities[0] - 0.5).abs() < 1e-6);
    assert!((marginal.probabilities[1] - 0.5).abs() < 1e-6);
}

#[test]
fn test_conditional_b_given_a_bell_state() {
    // Bell state: |00> + |11> with equal probability
    // If A=0, then B must be 0. If A=1, then B must be 1.
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![(0, 0), (1, 1)],
        vec![0.5, 0.5],
    );

    let cond_b_given_a0 = joint.conditional_b_given_a(0);
    assert_eq!(cond_b_given_a0.domain, vec![0]);
    assert!((cond_b_given_a0.probabilities[0] - 1.0).abs() < 1e-6);

    let cond_b_given_a1 = joint.conditional_b_given_a(1);
    assert_eq!(cond_b_given_a1.domain, vec![1]);
    assert!((cond_b_given_a1.probabilities[0] - 1.0).abs() < 1e-6);
}

#[test]
fn test_conditional_a_given_b_bell_state() {
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![(0, 0), (1, 1)],
        vec![0.5, 0.5],
    );

    let cond_a_given_b0 = joint.conditional_a_given_b(0);
    assert_eq!(cond_a_given_b0.domain, vec![0]);
    assert!((cond_a_given_b0.probabilities[0] - 1.0).abs() < 1e-6);
}

#[test]
fn test_joint_measure_deterministic() {
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![(0, 0), (0, 1), (1, 0), (1, 1)],
        vec![0.1, 0.2, 0.3, 0.4],
    );

    let result = joint.measure_deterministic();
    assert_eq!(result, Some((1, 1))); // highest probability is 0.4
}

#[test]
fn test_joint_measure_stochastic_distribution() {
    // Test that stochastic measurement roughly matches the distribution
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![(0, 0), (1, 1)],
        vec![0.3, 0.7],
    );

    let num_samples = 5_000;
    let mut count_00 = 0usize;
    let mut count_11 = 0usize;

    for _ in 0..num_samples {
        match joint.measure() {
            Some((0, 0)) => count_00 += 1,
            Some((1, 1)) => count_11 += 1,
            _ => panic!("Unexpected joint measurement result"),
        }
    }

    let freq_00 = count_00 as f64 / num_samples as f64;
    let freq_11 = count_11 as f64 / num_samples as f64;

    assert!(
        (freq_00 - 0.3).abs() < 0.05,
        "Expected ~0.3 for (0,0), got {}",
        freq_00
    );
    assert!(
        (freq_11 - 0.7).abs() < 0.05,
        "Expected ~0.7 for (1,1), got {}",
        freq_11
    );
}

#[test]
fn test_marginal_asymmetric() {
    // Joint distribution where A and B have different marginals
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![(0, 0), (0, 1), (1, 0), (1, 1)],
        vec![0.1, 0.4, 0.2, 0.3],
    );

    let marginal_a = joint.marginal_a();
    // P(A=0) = 0.1 + 0.4 = 0.5, P(A=1) = 0.2 + 0.3 = 0.5
    assert!((marginal_a.probabilities[0] - 0.5).abs() < 1e-6);
    assert!((marginal_a.probabilities[1] - 0.5).abs() < 1e-6);

    let marginal_b = joint.marginal_b();
    // P(B=0) = 0.1 + 0.2 = 0.3, P(B=1) = 0.4 + 0.3 = 0.7
    assert!((marginal_b.probabilities[0] - 0.3).abs() < 1e-6);
    assert!((marginal_b.probabilities[1] - 0.7).abs() < 1e-6);
}

#[test]
fn test_measure_empty_joint() {
    let joint = JointQDist::new(
        ("A".to_string(), "B".to_string()),
        vec![],
        vec![],
    );

    assert_eq!(joint.measure(), None);
    assert_eq!(joint.measure_deterministic(), None);
}
