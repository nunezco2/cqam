//! Grover's diffusion operator kernel (amplitude amplification step).

use cqam_core::error::CqamError;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use rayon::prelude::*;

use crate::constants::PAR_THRESHOLD;

/// Grover's diffusion kernel (kernel_id = 3).
///
/// D = 2|s><s| - I, where |s> = (1/sqrt(N)) sum_k |k>.
/// Matrix entries: D[j][k] = 2/N - delta_{j,k}
pub struct Diffuse;

impl Kernel for Diffuse {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        let mut unitary = vec![C64::ZERO; dim * dim];
        for j in 0..dim {
            for k in 0..dim {
                let val = 2.0 / n_f64 - if j == k { 1.0 } else { 0.0 };
                unitary[j * dim + k] = C64(val, 0.0);
            }
        }

        let mut result = input.clone();
        result.apply_unitary(&unitary);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, CqamError> {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        // D|psi> = 2|s><s|psi> - |psi>
        // <s|psi> = (1/sqrt(N)) sum_k psi_k
        let amps = input.amplitudes();
        let mean = if dim >= PAR_THRESHOLD {
            let sum = amps.par_iter().copied().reduce(|| C64::ZERO, |a, b| a + b);
            sum.scale(1.0 / n_f64)
        } else {
            let mut m = C64::ZERO;
            for amp in amps.iter().take(dim) {
                m += *amp;
            }
            m.scale(1.0 / n_f64)
        };

        // D|psi>_j = 2*mean - psi_j
        let two_mean = mean.scale(2.0);
        let result_amps: Vec<C64> = if dim >= PAR_THRESHOLD {
            amps.par_iter().map(|amp| {
                two_mean - *amp
            }).collect()
        } else {
            amps.iter().take(dim).map(|amp| {
                two_mean - *amp
            }).collect()
        };

        Ok(Statevector::from_amplitudes(result_amps)
            .expect("Diffuse apply_sv produced invalid amplitudes"))
    }
}
