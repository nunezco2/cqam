// cqam-sim/src/kernels/diffuse.rs
//
// Phase 6.6: Grover's diffusion operator (inversion about the mean).
//
// In the ensemble model, we work with pseudo-amplitudes (sqrt(p)).
// The diffusion operator inverts each amplitude about the mean of all amplitudes:
//   a'[i] = 2*mean - a[i]
// Then we square back to get probabilities.

use crate::qdist::QDist;
use crate::kernel::Kernel;

/// Grover's diffusion kernel (kernel_id = 3).
///
/// Applies the inversion-about-the-mean operation to pseudo-amplitudes.
/// This is one half of a Grover iteration (the other half is the oracle).
pub struct Diffuse;

impl Kernel<u16> for Diffuse {
    fn apply(&self, input: &QDist<u16>) -> QDist<u16> {
        let n = input.probabilities.len();
        if n == 0 {
            return input.clone();
        }

        // Compute pseudo-amplitudes: sqrt(p[i])
        let amplitudes: Vec<f64> = input.probabilities.iter()
            .map(|&p| p.sqrt())
            .collect();

        // Compute mean of amplitudes
        let mean: f64 = amplitudes.iter().sum::<f64>() / n as f64;

        // Inversion about the mean: a'[i] = 2*mean - a[i]
        let new_amplitudes: Vec<f64> = amplitudes.iter()
            .map(|&a| 2.0 * mean - a)
            .collect();

        // Square to get probabilities
        let new_probs: Vec<f64> = new_amplitudes.iter()
            .map(|&a| a * a)
            .collect();

        let mut result = QDist::new(&input.label, input.domain.clone(), new_probs)
            .expect("internal: domain/probability length mismatch");
        result.normalize();
        result
    }
}
