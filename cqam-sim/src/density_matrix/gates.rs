//! Unitary gate application methods for `DensityMatrix`.

use super::DensityMatrix;
use crate::complex::{self, C64, cx_add, cx_mul, cx_conj};
use crate::constants::{PAR_THRESHOLD, MAX_QUBITS};
use rayon::prelude::*;

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

    /// Apply a diagonal unitary in-place: rho'[i][j] = phases[i] * conj(phases[j]) * rho[i][j].
    ///
    /// This is O(dim^2) instead of O(dim^3) for the general apply_unitary path.
    /// The `phases` slice must have exactly `dim` elements, where phases[k] is
    /// the diagonal entry U[k][k] of the unitary.
    ///
    /// # Panics
    /// Panics if `phases.len() != dim`.
    pub fn apply_diagonal_unitary(&mut self, phases: &[C64]) {
        let dim = self.dimension();
        assert_eq!(phases.len(), dim,
            "Diagonal unitary size mismatch: expected {}, got {}", dim, phases.len());

        if dim >= PAR_THRESHOLD {
            let data = &mut self.data;
            // Process each row in parallel
            data.par_chunks_mut(dim).enumerate().for_each(|(i, row)| {
                let pi = phases[i];
                for (j, entry) in row.iter_mut().enumerate() {
                    // rho'[i][j] = phases[i] * conj(phases[j]) * rho[i][j]
                    let pj_conj = cx_conj(phases[j]);
                    *entry = cx_mul(pi, cx_mul(pj_conj, *entry));
                }
            });
        } else {
            for (i, &pi) in phases.iter().enumerate() {
                for (j, &pj) in phases.iter().enumerate() {
                    let pj_conj = cx_conj(pj);
                    self.data[i * dim + j] = cx_mul(pi, cx_mul(pj_conj, self.data[i * dim + j]));
                }
            }
        }
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

        // Collect valid paired indices (where the target bit is 0)
        let pairs: Vec<usize> = (0..dim).filter(|&i0| i0 & mask == 0).collect();

        // Step 1: Apply gate to rows (temp = G * rho)
        let temp = if dim >= PAR_THRESHOLD {
            let data = &self.data;
            let mut temp = data.to_vec();
            let updates: Vec<(usize, usize, C64, C64)> = pairs.par_iter().flat_map(|&i0| {
                let i1 = i0 | mask;
                (0..dim).map(move |j| {
                    let r0 = data[i0 * dim + j];
                    let r1 = data[i1 * dim + j];
                    (i0, j,
                     cx_add(cx_mul(g00, r0), cx_mul(g01, r1)),
                     cx_add(cx_mul(g10, r0), cx_mul(g11, r1)))
                }).collect::<Vec<_>>()
            }).collect();
            for (i0, j, v0, v1) in updates {
                let i1 = i0 | mask;
                temp[i0 * dim + j] = v0;
                temp[i1 * dim + j] = v1;
            }
            temp
        } else {
            let mut temp = self.data.clone();
            for &i0 in &pairs {
                let i1 = i0 | mask;
                for j in 0..dim {
                    let r0 = self.data[i0 * dim + j];
                    let r1 = self.data[i1 * dim + j];
                    temp[i0 * dim + j] = cx_add(cx_mul(g00, r0), cx_mul(g01, r1));
                    temp[i1 * dim + j] = cx_add(cx_mul(g10, r0), cx_mul(g11, r1));
                }
            }
            temp
        };

        // Step 2: Apply gate^dagger to columns (result = temp * G^dagger)
        if dim >= PAR_THRESHOLD {
            let temp_ref = &temp;
            let updates: Vec<(usize, usize, C64, C64)> = pairs.par_iter().flat_map(|&j0| {
                let j1 = j0 | mask;
                (0..dim).map(move |i| {
                    let c0 = temp_ref[i * dim + j0];
                    let c1 = temp_ref[i * dim + j1];
                    (i, j0,
                     cx_add(cx_mul(c0, cx_conj(g00)), cx_mul(c1, cx_conj(g01))),
                     cx_add(cx_mul(c0, cx_conj(g10)), cx_mul(c1, cx_conj(g11))))
                }).collect::<Vec<_>>()
            }).collect();
            for (i, j0, v0, v1) in updates {
                let j1 = j0 | mask;
                self.data[i * dim + j0] = v0;
                self.data[i * dim + j1] = v1;
            }
        } else {
            for &j0 in &pairs {
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
