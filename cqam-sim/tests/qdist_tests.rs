use cqam_sim::qdist::QDist;

#[test]
fn test_qdist_normalization() {
    let mut dist = QDist::new("qX", vec![0, 1], vec![2.0, 2.0]);
    dist.normalize();
    assert!((dist.probabilities[0] - 0.5).abs() < 1e-6);
}
