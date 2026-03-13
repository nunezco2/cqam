//! Construction methods for `DensityMatrix`.

use super::DensityMatrix;
use crate::complex::{self, C64, cx_add, cx_mul, cx_conj, cx_scale, cx_norm_sq};
use crate::constants::MAX_QUBITS;
use cqam_core::error::CqamError;

// =============================================================================
// Construction
// =============================================================================

impl DensityMatrix {
    /// Construct from pre-computed matrix data.
    ///
    /// `data` must have length `(2^num_qubits)^2`. Returns Err if
    /// `num_qubits` exceeds MAX_QUBITS or data length is wrong.
    pub fn from_raw(num_qubits: u8, data: Vec<C64>) -> Result<Self, CqamError> {
        if num_qubits > MAX_QUBITS {
            return Err(CqamError::QubitLimitExceeded {
                instruction: "DensityMatrix::from_raw".to_string(),
                required: num_qubits,
                max: MAX_QUBITS,
            });
        }
        let dim = 1usize << num_qubits;
        if data.len() != dim * dim {
            return Err(CqamError::TypeMismatch {
                instruction: "DensityMatrix::from_raw".to_string(),
                detail: format!(
                    "data length {} != expected {}",
                    data.len(), dim * dim
                ),
            });
        }
        Ok(Self { num_qubits, data })
    }

    /// Create the computational zero state |0...0><0...0|.
    ///
    /// # Panics
    /// Panics if `num_qubits == 0` or `num_qubits > MAX_QUBITS`.
    pub fn new_zero_state(num_qubits: u8) -> Self {
        assert!((1..=MAX_QUBITS).contains(&num_qubits),
            "num_qubits must be 1..={}, got {}", MAX_QUBITS, num_qubits);
        let dim = 1usize << num_qubits;
        let mut data = vec![complex::ZERO; dim * dim];
        data[0] = complex::ONE; // rho[0][0] = 1
        Self { num_qubits, data }
    }

    /// Create the equal-superposition pure state H^n|0><0|(H^n)^dagger.
    ///
    /// Every entry of the density matrix is (1/dim, 0) where dim = 2^num_qubits.
    /// This corresponds to the n-qubit state produced by applying the Hadamard
    /// gate to every qubit of the |0...0> state. It is a pure state (rank 1)
    /// with purity Tr(rho^2) = 1.
    ///
    /// # Panics
    ///
    /// Panics if `num_qubits == 0` or `num_qubits > MAX_QUBITS`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cqam_sim::density_matrix::DensityMatrix;
    ///
    /// let dm = DensityMatrix::new_uniform(2); // 2-qubit uniform state
    /// assert_eq!(dm.num_qubits(), 2);
    /// assert_eq!(dm.dimension(), 4);
    ///
    /// // All diagonal elements equal 1/dim = 0.25
    /// let probs = dm.diagonal_probabilities();
    /// for p in &probs {
    ///     assert!((p - 0.25).abs() < 1e-12);
    /// }
    ///
    /// // Pure state: purity = 1
    /// assert!((dm.purity() - 1.0).abs() < 1e-10);
    /// ```
    pub fn new_uniform(num_qubits: u8) -> Self {
        assert!((1..=MAX_QUBITS).contains(&num_qubits),
            "num_qubits must be 1..={}, got {}", MAX_QUBITS, num_qubits);
        let dim = 1usize << num_qubits;
        let val = 1.0 / dim as f64;
        let data = vec![(val, 0.0); dim * dim];
        Self { num_qubits, data }
    }

    /// Create the Bell state |Phi+> = (|00> + |11>) / sqrt(2).
    ///
    /// Produces a 4x4 density matrix (2 qubits).
    pub fn new_bell() -> Self {
        let num_qubits = 2u8;
        let dim = 4usize;
        let mut data = vec![complex::ZERO; dim * dim];
        // rho[0][0] = rho[0][3] = rho[3][0] = rho[3][3] = 0.5
        //   row 0, col 0        row 0, col 3
        data[0]     = (0.5, 0.0);
        data[3]     = (0.5, 0.0);
        //   row 3, col 0        row 3, col 3
        data[3 * dim]     = (0.5, 0.0);
        data[3 * dim + 3] = (0.5, 0.0);
        Self { num_qubits, data }
    }

    /// Create the GHZ state (|0...0> + |1...1>) / sqrt(2) for n qubits.
    ///
    /// # Panics
    /// Panics if `num_qubits < 2` or `num_qubits > MAX_QUBITS`.
    pub fn new_ghz(num_qubits: u8) -> Self {
        assert!((2..=MAX_QUBITS).contains(&num_qubits),
            "num_qubits must be 2..={}, got {}", MAX_QUBITS, num_qubits);
        let dim = 1usize << num_qubits;
        let mut data = vec![complex::ZERO; dim * dim];
        //   row 0, col 0                row 0, col dim-1
        data[0]                     = (0.5, 0.0);
        data[dim - 1]               = (0.5, 0.0);
        //   row dim-1, col 0            row dim-1, col dim-1
        data[(dim - 1) * dim]       = (0.5, 0.0);
        data[(dim - 1) * dim + (dim - 1)] = (0.5, 0.0);
        Self { num_qubits, data }
    }

    /// Construct a mixed state from weighted pure states.
    ///
    /// rho = sum_i weights[i] * |psi_i><psi_i|
    ///
    /// All statevectors must have the same dimension. Weights should be
    /// non-negative and will be normalized to sum to 1.
    ///
    /// # Errors
    /// Returns Err if statevectors have mismatched dimensions or zero total weight.
    pub fn from_mixture(states: &[(f64, &[C64])]) -> Result<Self, CqamError> {
        if states.is_empty() {
            return Err(CqamError::TypeMismatch {
                instruction: "DensityMatrix::from_mixture".to_string(),
                detail: "empty state list".to_string(),
            });
        }

        let dim = states[0].1.len();
        if dim == 0 || (dim & (dim - 1)) != 0 {
            return Err(CqamError::TypeMismatch {
                instruction: "DensityMatrix::from_mixture".to_string(),
                detail: format!("dimension {} is not a power of 2", dim),
            });
        }
        let num_qubits = dim.trailing_zeros() as u8;
        if num_qubits > MAX_QUBITS {
            return Err(CqamError::QubitLimitExceeded {
                instruction: "DensityMatrix::from_mixture".to_string(),
                required: num_qubits,
                max: MAX_QUBITS,
            });
        }

        // Validate all dims match
        for (i, (_, psi)) in states.iter().enumerate() {
            if psi.len() != dim {
                return Err(CqamError::TypeMismatch {
                    instruction: "DensityMatrix::from_mixture".to_string(),
                    detail: format!(
                        "state {} has dimension {} but expected {}",
                        i, psi.len(), dim
                    ),
                });
            }
        }

        // Compute total weight
        let total_weight: f64 = states.iter().map(|(w, _)| *w).sum();
        if total_weight <= 0.0 {
            return Err(CqamError::TypeMismatch {
                instruction: "DensityMatrix::from_mixture".to_string(),
                detail: "total weight must be positive".to_string(),
            });
        }

        let mut data = vec![complex::ZERO; dim * dim];

        for (weight, psi) in states {
            let normalized_weight = weight / total_weight;

            // Normalize this statevector
            let norm_sq: f64 = psi.iter().map(|z| cx_norm_sq(*z)).sum();
            let norm = norm_sq.sqrt();
            let psi_norm: Vec<C64> = if (norm - 1.0).abs() > 1e-12 && norm > 1e-30 {
                psi.iter().map(|z| cx_scale(1.0 / norm, *z)).collect()
            } else {
                psi.to_vec()
            };

            // Accumulate: rho += w * |psi><psi|
            for i in 0..dim {
                for j in 0..dim {
                    let outer = cx_mul(psi_norm[i], cx_conj(psi_norm[j]));
                    let scaled = cx_scale(normalized_weight, outer);
                    data[i * dim + j] = cx_add(data[i * dim + j], scaled);
                }
            }
        }

        Ok(Self { num_qubits, data })
    }

    /// Construct a density matrix from a pure state vector: rho = |psi><psi|.
    ///
    /// The statevector length must be a power of 2 (2^n for some n >= 1).
    pub fn from_statevector(psi: &[C64]) -> Result<Self, CqamError> {
        let len = psi.len();
        if len == 0 || (len & (len - 1)) != 0 {
            return Err(CqamError::TypeMismatch {
                instruction: "DensityMatrix::from_statevector".to_string(),
                detail: format!("statevector length {} is not a power of 2", len),
            });
        }
        let num_qubits = len.trailing_zeros() as u8;
        if num_qubits > MAX_QUBITS {
            return Err(CqamError::QubitLimitExceeded {
                instruction: "DensityMatrix::from_statevector".to_string(),
                required: num_qubits,
                max: MAX_QUBITS,
            });
        }

        // Normalize the statevector
        let norm_sq: f64 = psi.iter().map(|z| cx_norm_sq(*z)).sum();
        let norm = norm_sq.sqrt();
        let psi_norm: Vec<C64> = if (norm - 1.0).abs() > 1e-12 {
            psi.iter().map(|z| cx_scale(1.0 / norm, *z)).collect()
        } else {
            psi.to_vec()
        };

        // rho[i][j] = psi[i] * conj(psi[j])
        let dim = len;
        let mut data = vec![complex::ZERO; dim * dim];
        for i in 0..dim {
            for j in 0..dim {
                data[i * dim + j] = cx_mul(psi_norm[i], cx_conj(psi_norm[j]));
            }
        }

        Ok(Self { num_qubits, data })
    }
}
