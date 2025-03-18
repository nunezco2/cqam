// cqam-sim/src/kernels/init.rs

use crate::qdist::QDist;
use crate::kernel::Kernel;

pub struct InitDist<T>
where
    T: Clone,
{
    pub domain: Vec<T>,
}

impl<T: Clone> Kernel<T> for InitDist<T> {
    fn apply(&self, _input: &QDist<T>) -> QDist<T> {
        let n = self.domain.len();
        let prob = 1.0 / n as f64;
        QDist::new("init", self.domain.clone(), vec![prob; n])
    }
}
