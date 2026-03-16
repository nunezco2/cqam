//! Kraus channel application methods for `DensityMatrix`.
//!
//! These methods implement the quantum channel operation
//! rho -> sum_k K_k rho K_k^dag for single-qubit and two-qubit channels.

use super::DensityMatrix;
use crate::complex::C64;

impl DensityMatrix {
    /// Apply a single-qubit Kraus channel to `target` qubit.
    ///
    /// The channel is specified as a slice of 2x2 Kraus operators,
    /// each in row-major order as `[C64; 4]`.
    ///
    /// Computes: rho' = sum_k (I_rest tensor K_k) rho (I_rest tensor K_k)^dag
    ///
    /// # Panics
    /// Panics if `target >= self.num_qubits`.
    pub fn apply_single_qubit_channel(
        &mut self,
        target: u8,
        kraus_ops: &[[C64; 4]],
    ) {
        let n = self.num_qubits as usize;
        let dim = self.dimension();
        assert!(
            (target as usize) < n,
            "target qubit {} out of range for {}-qubit system",
            target, n
        );

        let bit = n - 1 - target as usize;
        let mask = 1usize << bit;

        let mut result = vec![C64::ZERO; dim * dim];

        for kraus in kraus_ops {
            let [k00, k01, k10, k11] = *kraus;

            // Step 1: temp = K * rho (apply K to rows)
            let mut temp = vec![C64::ZERO; dim * dim];
            for i0 in 0..dim {
                if i0 & mask != 0 { continue; }
                let i1 = i0 | mask;
                for j in 0..dim {
                    let r0 = self.data[i0 * dim + j];
                    let r1 = self.data[i1 * dim + j];
                    temp[i0 * dim + j] = k00 * r0 + k01 * r1;
                    temp[i1 * dim + j] = k10 * r0 + k11 * r1;
                }
            }

            // Step 2: result += temp * K^dag (apply K^dag to columns)
            for j0 in 0..dim {
                if j0 & mask != 0 { continue; }
                let j1 = j0 | mask;
                for i in 0..dim {
                    let c0 = temp[i * dim + j0];
                    let c1 = temp[i * dim + j1];
                    result[i * dim + j0] +=
                        c0 * k00.conj() + c1 * k01.conj();
                    result[i * dim + j1] +=
                        c0 * k10.conj() + c1 * k11.conj();
                }
            }
        }

        self.data = result;
    }

    /// Apply a two-qubit Kraus channel to qubits `(qubit_a, qubit_b)`.
    ///
    /// Each Kraus operator is a 4x4 matrix in row-major order as `[C64; 16]`.
    ///
    /// # Panics
    /// Panics if qubit indices are out of range or equal.
    pub fn apply_two_qubit_channel(
        &mut self,
        qubit_a: u8,
        qubit_b: u8,
        kraus_ops: &[[C64; 16]],
    ) {
        let n = self.num_qubits as usize;
        let dim = self.dimension();
        assert!(
            (qubit_a as usize) < n && (qubit_b as usize) < n,
            "qubit indices ({}, {}) out of range for {}-qubit system",
            qubit_a, qubit_b, n
        );
        assert!(qubit_a != qubit_b);

        let bit_a = n - 1 - qubit_a as usize;
        let bit_b = n - 1 - qubit_b as usize;
        let mask_a = 1usize << bit_a;
        let mask_b = 1usize << bit_b;

        let bases: Vec<usize> = (0..dim)
            .filter(|&b| b & (mask_a | mask_b) == 0)
            .collect();

        let mut result = vec![C64::ZERO; dim * dim];

        for kraus in kraus_ops {
            // Step 1: temp = K * rho (apply K to rows on the 4-subspace)
            let mut temp = self.data.clone();
            for &base in &bases {
                let i00 = base;
                let i01 = base | mask_b;
                let i10 = base | mask_a;
                let i11 = base | mask_a | mask_b;
                let idxs = [i00, i01, i10, i11];
                for j in 0..dim {
                    let orig = [
                        self.data[i00 * dim + j],
                        self.data[i01 * dim + j],
                        self.data[i10 * dim + j],
                        self.data[i11 * dim + j],
                    ];
                    for (a, &row_idx) in idxs.iter().enumerate() {
                        let mut sum = C64::ZERO;
                        for b in 0..4 {
                            sum += kraus[a * 4 + b] * orig[b];
                        }
                        temp[row_idx * dim + j] = sum;
                    }
                }
            }

            // Step 2: result += temp * K^dag (apply K^dag to columns)
            for &base in &bases {
                let j00 = base;
                let j01 = base | mask_b;
                let j10 = base | mask_a;
                let j11 = base | mask_a | mask_b;
                let jdxs = [j00, j01, j10, j11];
                for i in 0..dim {
                    let orig = [
                        temp[i * dim + j00],
                        temp[i * dim + j01],
                        temp[i * dim + j10],
                        temp[i * dim + j11],
                    ];
                    for (a, &col_idx) in jdxs.iter().enumerate() {
                        let mut sum = C64::ZERO;
                        for b in 0..4 {
                            sum += orig[b] * kraus[a * 4 + b].conj();
                        }
                        result[i * dim + col_idx] += sum;
                    }
                }
            }
        }

        self.data = result;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_channel_preserves_state() {
        // Identity Kraus operator: [I]
        let identity = [C64::ONE, C64::ZERO, C64::ZERO, C64::ONE];
        let mut dm = DensityMatrix::new_uniform(2);
        let original = dm.data.clone();
        dm.apply_single_qubit_channel(0, &[identity]);
        for (a, b) in dm.data.iter().zip(original.iter()) {
            assert!((*a - *b).norm_sq() < 1e-20);
        }
    }

    #[test]
    fn complete_dephasing_kills_off_diagonals() {
        // K0 = |0><0|, K1 = |1><1|
        let k0 = [C64::ONE, C64::ZERO, C64::ZERO, C64::ZERO];
        let k1 = [C64::ZERO, C64::ZERO, C64::ZERO, C64::ONE];

        // Create |+> state: (|0> + |1>)/sqrt(2)
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let amps = vec![C64(inv_sqrt2, 0.0), C64(inv_sqrt2, 0.0)];
        let mut dm = DensityMatrix::from_statevector(&amps).unwrap();

        dm.apply_single_qubit_channel(0, &[k0, k1]);

        // Off-diagonals should be zero
        assert!(dm.get(0, 1).norm_sq() < 1e-20);
        assert!(dm.get(1, 0).norm_sq() < 1e-20);
        // Diagonal should be 0.5 each
        assert!((dm.get(0, 0).0 - 0.5).abs() < 1e-10);
        assert!((dm.get(1, 1).0 - 0.5).abs() < 1e-10);
    }

    #[test]
    fn trace_preserved_after_channel() {
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let amps = vec![C64(inv_sqrt2, 0.0), C64(inv_sqrt2, 0.0)];
        let mut dm = DensityMatrix::from_statevector(&amps).unwrap();

        // Amplitude damping with gamma = 0.3
        let gamma = 0.3_f64;
        let sg = (1.0 - gamma).sqrt();
        let sqg = gamma.sqrt();
        let k0 = [C64::ONE, C64::ZERO, C64::ZERO, C64(sg, 0.0)];
        let k1 = [C64::ZERO, C64(sqg, 0.0), C64::ZERO, C64::ZERO];

        dm.apply_single_qubit_channel(0, &[k0, k1]);

        let trace = dm.get(0, 0).0 + dm.get(1, 1).0;
        assert!((trace - 1.0).abs() < 1e-10, "Trace = {}", trace);
    }

    #[test]
    fn two_qubit_identity_channel() {
        let mut identity = [C64::ZERO; 16];
        identity[0] = C64::ONE;
        identity[5] = C64::ONE;
        identity[10] = C64::ONE;
        identity[15] = C64::ONE;

        let mut dm = DensityMatrix::new_bell();
        let original = dm.data.clone();
        dm.apply_two_qubit_channel(0, 1, &[identity]);
        for (a, b) in dm.data.iter().zip(original.iter()) {
            assert!((*a - *b).norm_sq() < 1e-20);
        }
    }

    #[test]
    fn two_qubit_trace_preserved() {
        let mut dm = DensityMatrix::new_bell();

        // Apply a two-qubit depolarizing-like channel
        let mut identity = [C64::ZERO; 16];
        identity[0] = C64::ONE;
        identity[5] = C64::ONE;
        identity[10] = C64::ONE;
        identity[15] = C64::ONE;
        // Scale by sqrt(0.9)
        let s0 = 0.9_f64.sqrt();
        let k0: [C64; 16] = identity.map(|c| c.scale(s0));

        // A swap-like operator scaled by sqrt(0.1)
        let s1 = 0.1_f64.sqrt();
        let mut swap = [C64::ZERO; 16];
        swap[0] = C64(s1, 0.0);
        swap[6] = C64(s1, 0.0);
        swap[9] = C64(s1, 0.0);
        swap[15] = C64(s1, 0.0);

        dm.apply_two_qubit_channel(0, 1, &[k0, swap]);

        let dim = dm.dimension();
        let mut trace = 0.0;
        for i in 0..dim {
            trace += dm.get(i, i).0;
        }
        assert!((trace - 1.0).abs() < 1e-10, "Trace = {}", trace);
    }
}
