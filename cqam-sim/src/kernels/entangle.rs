// cqam-sim/src/kernels/entangle.rs

use crate::qdist::QDist;
use crate::kernel::Kernel;

pub struct Entangle {
    pub strength: f64,
}

impl<T: Clone> Kernel<T> for Entangle {
    fn apply(&self, input: &QDist<T>) -> QDist<T> {
        // Fake entanglement effect: amplify first state's probability
        let mut output = input.clone();
        if !output.probabilities.is_empty() {
            output.probabilities[0] += self.strength;
            output.normalize();
        }
        output
    }
}