// cqam-sim/src/kernels/fourier.rs
//
// Phase 2: Quantum Fourier Transform kernel on DensityMatrix.

use std::f64::consts::PI;
use crate::complex::{self, cx_scale, cx_exp_i};
use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;

/// Quantum Fourier Transform kernel (kernel_id = 2).
///
/// Constructs the QFT unitary:
///   QFT[j][k] = (1/sqrt(N)) * exp(2*pi*i*j*k/N)
pub struct Fourier;

impl Kernel for Fourier {
    fn apply(&self, input: &DensityMatrix) -> DensityMatrix {
        let dim = input.dimension();
        let n_f64 = dim as f64;
        let norm = 1.0 / n_f64.sqrt();

        // Construct QFT unitary
        let mut unitary = vec![complex::ZERO; dim * dim];
        for j in 0..dim {
            for k in 0..dim {
                let angle = 2.0 * PI * (j as f64) * (k as f64) / n_f64;
                let entry = cx_exp_i(angle);
                unitary[j * dim + k] = cx_scale(norm, entry);
            }
        }

        let mut result = input.clone();
        result.apply_unitary(&unitary);
        result
    }
}
