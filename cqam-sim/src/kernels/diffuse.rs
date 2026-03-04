// cqam-sim/src/kernels/diffuse.rs
//
// Phase 2: Grover's diffusion operator on DensityMatrix.

use crate::complex;
use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;

/// Grover's diffusion kernel (kernel_id = 3).
///
/// D = 2|s><s| - I, where |s> = (1/sqrt(N)) sum_k |k>.
/// Matrix entries: D[j][k] = 2/N - delta_{j,k}
pub struct Diffuse;

impl Kernel for Diffuse {
    fn apply(&self, input: &DensityMatrix) -> DensityMatrix {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        let mut unitary = vec![complex::ZERO; dim * dim];
        for j in 0..dim {
            for k in 0..dim {
                let val = 2.0 / n_f64 - if j == k { 1.0 } else { 0.0 };
                unitary[j * dim + k] = (val, 0.0);
            }
        }

        let mut result = input.clone();
        result.apply_unitary(&unitary);
        result
    }
}
