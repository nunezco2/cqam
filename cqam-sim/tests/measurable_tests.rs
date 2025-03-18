use cqam_sim::qdist::{QDist, Measurable};

#[test]
fn test_measure_returns_max_probability_value() {
    let domain = vec![0, 1, 2];
    let probabilities = vec![0.1, 0.7, 0.2];
    let qdist = QDist::new("qX", domain.clone(), probabilities);

    let measured = qdist.measure();
    assert_eq!(measured, Some(1));
}

#[test]
fn test_expected_value_computes_correctly() {
    let domain = vec![0, 1, 2];
    let probabilities = vec![0.1, 0.7, 0.2];
    let qdist = QDist::new("qX", domain, probabilities);

    let expected = qdist.expected_value().unwrap();
    assert!((expected - 1.1).abs() < 1e-6);
}
