//! Density matrix representation for n-qubit quantum states.
//!
//! The density matrix rho is a 2^n x 2^n Hermitian, positive semi-definite
//! matrix with Tr(rho) = 1. It is stored as a flat row-major `Vec<C64>`,
//! where dim = 2^n. Supports construction of standard states (zero, uniform,
//! Bell, GHZ), unitary evolution, measurement, and fidelity metrics.

use crate::complex::{self, C64, cx_add, cx_mul, cx_conj, cx_scale, cx_norm_sq};
use cqam_core::quantum_state::QuantumState;
use rand::Rng;
use rayon::prelude::*;

/// Minimum dimension to use parallel iteration. Below this, sequential is faster.
const PAR_THRESHOLD: usize = 256;

/// Maximum number of qubits supported by the full density matrix.
pub const MAX_QUBITS: u8 = 16;

/// A density matrix representing an n-qubit quantum state.
///
/// Invariants (maintained by all public constructors and operations):
/// - `data.len() == dim * dim` where `dim = 2^num_qubits`
/// - `Tr(rho) = 1.0` (within floating-point tolerance)
/// - `rho` is Hermitian: `rho[i][j] = conj(rho[j][i])`
/// - `rho` is positive semi-definite
#[derive(Debug, Clone)]
pub struct DensityMatrix {
    num_qubits: u8,
    data: Vec<C64>,
}

// =============================================================================
// Construction
// =============================================================================

impl DensityMatrix {
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
    pub fn from_mixture(states: &[(f64, &[C64])]) -> Result<Self, String> {
        if states.is_empty() {
            return Err("from_mixture: empty state list".to_string());
        }

        let dim = states[0].1.len();
        if dim == 0 || (dim & (dim - 1)) != 0 {
            return Err(format!("from_mixture: dimension {} is not a power of 2", dim));
        }
        let num_qubits = dim.trailing_zeros() as u8;
        if num_qubits > MAX_QUBITS {
            return Err(format!("from_mixture: {} qubits exceeds MAX_QUBITS", num_qubits));
        }

        // Validate all dims match
        for (i, (_, psi)) in states.iter().enumerate() {
            if psi.len() != dim {
                return Err(format!(
                    "from_mixture: state {} has dimension {} but expected {}",
                    i, psi.len(), dim
                ));
            }
        }

        // Compute total weight
        let total_weight: f64 = states.iter().map(|(w, _)| *w).sum();
        if total_weight <= 0.0 {
            return Err("from_mixture: total weight must be positive".to_string());
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
    pub fn from_statevector(psi: &[C64]) -> Result<Self, String> {
        let len = psi.len();
        if len == 0 || (len & (len - 1)) != 0 {
            return Err(format!("Statevector length {} is not a power of 2", len));
        }
        let num_qubits = len.trailing_zeros() as u8;
        if num_qubits > MAX_QUBITS {
            return Err(format!("Statevector implies {} qubits, max is {}", num_qubits, MAX_QUBITS));
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

// =============================================================================
// Element Access
// =============================================================================

impl DensityMatrix {
    /// Get the element at row i, column j.
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> C64 {
        let dim = self.dimension();
        self.data[i * dim + j]
    }

    /// Set the element at row i, column j.
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, val: C64) {
        let dim = self.dimension();
        self.data[i * dim + j] = val;
    }

    /// Number of qubits.
    #[inline]
    pub fn num_qubits(&self) -> u8 {
        self.num_qubits
    }

    /// Hilbert space dimension: 2^num_qubits.
    #[inline]
    pub fn dimension(&self) -> usize {
        1 << self.num_qubits
    }
}

// =============================================================================
// Unitary Application
// =============================================================================

impl DensityMatrix {
    /// Apply a unitary transformation in-place: rho <- U * rho * U^dagger.
    ///
    /// The `unitary` slice must contain dim*dim elements in row-major order.
    ///
    /// # Panics
    /// Panics if `unitary.len() != dim * dim`.
    pub fn apply_unitary(&mut self, unitary: &[C64]) {
        let dim = self.dimension();
        assert_eq!(unitary.len(), dim * dim,
            "Unitary size mismatch: expected {}, got {}", dim * dim, unitary.len());

        // Step 1: temp = U * rho (parallelize outer row loop)
        let rho = &self.data;
        let temp: Vec<C64> = if dim >= PAR_THRESHOLD {
            (0..dim).into_par_iter().flat_map(|i| {
                let mut row = vec![complex::ZERO; dim];
                for j in 0..dim {
                    let mut sum = complex::ZERO;
                    for k in 0..dim {
                        sum = cx_add(sum, cx_mul(unitary[i * dim + k], rho[k * dim + j]));
                    }
                    row[j] = sum;
                }
                row
            }).collect()
        } else {
            let mut temp = vec![complex::ZERO; dim * dim];
            for i in 0..dim {
                for j in 0..dim {
                    let mut sum = complex::ZERO;
                    for k in 0..dim {
                        sum = cx_add(sum, cx_mul(unitary[i * dim + k], rho[k * dim + j]));
                    }
                    temp[i * dim + j] = sum;
                }
            }
            temp
        };

        // Step 2: result = temp * U^dagger (parallelize outer row loop)
        self.data = if dim >= PAR_THRESHOLD {
            (0..dim).into_par_iter().flat_map(|i| {
                let mut row = vec![complex::ZERO; dim];
                for j in 0..dim {
                    let mut sum = complex::ZERO;
                    for k in 0..dim {
                        sum = cx_add(sum, cx_mul(temp[i * dim + k], cx_conj(unitary[j * dim + k])));
                    }
                    row[j] = sum;
                }
                row
            }).collect()
        } else {
            let mut result = vec![complex::ZERO; dim * dim];
            for i in 0..dim {
                for j in 0..dim {
                    let mut sum = complex::ZERO;
                    for k in 0..dim {
                        sum = cx_add(sum, cx_mul(temp[i * dim + k], cx_conj(unitary[j * dim + k])));
                    }
                    result[i * dim + j] = sum;
                }
            }
            result
        };
    }
}

// =============================================================================
// Measurement
// =============================================================================

impl DensityMatrix {
    /// Measure a single qubit via the Born rule, returning the outcome
    /// and the post-measurement density matrix.
    ///
    /// Steps:
    ///   1. Compute p(0) = sum over basis states with qubit=0 of rho[k][k]
    ///   2. Sample outcome: 0 with probability p(0), else 1
    ///   3. Project: rho' = P_outcome * rho * P_outcome / p(outcome)
    ///      where P_0 = I_rest tensor |0><0| and P_1 = I_rest tensor |1><1|
    ///   4. Return (outcome, rho')
    ///
    /// The returned DensityMatrix has the SAME number of qubits (no trace-out).
    ///
    /// # Panics
    /// Panics if `target >= self.num_qubits`.
    pub fn measure_qubit(&self, target: u8) -> (u8, DensityMatrix) {
        let n = self.num_qubits as usize;
        let dim = self.dimension();
        assert!(
            (target as usize) < n,
            "target qubit {} out of range for {}-qubit system",
            target, n
        );

        let bit = n - 1 - target as usize;
        let mask = 1usize << bit;

        // Step 1: Compute p(0) = sum of rho[k][k] where bit `target` of k is 0
        let mut p0: f64 = 0.0;
        for k in 0..dim {
            if k & mask == 0 {
                p0 += self.data[k * dim + k].0;
            }
        }
        // Clamp to valid probability range
        p0 = p0.clamp(0.0, 1.0);
        let p1 = 1.0 - p0;

        // Step 2: Sample outcome
        let mut rng = rand::thread_rng();
        let r: f64 = rng.r#gen();
        let outcome: u8 = if r < p0 { 0 } else { 1 };
        let p_outcome = if outcome == 0 { p0 } else { p1 };

        // Step 3: Project and renormalize
        // P_outcome zeroes out all rows and columns where the target bit
        // doesn't match the outcome.
        let mut result = self.clone();
        let outcome_bit = if outcome == 0 { 0 } else { mask };

        for i in 0..dim {
            for j in 0..dim {
                if (i & mask) != outcome_bit || (j & mask) != outcome_bit {
                    result.data[i * dim + j] = complex::ZERO;
                }
            }
        }

        // Renormalize: rho' = projected / p(outcome)
        if p_outcome > 1e-30 {
            let inv_p = 1.0 / p_outcome;
            for entry in result.data.iter_mut() {
                *entry = cx_scale(inv_p, *entry);
            }
        }

        (outcome, result)
    }

    /// Stochastic measurement of all qubits using the Born rule.
    pub fn measure_all(&self) -> (u16, DensityMatrix) {
        let dim = self.dimension();
        let probs = self.diagonal_probabilities();

        let mut rng = rand::thread_rng();
        let r: f64 = rng.r#gen();

        let mut cumulative = 0.0;
        let mut outcome = dim - 1; // fallback
        for (k, &p) in probs.iter().enumerate() {
            cumulative += p;
            if r < cumulative {
                outcome = k;
                break;
            }
        }

        // Collapsed state: |outcome><outcome|
        let mut collapsed = DensityMatrix::new_zero_state(self.num_qubits);
        // Clear the default zero state and set the outcome
        for entry in collapsed.data.iter_mut() {
            *entry = complex::ZERO;
        }
        collapsed.data[outcome * dim + outcome] = complex::ONE;

        (outcome as u16, collapsed)
    }

    /// Deterministic measurement: return the basis state with the highest
    /// diagonal probability (argmax of Re(rho[k][k])).
    pub fn measure_deterministic(&self) -> u16 {
        let dim = self.dimension();
        let mut max_idx = 0;
        let mut max_prob = f64::NEG_INFINITY;
        for k in 0..dim {
            let p = self.data[k * dim + k].0;
            if p > max_prob {
                max_prob = p;
                max_idx = k;
            }
        }
        max_idx as u16
    }

    /// Extract the diagonal probabilities as a Vec.
    pub fn diagonal_probabilities(&self) -> Vec<f64> {
        let dim = self.dimension();
        if dim >= PAR_THRESHOLD {
            (0..dim).into_par_iter().map(|k| self.data[k * dim + k].0).collect()
        } else {
            (0..dim).map(|k| self.data[k * dim + k].0).collect()
        }
    }
}

// =============================================================================
// Metrics
// =============================================================================

impl DensityMatrix {
    /// Purity: Tr(rho^2).
    ///
    /// Computed as sum_{i,j} |rho[i][j]|^2 (which equals Tr(rho^2) for
    /// Hermitian rho).
    pub fn purity(&self) -> f64 {
        let dim = self.dimension();
        if dim >= PAR_THRESHOLD {
            self.data.par_iter().map(|z| cx_norm_sq(*z)).sum()
        } else {
            self.data.iter().map(|z| cx_norm_sq(*z)).sum()
        }
    }

    /// Shannon entropy of the diagonal probability distribution, normalized
    /// to [0, 1].
    ///
    /// S_diag = -sum_k p_k * log2(p_k) / log2(dim)
    ///
    /// This is NOT the von Neumann entropy. It measures the spread of
    /// measurement probabilities, not the mixedness of the quantum state.
    /// For diagnostic and backward-compatibility use.
    ///
    /// Previously named `von_neumann_entropy()`.
    pub fn diagonal_entropy(&self) -> f64 {
        let dim = self.dimension();
        if dim <= 1 {
            return 0.0;
        }
        let probs = self.diagonal_probabilities();
        let entropy: f64 = probs.iter()
            .filter(|&&p| p > 0.0)
            .map(|&p| -p * p.log2())
            .sum();
        let max_entropy = (dim as f64).log2();
        if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        }
    }

    /// Compute the eigenvalues of the density matrix via Jacobi iteration.
    ///
    /// Returns eigenvalues sorted in descending order. All eigenvalues are
    /// real (density matrices are Hermitian). Negative eigenvalues from
    /// floating-point error are clamped to 0.
    pub fn eigenvalues(&self) -> Vec<f64> {
        let dim = self.dimension();
        let mut matrix = self.data.clone();
        jacobi_eigenvalues(&mut matrix, dim, 1e-12, 1000 * dim * dim)
    }

    /// True von Neumann entropy: S(rho) = -Tr(rho log rho).
    ///
    /// Computed as S = -sum_i (lambda_i * ln(lambda_i)) / ln(dim) where
    /// lambda_i are the eigenvalues of the density matrix. Normalized by
    /// ln(dim) so the result is in [0, 1].
    ///
    /// Properties:
    /// - S = 0 for all pure states (rank-1 density matrices).
    /// - S = 1 for the maximally mixed state (rho = I/dim).
    /// - S is unitarily invariant.
    pub fn von_neumann_entropy(&self) -> f64 {
        let dim = self.dimension();
        if dim <= 1 {
            return 0.0;
        }
        let eigenvalues = self.eigenvalues();
        let raw_entropy: f64 = if dim >= PAR_THRESHOLD {
            eigenvalues.par_iter()
                .filter(|&&lam| lam > 1e-15)
                .map(|&lam| -lam * lam.ln())
                .sum()
        } else {
            eigenvalues.iter()
                .filter(|&&lam| lam > 1e-15)
                .map(|&lam| -lam * lam.ln())
                .sum()
        };
        let max_entropy = (dim as f64).ln();
        if max_entropy > 0.0 {
            raw_entropy / max_entropy
        } else {
            0.0
        }
    }

    /// Trace of the density matrix: sum of diagonal elements.
    pub fn trace(&self) -> C64 {
        let dim = self.dimension();
        let mut sum = complex::ZERO;
        for k in 0..dim {
            sum = cx_add(sum, self.data[k * dim + k]);
        }
        sum
    }

    /// Compute the partial trace over subsystem B, yielding the reduced
    /// density matrix for subsystem A.
    ///
    /// The total system has `self.num_qubits()` qubits. Subsystem A consists
    /// of the first `num_qubits_a` qubits; subsystem B is the remainder.
    ///
    /// # Panics
    /// Panics if `num_qubits_a == 0` or `num_qubits_a >= self.num_qubits()`.
    pub fn partial_trace_b(&self, num_qubits_a: u8) -> DensityMatrix {
        assert!(num_qubits_a > 0 && num_qubits_a < self.num_qubits,
            "partition must be 1..{}, got {}", self.num_qubits, num_qubits_a);

        let dim_a = 1usize << num_qubits_a;
        let dim_b = 1usize << (self.num_qubits - num_qubits_a);
        let dim = self.dimension();

        // rho_A[i][j] = sum_k rho[i*dim_b + k][j*dim_b + k]
        let rho_a: Vec<C64> = if dim >= PAR_THRESHOLD {
            let data = &self.data;
            (0..dim_a).into_par_iter().flat_map(|i| {
                (0..dim_a).map(|j| {
                    let mut sum = complex::ZERO;
                    for k in 0..dim_b {
                        let r = i * dim_b + k;
                        let c = j * dim_b + k;
                        sum = cx_add(sum, data[r * dim + c]);
                    }
                    sum
                }).collect::<Vec<_>>()
            }).collect()
        } else {
            let mut rho_a = vec![complex::ZERO; dim_a * dim_a];
            for i in 0..dim_a {
                for j in 0..dim_a {
                    let mut sum = complex::ZERO;
                    for k in 0..dim_b {
                        let row = i * dim_b + k;
                        let col = j * dim_b + k;
                        sum = cx_add(sum, self.data[row * dim + col]);
                    }
                    rho_a[i * dim_a + j] = sum;
                }
            }
            rho_a
        };

        DensityMatrix {
            num_qubits: num_qubits_a,
            data: rho_a,
        }
    }

    /// Entanglement entropy: S(rho_A) for bipartite system A|B.
    ///
    /// Computes the true von Neumann entropy of the reduced density matrix
    /// obtained by tracing out subsystem B, using eigendecomposition.
    ///
    /// For a product state, entanglement entropy is 0.
    /// For a maximally entangled state of 2 qubits, it is 1.0 (in bits).
    ///
    /// The result is in bits (log base 2), NOT normalized by log2(dim_A).
    ///
    /// # Panics
    /// Panics if `num_qubits_a == 0` or `num_qubits_a >= self.num_qubits()`.
    pub fn entanglement_entropy(&self, num_qubits_a: u8) -> f64 {
        let rho_a = self.partial_trace_b(num_qubits_a);
        let eigenvalues = rho_a.eigenvalues();

        let entropy: f64 = eigenvalues.iter()
            .filter(|&&lam| lam > 1e-15)
            .map(|&lam| -lam * lam.log2())
            .sum();

        entropy
    }

    /// Check if the density matrix satisfies basic validity constraints.
    pub fn is_valid(&self, tolerance: f64) -> bool {
        let dim = self.dimension();

        // Check trace ~= 1
        let tr = self.trace();
        if (tr.0 - 1.0).abs() > tolerance || tr.1.abs() > tolerance {
            return false;
        }

        // Check Hermitian: rho[i][j] ~= conj(rho[j][i])
        for i in 0..dim {
            for j in 0..dim {
                let rij = self.data[i * dim + j];
                let rji = self.data[j * dim + i];
                let conj_rji = cx_conj(rji);
                if (rij.0 - conj_rji.0).abs() > tolerance || (rij.1 - conj_rji.1).abs() > tolerance {
                    return false;
                }
            }
        }

        // Check diagonal entries are real and non-negative
        for k in 0..dim {
            let diag = self.data[k * dim + k];
            if diag.1.abs() > tolerance || diag.0 < -tolerance {
                return false;
            }
        }

        true
    }
}

// =============================================================================
// Qubit-Level Gate Application
// =============================================================================

impl DensityMatrix {
    /// Apply a two-qubit gate to specific control and target qubits.
    ///
    /// The `gate` parameter is a 4x4 unitary matrix in row-major order,
    /// acting on the 2-qubit subspace of (ctrl, tgt).
    ///
    /// The basis ordering for the 4x4 gate is:
    ///   index 0 = (ctrl=0, tgt=0), index 1 = (ctrl=0, tgt=1),
    ///   index 2 = (ctrl=1, tgt=0), index 3 = (ctrl=1, tgt=1).
    ///
    /// # Panics
    /// Panics if ctrl or tgt >= num_qubits, or if ctrl == tgt.
    pub fn apply_two_qubit_gate(&mut self, ctrl: u8, tgt: u8, gate: &[C64; 16]) {
        let n = self.num_qubits as usize;
        let dim = self.dimension();
        assert!(
            (ctrl as usize) < n && (tgt as usize) < n,
            "qubit indices ({}, {}) out of range for {}-qubit system",
            ctrl, tgt, n
        );
        assert!(ctrl != tgt, "ctrl ({}) must differ from tgt ({})", ctrl, tgt);

        let ctrl_bit = n - 1 - ctrl as usize;
        let tgt_bit = n - 1 - tgt as usize;
        let ctrl_mask = 1usize << ctrl_bit;
        let tgt_mask = 1usize << tgt_bit;

        // Collect valid base indices (where both ctrl and tgt bits are 0)
        let bases: Vec<usize> = (0..dim)
            .filter(|&base| base & (ctrl_mask | tgt_mask) == 0)
            .collect();

        // Step 1: Apply gate to rows: temp = G * rho
        let mut temp = self.data.clone();
        if dim >= PAR_THRESHOLD {
            // Each base produces 4 rows of updates; collect (flat_idx, value) pairs
            let updates: Vec<(usize, C64)> = bases.par_iter().flat_map(|&base| {
                let i00 = base;
                let i01 = base | tgt_mask;
                let i10 = base | ctrl_mask;
                let i11 = base | ctrl_mask | tgt_mask;
                let idxs = [i00, i01, i10, i11];
                let mut local: Vec<(usize, C64)> = Vec::with_capacity(4 * dim);
                for j in 0..dim {
                    let orig = [
                        self.data[i00 * dim + j],
                        self.data[i01 * dim + j],
                        self.data[i10 * dim + j],
                        self.data[i11 * dim + j],
                    ];
                    for (a, &row_idx) in idxs.iter().enumerate() {
                        let mut sum = complex::ZERO;
                        for b in 0..4 {
                            sum = cx_add(sum, cx_mul(gate[a * 4 + b], orig[b]));
                        }
                        local.push((row_idx * dim + j, sum));
                    }
                }
                local
            }).collect();
            for (idx, val) in updates {
                temp[idx] = val;
            }
        } else {
            for &base in &bases {
                let i00 = base;
                let i01 = base | tgt_mask;
                let i10 = base | ctrl_mask;
                let i11 = base | ctrl_mask | tgt_mask;
                let idxs = [i00, i01, i10, i11];
                for j in 0..dim {
                    let orig = [
                        self.data[i00 * dim + j],
                        self.data[i01 * dim + j],
                        self.data[i10 * dim + j],
                        self.data[i11 * dim + j],
                    ];
                    for (a, &row_idx) in idxs.iter().enumerate() {
                        let mut sum = complex::ZERO;
                        for b in 0..4 {
                            sum = cx_add(sum, cx_mul(gate[a * 4 + b], orig[b]));
                        }
                        temp[row_idx * dim + j] = sum;
                    }
                }
            }
        }

        // Step 2: Apply gate^dagger to columns: result = temp * G^dagger
        if dim >= PAR_THRESHOLD {
            let updates: Vec<(usize, C64)> = bases.par_iter().flat_map(|&base| {
                let j00 = base;
                let j01 = base | tgt_mask;
                let j10 = base | ctrl_mask;
                let j11 = base | ctrl_mask | tgt_mask;
                let jdxs = [j00, j01, j10, j11];
                let mut local: Vec<(usize, C64)> = Vec::with_capacity(4 * dim);
                for i in 0..dim {
                    let orig = [
                        temp[i * dim + j00],
                        temp[i * dim + j01],
                        temp[i * dim + j10],
                        temp[i * dim + j11],
                    ];
                    for (a, &col_idx) in jdxs.iter().enumerate() {
                        let mut sum = complex::ZERO;
                        for b in 0..4 {
                            sum = cx_add(sum, cx_mul(orig[b], cx_conj(gate[a * 4 + b])));
                        }
                        local.push((i * dim + col_idx, sum));
                    }
                }
                local
            }).collect();
            for (idx, val) in updates {
                self.data[idx] = val;
            }
        } else {
            for &base in &bases {
                let j00 = base;
                let j01 = base | tgt_mask;
                let j10 = base | ctrl_mask;
                let j11 = base | ctrl_mask | tgt_mask;
                let jdxs = [j00, j01, j10, j11];
                for i in 0..dim {
                    let orig = [
                        temp[i * dim + j00],
                        temp[i * dim + j01],
                        temp[i * dim + j10],
                        temp[i * dim + j11],
                    ];
                    for (a, &col_idx) in jdxs.iter().enumerate() {
                        let mut sum = complex::ZERO;
                        for b in 0..4 {
                            sum = cx_add(sum, cx_mul(orig[b], cx_conj(gate[a * 4 + b])));
                        }
                        self.data[i * dim + col_idx] = sum;
                    }
                }
            }
        }
    }

    /// Apply a single-qubit gate to a specific qubit in the register.
    ///
    /// Performs the transformation rho' = U * rho * U^dagger where U is the
    /// full-register unitary constructed by embedding the 2x2 gate at the
    /// target qubit position via Kronecker product.
    ///
    /// # Panics
    /// Panics if `target >= self.num_qubits`.
    pub fn apply_single_qubit_gate(&mut self, target: u8, gate: &[C64; 4]) {
        let n = self.num_qubits as usize;
        let dim = self.dimension();
        assert!(
            (target as usize) < n,
            "target qubit {} out of range for {}-qubit system",
            target,
            n
        );

        let bit = n - 1 - target as usize;
        let mask = 1usize << bit;

        let [g00, g01, g10, g11] = *gate;

        // Step 1: Apply gate to rows (temp = G * rho)
        let mut temp = self.data.clone();
        for i0 in 0..dim {
            if i0 & mask != 0 {
                continue;
            }
            let i1 = i0 | mask;
            for j in 0..dim {
                let r0 = self.data[i0 * dim + j];
                let r1 = self.data[i1 * dim + j];
                temp[i0 * dim + j] = cx_add(cx_mul(g00, r0), cx_mul(g01, r1));
                temp[i1 * dim + j] = cx_add(cx_mul(g10, r0), cx_mul(g11, r1));
            }
        }

        // Step 2: Apply gate^dagger to columns (result = temp * G^dagger)
        for j0 in 0..dim {
            if j0 & mask != 0 {
                continue;
            }
            let j1 = j0 | mask;
            for i in 0..dim {
                let c0 = temp[i * dim + j0];
                let c1 = temp[i * dim + j1];
                self.data[i * dim + j0] =
                    cx_add(cx_mul(c0, cx_conj(g00)), cx_mul(c1, cx_conj(g01)));
                self.data[i * dim + j1] =
                    cx_add(cx_mul(c0, cx_conj(g10)), cx_mul(c1, cx_conj(g11)));
            }
        }
    }

    /// Compute the tensor product of two density matrices.
    ///
    /// rho_AB = rho_A tensor rho_B
    ///
    /// The resulting matrix has dimension dim_A * dim_B and
    /// (num_qubits_A + num_qubits_B) qubits.
    ///
    /// # Panics
    /// Panics if combined qubit count exceeds MAX_QUBITS.
    pub fn tensor_product(&self, other: &DensityMatrix) -> DensityMatrix {
        let n0 = self.num_qubits;
        let n1 = other.num_qubits;
        let n_total = n0 + n1;
        assert!(
            n_total <= MAX_QUBITS,
            "tensor_product: combined qubits {} + {} = {} exceeds MAX_QUBITS ({})",
            n0, n1, n_total, MAX_QUBITS
        );

        let dim_a = self.dimension();
        let dim_b = other.dimension();
        let dim_ab = dim_a * dim_b;

        // Kronecker product: result[i*dim_b + j][k*dim_b + l] = self[i][k] * other[j][l]
        let data: Vec<C64> = if dim_ab >= PAR_THRESHOLD {
            let self_data = &self.data;
            let other_data = &other.data;
            (0..dim_a).into_par_iter().flat_map(|i| {
                let mut block = vec![complex::ZERO; dim_b * dim_ab];
                for j in 0..dim_b {
                    let row = i * dim_b + j;
                    for k in 0..dim_a {
                        let a_ik = self_data[i * dim_a + k];
                        for l in 0..dim_b {
                            let col = k * dim_b + l;
                            let b_jl = other_data[j * dim_b + l];
                            block[j * dim_ab + col] = cx_mul(a_ik, b_jl);
                        }
                    }
                    let _ = row; // used for clarity
                }
                block
            }).collect()
        } else {
            let mut data = vec![complex::ZERO; dim_ab * dim_ab];
            for i in 0..dim_a {
                for j in 0..dim_b {
                    let row = i * dim_b + j;
                    for k in 0..dim_a {
                        let a_ik = self.data[i * dim_a + k];
                        for l in 0..dim_b {
                            let col = k * dim_b + l;
                            let b_jl = other.data[j * dim_b + l];
                            data[row * dim_ab + col] = cx_mul(a_ik, b_jl);
                        }
                    }
                }
            }
            data
        };

        DensityMatrix {
            num_qubits: n_total,
            data,
        }
    }
}

// =============================================================================
// Display
// =============================================================================

impl std::fmt::Display for DensityMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dim = self.dimension();
        writeln!(f, "DensityMatrix({} qubits, dim={})", self.num_qubits, dim)?;

        if dim <= 8 {
            // Print full matrix
            for i in 0..dim {
                write!(f, "  [")?;
                for j in 0..dim {
                    let (re, im) = self.data[i * dim + j];
                    if j > 0 { write!(f, ", ")?; }
                    if im.abs() < 1e-10 {
                        write!(f, "{:7.4}", re)?;
                    } else {
                        write!(f, "({:.4},{:.4}i)", re, im)?;
                    }
                }
                writeln!(f, "]")?;
            }
        } else {
            // Print only diagonal probabilities
            writeln!(f, "  Diagonal probabilities:")?;
            let probs = self.diagonal_probabilities();
            for (k, p) in probs.iter().enumerate() {
                if *p > 1e-10 {
                    writeln!(f, "    |{:0width$b}> : {:.6}", k, p, width = self.num_qubits as usize)?;
                }
            }
        }
        write!(f, "  Purity: {:.6}, Trace: ({:.6}, {:.6})",
            self.purity(), self.trace().0, self.trace().1)
    }
}

// --- QuantumState trait implementation ---------------------------------------

impl QuantumState for DensityMatrix {
    fn num_qubits(&self) -> u8 {
        DensityMatrix::num_qubits(self)
    }

    fn dimension(&self) -> usize {
        DensityMatrix::dimension(self)
    }

    fn diagonal_probabilities(&self) -> Vec<f64> {
        DensityMatrix::diagonal_probabilities(self)
    }

    fn purity(&self) -> f64 {
        DensityMatrix::purity(self)
    }
}

// =============================================================================
// Jacobi eigenvalue decomposition for Hermitian matrices
// =============================================================================

/// Jacobi eigenvalue algorithm for complex Hermitian matrices.
///
/// Given a dim x dim Hermitian matrix (stored as flat Vec<C64> in row-major),
/// computes all eigenvalues by iterative Jacobi rotations that zero out
/// off-diagonal elements.
///
/// Returns eigenvalues sorted in descending order, with negative values
/// (from floating-point error) clamped to 0.
fn jacobi_eigenvalues(
    matrix: &mut [C64],
    dim: usize,
    tolerance: f64,
    max_sweeps: usize,
) -> Vec<f64> {
    if dim == 0 {
        return Vec::new();
    }
    if dim == 1 {
        return vec![matrix[0].0.max(0.0)];
    }

    for _sweep in 0..max_sweeps {
        // Find the off-diagonal element with largest magnitude
        let (max_val, p, q) = if dim >= PAR_THRESHOLD {
            (0..dim).into_par_iter().map(|i| {
                let mut local_max = 0.0_f64;
                let mut local_p = i;
                let mut local_q = if i + 1 < dim { i + 1 } else { i };
                for j in (i + 1)..dim {
                    let mag = cx_norm_sq(matrix[i * dim + j]);
                    if mag > local_max {
                        local_max = mag;
                        local_p = i;
                        local_q = j;
                    }
                }
                (local_max, local_p, local_q)
            }).reduce(|| (0.0_f64, 0, 1), |a, b| if a.0 >= b.0 { a } else { b })
        } else {
            let mut max_val = 0.0_f64;
            let mut p = 0;
            let mut q = 1;
            for i in 0..dim {
                for j in (i + 1)..dim {
                    let mag = cx_norm_sq(matrix[i * dim + j]);
                    if mag > max_val {
                        max_val = mag;
                        p = i;
                        q = j;
                    }
                }
            }
            (max_val, p, q)
        };

        // Check convergence
        if max_val.sqrt() < tolerance {
            break;
        }

        // Apply Jacobi rotation to zero out element (p, q).
        // For Hermitian matrix: a_pq = conj(a_qp)
        let app = matrix[p * dim + p].0; // real (diagonal)
        let aqq = matrix[q * dim + q].0; // real (diagonal)
        let apq = matrix[p * dim + q];   // complex off-diagonal

        let apq_mag = cx_norm_sq(apq).sqrt();
        if apq_mag < 1e-30 {
            continue;
        }

        // Phase factor: apq = |apq| * e^{i*phi}
        let phase = (apq.0 / apq_mag, apq.1 / apq_mag); // e^{i*phi}
        let phase_conj = cx_conj(phase);                  // e^{-i*phi}

        // Now compute the 2x2 real Jacobi rotation for the matrix
        // [[app, |apq|], [|apq|, aqq]]
        let tau = (aqq - app) / (2.0 * apq_mag);
        let t = if tau.abs() < 1e-30 {
            1.0 // tau ~ 0, so t = 1
        } else {
            let sign = if tau >= 0.0 { 1.0 } else { -1.0 };
            sign / (tau.abs() + (1.0 + tau * tau).sqrt())
        };

        let c = 1.0 / (1.0 + t * t).sqrt(); // cos(theta)
        let s = t * c;                        // sin(theta)

        // The complex rotation is:
        // G[p,p] = c,  G[p,q] = s * e^{i*phi},  G[q,p] = -s * e^{-i*phi},  G[q,q] = c
        // After rotation: A' = G^H A G

        // Update diagonal elements
        matrix[p * dim + p] = (app - t * apq_mag, 0.0);
        matrix[q * dim + q] = (aqq + t * apq_mag, 0.0);
        // Zero out (p,q) and (q,p)
        matrix[p * dim + q] = (0.0, 0.0);
        matrix[q * dim + p] = (0.0, 0.0);

        // Update the rest of rows/columns p and q
        if dim >= PAR_THRESHOLD {
            let updates: Vec<(usize, C64, C64)> = (0..dim).into_par_iter()
                .filter(|&r| r != p && r != q)
                .map(|r| {
                    let arp = matrix[r * dim + p];
                    let arq = matrix[r * dim + q];
                    let new_rp = cx_add(
                        cx_scale(c, arp),
                        cx_scale(-s, cx_mul(phase_conj, arq)),
                    );
                    let new_rq = cx_add(
                        cx_scale(s, cx_mul(phase, arp)),
                        cx_scale(c, arq),
                    );
                    (r, new_rp, new_rq)
                }).collect();
            for (r, new_rp, new_rq) in updates {
                matrix[r * dim + p] = new_rp;
                matrix[r * dim + q] = new_rq;
                matrix[p * dim + r] = cx_conj(new_rp);
                matrix[q * dim + r] = cx_conj(new_rq);
            }
        } else {
            for r in 0..dim {
                if r == p || r == q {
                    continue;
                }
                let arp = matrix[r * dim + p];
                let arq = matrix[r * dim + q];
                let new_rp = cx_add(
                    cx_scale(c, arp),
                    cx_scale(-s, cx_mul(phase_conj, arq)),
                );
                let new_rq = cx_add(
                    cx_scale(s, cx_mul(phase, arp)),
                    cx_scale(c, arq),
                );
                matrix[r * dim + p] = new_rp;
                matrix[r * dim + q] = new_rq;
                matrix[p * dim + r] = cx_conj(new_rp);
                matrix[q * dim + r] = cx_conj(new_rq);
            }
        }
    }

    // Extract diagonal as eigenvalues, clamp negative, sort descending
    let mut eigenvalues: Vec<f64> = (0..dim)
        .map(|i| matrix[i * dim + i].0.max(0.0))
        .collect();
    eigenvalues.sort_by(|a, b| b.partial_cmp(a).unwrap());
    eigenvalues
}
