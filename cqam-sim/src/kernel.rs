// cqam-sim/src/kernel.rs

use crate::qdist::QDist;

pub trait Kernel<T> {
    fn apply(&self, input: &QDist<T>) -> QDist<T>;
}
