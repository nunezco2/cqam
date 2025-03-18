use cqam_sim::qdist::QDist;
use cqam_sim::kernels::init::InitDist;
use cqam_sim::kernel::Kernel;

#[test]
fn test_init_dist_kernel() {
    let init = InitDist { domain: vec![0, 1, 2] };
    let dummy = QDist::new("dummy", vec![0], vec![1.0]);
    let output = init.apply(&dummy);
    assert_eq!(output.domain.len(), 3);
    assert!((output.probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-6);
}
