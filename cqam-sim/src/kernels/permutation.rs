//! Permutation kernel (kernel_id = 10).
//!
//! Applies a basis-state permutation |k> -> |sigma(k)> to a quantum register.
//! The permutation table is read from CMEM at dispatch time. Each entry
//! sigma(k) is a plain integer in 0..dim, stored via ISTR/ISTRX.
//!
//! Permutation matrices are structured-sparse unitaries with exactly one entry
//! of 1 in each row and column. Together with DIAGONAL_UNITARY (kernel_id = 9),
//! they form a composable basis for expressing arbitrary unitaries.

use cqam_core::error::CqamError;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;
use crate::statevector::Statevector;
use rayon::prelude::*;

use crate::constants::PAR_THRESHOLD;

/// Permutation kernel (kernel_id = 10).
///
/// Applies a basis-state permutation: |k> -> |sigma(k)> for all k.
/// The permutation is specified by a table where `table[k] = sigma(k)`.
/// An inverse table is precomputed at construction time to enable
/// cache-friendly parallel gather patterns in `apply_sv` and `apply`.
pub struct Permutation {
    /// The permutation table: table[k] = sigma(k).
    /// Length must equal the dimension of the quantum register.
    /// Must be a valid permutation: each value in 0..dim appears exactly once.
    pub table: Vec<usize>,
    /// Precomputed inverse: inverse[sigma(k)] = k.
    /// Enables cache-friendly parallel gather in apply_sv and apply.
    inverse: Vec<usize>,
}

impl Permutation {
    /// Construct a new permutation kernel from a permutation table.
    ///
    /// Validates that the table is a valid permutation (all entries in 0..dim,
    /// each appearing exactly once) and precomputes the inverse permutation.
    ///
    /// # Errors
    ///
    /// Returns `Err` if any entry is out of range or appears more than once.
    pub fn new(table: Vec<usize>) -> Result<Self, CqamError> {
        let dim = table.len();
        // Validation: each value must be in 0..dim, each appears exactly once
        let mut seen = vec![false; dim];
        for (k, &sigma_k) in table.iter().enumerate() {
            if sigma_k >= dim {
                return Err(CqamError::TypeMismatch {
                    instruction: "QKERNEL/PERMUTATION".to_string(),
                    detail: format!(
                        "permutation entry {}={} out of range 0..{}", k, sigma_k, dim
                    ),
                });
            }
            if seen[sigma_k] {
                return Err(CqamError::TypeMismatch {
                    instruction: "QKERNEL/PERMUTATION".to_string(),
                    detail: format!(
                        "permutation entry {} appears more than once (duplicate at index {})",
                        sigma_k, k
                    ),
                });
            }
            seen[sigma_k] = true;
        }
        // Build inverse: inverse[sigma(k)] = k
        let mut inverse = vec![0usize; dim];
        for (k, &sigma_k) in table.iter().enumerate() {
            inverse[sigma_k] = k;
        }
        Ok(Self { table, inverse })
    }
}

impl Kernel for Permutation {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let dim = input.dimension();
        if self.table.len() != dim {
            return Err(CqamError::TypeMismatch {
                instruction: "QKERNEL/PERMUTATION".to_string(),
                detail: format!(
                    "permutation has {} entries but register dimension is {}",
                    self.table.len(), dim
                ),
            });
        }

        // Gather pattern: result[i][j] = input[inverse[i]][inverse[j]]
        let mut result = input.clone();
        if dim >= PAR_THRESHOLD {
            // Parallel: collect row data then write
            let inv = &self.inverse;
            let new_data: Vec<C64> = (0..dim).into_par_iter().flat_map(|i| {
                let src_i = inv[i];
                (0..dim).map(move |j| {
                    let src_j = inv[j];
                    input.get(src_i, src_j)
                }).collect::<Vec<_>>()
            }).collect();
            for i in 0..dim {
                for j in 0..dim {
                    result.set(i, j, new_data[i * dim + j]);
                }
            }
        } else {
            for i in 0..dim {
                let src_i = self.inverse[i];
                for j in 0..dim {
                    let src_j = self.inverse[j];
                    result.set(i, j, input.get(src_i, src_j));
                }
            }
        }
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, CqamError> {
        let dim = input.dimension();
        if self.table.len() != dim {
            return Err(CqamError::TypeMismatch {
                instruction: "QKERNEL/PERMUTATION".to_string(),
                detail: format!(
                    "permutation has {} entries but register dimension is {}",
                    self.table.len(), dim
                ),
            });
        }

        let amps = input.amplitudes();
        let inv = &self.inverse;

        // Gather pattern: new_amps[j] = amps[inverse[j]]
        let new_amps: Vec<C64> = if dim >= PAR_THRESHOLD {
            (0..dim).into_par_iter()
                .map(|j| amps[inv[j]])
                .collect()
        } else {
            (0..dim).map(|j| amps[inv[j]]).collect()
        };

        Statevector::from_amplitudes(new_amps)
    }
}
