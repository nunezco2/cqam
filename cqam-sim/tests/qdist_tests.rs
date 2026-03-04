use cqam_sim::qdist::QDist;

#[test]
fn test_qdist_normalization() {
    let mut dist = QDist::new("qX", vec![0u16, 1], vec![2.0, 2.0]).unwrap();
    dist.normalize();
    assert!((dist.probabilities[0] - 0.5).abs() < 1e-6);
}

#[test]
fn test_qdist_new_mismatched_sizes_returns_error() {
    let result = QDist::<u16>::new("x", vec![0], vec![]);
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("domain length"));
}

#[test]
fn test_measure_deterministic_with_nan_does_not_panic() {
    use cqam_sim::qdist::Measurable;
    let qdist = QDist::new("q", vec![0u16, 1], vec![f64::NAN, 1.0]).unwrap();
    let result = qdist.measure_deterministic();
    assert!(result.is_some());
}
