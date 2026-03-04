// cqam-sim/src/kernels/grover.rs
//
// Phase 6.6: One Grover iteration = oracle + diffusion.
//
// In the ensemble model:
// 1. Oracle: flip the sign of the pseudo-amplitude for the target (marked) state.
//    Since we're working with sqrt(p) as pseudo-amplitudes, flipping the sign
//    of the target state's amplitude is equivalent to negating it.
// 2. Diffusion: inversion about the mean of all amplitudes.
//
// After both steps, square back to get probabilities.

use crate::qdist::QDist;
use crate::kernel::Kernel;

/// One Grover iteration kernel (kernel_id = 4).
///
/// Combines the oracle (marking the target state by flipping its amplitude sign)
/// with the diffusion operator (inversion about the mean).
///
/// The `target` field specifies which basis state is the marked state.
/// This is typically read from the integer register file via the ctx0 parameter
/// of the QKernel instruction.
pub struct GroverIter {
    /// The marked (target) state whose amplitude sign is flipped by the oracle.
    pub target: u16,
}

impl Kernel<u16> for GroverIter {
    fn apply(&self, input: &QDist<u16>) -> QDist<u16> {
        let n = input.probabilities.len();
        if n == 0 {
            return input.clone();
        }

        // Step 1: Compute pseudo-amplitudes
        let mut amplitudes: Vec<f64> = input.probabilities.iter()
            .map(|&p| p.sqrt())
            .collect();

        // Step 2: Oracle - flip the sign of the target state's amplitude
        for (i, &state) in input.domain.iter().enumerate() {
            if state == self.target {
                amplitudes[i] = -amplitudes[i];
            }
        }

        // Step 3: Diffusion - inversion about the mean
        let mean: f64 = amplitudes.iter().sum::<f64>() / n as f64;
        let new_amplitudes: Vec<f64> = amplitudes.iter()
            .map(|&a| 2.0 * mean - a)
            .collect();

        // Step 4: Square to get probabilities
        let new_probs: Vec<f64> = new_amplitudes.iter()
            .map(|&a| a * a)
            .collect();

        let mut result = QDist::new(&input.label, input.domain.clone(), new_probs)
            .expect("internal: domain/probability length mismatch");
        result.normalize();
        result
    }
}
