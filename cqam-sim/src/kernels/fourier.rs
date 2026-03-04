// cqam-sim/src/kernels/fourier.rs
//
// Phase 6.6: Quantum Fourier Transform approximation on probability distributions.
//
// Since the ensemble model uses real probabilities (not complex amplitudes),
// we treat sqrt(p) as pseudo-amplitudes, apply a DFT-like transformation,
// then square back to get probabilities.
//
// p_new[k] = |sum_j sqrt(p[j]) * exp(2*pi*i*j*k/N)|^2 / N

use std::f64::consts::PI;
use crate::qdist::QDist;
use crate::kernel::Kernel;

/// Quantum Fourier Transform kernel (kernel_id = 2).
///
/// Applies a DFT-like transformation to the probability distribution.
/// Treats sqrt(p) as pseudo-amplitudes, computes the DFT, then squares
/// the magnitudes to produce the output probability distribution.
pub struct Fourier;

impl Kernel<u16> for Fourier {
    fn apply(&self, input: &QDist<u16>) -> QDist<u16> {
        let n = input.probabilities.len();
        if n == 0 {
            return input.clone();
        }

        // Compute pseudo-amplitudes: sqrt(p[j])
        let amplitudes: Vec<f64> = input.probabilities.iter()
            .map(|&p| p.sqrt())
            .collect();

        // Apply DFT: for each output index k, compute
        // c_k = sum_j amplitude[j] * exp(2*pi*i*j*k/N)
        // p_new[k] = |c_k|^2 / N
        let mut new_probs = vec![0.0; n];
        let n_f64 = n as f64;

        for (k, prob_k) in new_probs.iter_mut().enumerate() {
            let mut re = 0.0;
            let mut im = 0.0;

            for (j, &amp_j) in amplitudes.iter().enumerate() {
                let angle = 2.0 * PI * (j as f64) * (k as f64) / n_f64;
                re += amp_j * angle.cos();
                im += amp_j * angle.sin();
            }

            // |c_k|^2 / N
            *prob_k = (re * re + im * im) / n_f64;
        }

        let mut result = QDist::new(&input.label, input.domain.clone(), new_probs);
        result.normalize();
        result
    }
}
