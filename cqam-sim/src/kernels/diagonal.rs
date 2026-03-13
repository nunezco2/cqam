//! Diagonal unitary kernel (kernel_id = 9).
//!
//! Applies an arbitrary diagonal unitary matrix whose entries are provided
//! as a vector of complex numbers read from CMEM. Each entry d_k multiplies
//! basis state |k>: the statevector transform is psi'[k] = d_k * psi[k],
//! and the density matrix transform is rho'[i][j] = d_i * rho[i][j] * conj(d_j).
//!
//! Unlike Rotate and PhaseShift, which compute diagonal entries from a
//! formula, this kernel reads entries from CMEM, allowing fully arbitrary
//! diagonal unitaries.

use cqam_core::error::CqamError;
use crate::complex::{C64, cx_mul};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use rayon::prelude::*;

use crate::constants::PAR_THRESHOLD;

/// Diagonal unitary kernel (kernel_id = 9).
///
/// Applies an arbitrary diagonal unitary matrix whose entries are
/// provided as a vector of complex numbers. Each entry d_k multiplies
/// basis state |k>: the statevector transform is psi'[k] = d_k * psi[k],
/// and the density matrix transform is rho'[i][j] = d_i * rho[i][j] * conj(d_j).
///
/// Unlike Rotate and PhaseShift, which compute diagonal entries from a
/// formula, this kernel reads entries from CMEM, allowing fully arbitrary
/// diagonal unitaries.
pub struct DiagonalUnitary {
    /// The diagonal entries d_0, d_1, ..., d_{dim-1}.
    /// Each entry should have unit modulus for the matrix to be unitary.
    /// Length must equal the dimension of the quantum register.
    pub diagonal: Vec<C64>,
}

impl Kernel for DiagonalUnitary {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let dim = input.dimension();
        if self.diagonal.len() != dim {
            return Err(CqamError::TypeMismatch {
                instruction: "QKERNEL/DIAGONAL_UNITARY".to_string(),
                detail: format!(
                    "diagonal has {} entries but register dimension is {}",
                    self.diagonal.len(), dim
                ),
            });
        }
        let mut result = input.clone();
        result.apply_diagonal_unitary(&self.diagonal);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, CqamError> {
        let dim = input.dimension();
        if self.diagonal.len() != dim {
            return Err(CqamError::TypeMismatch {
                instruction: "QKERNEL/DIAGONAL_UNITARY".to_string(),
                detail: format!(
                    "diagonal has {} entries but register dimension is {}",
                    self.diagonal.len(), dim
                ),
            });
        }
        let amps = input.amplitudes();
        let diag = &self.diagonal;
        let result_amps: Vec<C64> = if dim >= PAR_THRESHOLD {
            amps.par_iter().zip(diag.par_iter()).map(|(&a, &d)| {
                cx_mul(d, a)
            }).collect()
        } else {
            amps.iter().zip(diag.iter()).map(|(&a, &d)| {
                cx_mul(d, a)
            }).collect()
        };
        Ok(Statevector::from_amplitudes(result_amps)
            .expect("DiagonalUnitary apply_sv produced invalid amplitudes"))
    }
}
