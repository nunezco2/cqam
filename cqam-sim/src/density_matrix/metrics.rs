//! Purity, entropy, partial trace, and validation metrics for `DensityMatrix`.

use super::DensityMatrix;
use super::jacobi::jacobi_eigenvalues;
use crate::complex::{self, C64, cx_add, cx_conj, cx_norm_sq};
use crate::constants::{PAR_THRESHOLD, EF_EPSILON, SF_EPSILON};
use rayon::prelude::*;

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

    /// Returns true if any qubit's single-qubit reduced state has purity < 1 - epsilon.
    ///
    /// Returns true if the state is in superposition in the computational basis.
    ///
    /// For a density matrix, checks whether more than one diagonal element
    /// (measurement probability) is nonzero. Returns false when the state is
    /// a single basis state or a mixture that collapses to a single outcome.
    ///
    /// Cost: O(2^n) worst case.
    pub fn is_in_superposition(&self) -> bool {
        let dim = self.dimension();
        let mut nonzero_count = 0usize;
        for k in 0..dim {
            let (re, _im) = self.data[k * dim + k]; // diagonal: rho[k][k] is real
            if re > SF_EPSILON {
                nonzero_count += 1;
                if nonzero_count > 1 {
                    return true;
                }
            }
        }
        false
    }

    /// For each qubit k, computes the 2x2 reduced density matrix by tracing out
    /// all other qubits. If any qubit has reduced purity < 1 - EF_EPSILON, the
    /// state is entangled (or mixed in a way that appears entangled at the
    /// single-qubit level). Early-exits on the first entangled qubit found.
    pub fn is_any_qubit_entangled(&self) -> bool {
        let n = self.num_qubits as usize;
        if n < 2 {
            return false;
        }
        let dim = self.dimension();

        for k in 0..n {
            // Bit position for qubit k (MSB-first convention)
            let bit = n - 1 - k;
            let mask = 1usize << bit;

            let mut rho_k_00: (f64, f64) = (0.0, 0.0);
            let mut rho_k_11: (f64, f64) = (0.0, 0.0);
            let mut rho_k_01: (f64, f64) = (0.0, 0.0);

            // Iterate over all 2^(n-1) configurations m of the other qubits.
            // For each m, compute the indices with bit `bit` set to 0 and 1.
            for m_compact in 0..(dim >> 1) {
                // Insert a 0 at position `bit` in m_compact to get the full index
                let low = m_compact & ((1 << bit) - 1);
                let high = m_compact >> bit;
                let idx0 = (high << (bit + 1)) | low;        // bit k = 0
                let idx1 = idx0 | mask;                       // bit k = 1

                // rho_k[0][0] += rho[idx0][idx0]
                let v00 = self.data[idx0 * dim + idx0];
                rho_k_00.0 += v00.0;
                rho_k_00.1 += v00.1;

                // rho_k[1][1] += rho[idx1][idx1]
                let v11 = self.data[idx1 * dim + idx1];
                rho_k_11.0 += v11.0;
                rho_k_11.1 += v11.1;

                // rho_k[0][1] += rho[idx0][idx1]
                let v01 = self.data[idx0 * dim + idx1];
                rho_k_01.0 += v01.0;
                rho_k_01.1 += v01.1;
            }

            // purity = rho_k_00.re^2 + rho_k_11.re^2 + 2*|rho_k_01|^2
            // (imaginary parts of diagonal entries should be ~0 for valid DM,
            //  but we use only .re for robustness)
            let purity = rho_k_00.0 * rho_k_00.0 + rho_k_11.0 * rho_k_11.0
                + 2.0 * (rho_k_01.0 * rho_k_01.0 + rho_k_01.1 * rho_k_01.1);

            if purity < 1.0 - EF_EPSILON {
                return true;
            }
        }
        false
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
