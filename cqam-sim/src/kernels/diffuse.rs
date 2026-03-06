//! Grover's diffusion operator kernel (amplitude amplification step).

use cqam_core::error::CqamError;
use crate::complex::{self, C64, cx_add, cx_scale};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;

/// Grover's diffusion kernel (kernel_id = 3).
///
/// D = 2|s><s| - I, where |s> = (1/sqrt(N)) sum_k |k>.
/// Matrix entries: D[j][k] = 2/N - delta_{j,k}
pub struct Diffuse;

impl Kernel for Diffuse {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
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
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, String> {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        // D|psi> = 2|s><s|psi> - |psi>
        // <s|psi> = (1/sqrt(N)) sum_k psi_k
        let amps = input.amplitudes();
        let mut mean = complex::ZERO;
        for amp in amps.iter().take(dim) {
            mean = cx_add(mean, *amp);
        }
        mean = cx_scale(1.0 / n_f64, mean); // <s|psi>/sqrt(N) * sqrt(N) / N = mean

        // Actually: <s|psi> = (1/sqrt(N)) sum_k psi_k
        // 2|s><s|psi> = 2 * (1/sqrt(N)) * <s|psi> for each component = 2 * mean
        // D|psi>_j = 2*mean - psi_j
        let mut result_amps: Vec<C64> = Vec::with_capacity(dim);
        for amp in amps.iter().take(dim) {
            let two_mean = cx_scale(2.0, mean);
            let val = (two_mean.0 - amp.0, two_mean.1 - amp.1);
            result_amps.push(val);
        }

        Ok(Statevector::from_amplitudes(result_amps)
            .expect("Diffuse apply_sv produced invalid amplitudes"))
    }
}
