//! Inverse Quantum Fourier Transform (IQFT) kernel.

use std::f64::consts::PI;
use cqam_core::error::CqamError;
use crate::complex::{self, C64, cx_scale, cx_exp_i, cx_mul};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;

/// Inverse Quantum Fourier Transform kernel (kernel_id = 7).
///
/// Constructs the IQFT unitary (conjugate transpose of QFT):
///   IQFT[j][k] = (1/sqrt(N)) * exp(-2*pi*i*j*k/N)
pub struct FourierInv;

impl FourierInv {
    /// Build the IQFT unitary matrix for a given dimension.
    fn build_unitary(dim: usize) -> Vec<C64> {
        let n_f64 = dim as f64;
        let norm = 1.0 / n_f64.sqrt();
        let mut unitary = vec![complex::ZERO; dim * dim];
        for j in 0..dim {
            for k in 0..dim {
                let angle = -2.0 * PI * (j as f64) * (k as f64) / n_f64;
                let entry = cx_exp_i(angle);
                unitary[j * dim + k] = cx_scale(norm, entry);
            }
        }
        unitary
    }
}

impl Kernel for FourierInv {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let unitary = Self::build_unitary(input.dimension());
        let mut result = input.clone();
        result.apply_unitary(&unitary);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, String> {
        // Factored inverse QFT: reverse of the QFT circuit.
        // IQFT = bit-reversal, then for j = n-1..0: inverse CR gates, then H.
        // O(n^2 * 2^n) time, O(2^n) memory.
        let n = input.num_qubits() as usize;
        let dim = input.dimension();
        let mut amps = input.amplitudes().to_vec();

        // Bit-reversal permutation on qubits (same as QFT)
        let mut swapped = vec![false; dim];
        for state in 0..dim {
            if swapped[state] { continue; }
            let mut reversed = 0usize;
            for bit in 0..n {
                if state & (1 << bit) != 0 {
                    reversed |= 1 << (n - 1 - bit);
                }
            }
            if reversed != state {
                amps.swap(state, reversed);
                swapped[state] = true;
                swapped[reversed] = true;
            }
        }

        // Reverse order of QFT gates
        for j in (0..n).rev() {
            let bit_j = 1 << (n - 1 - j);

            // Inverse controlled phase rotations (negative angle)
            for k in ((j + 1)..n).rev() {
                let m = k - j;
                let angle = -2.0 * PI / (1u64 << m) as f64;
                let bit_k = 1 << (n - 1 - k);
                let phase = cx_exp_i(angle);
                let mask = bit_j | bit_k;
                for (state, amp) in amps.iter_mut().enumerate() {
                    if (state & mask) == mask {
                        *amp = cx_mul(phase, *amp);
                    }
                }
            }

            // Apply Hadamard to qubit j (self-inverse)
            let h_inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
            for state in 0..dim {
                if state & bit_j == 0 {
                    let partner = state | bit_j;
                    let a = amps[state];
                    let b = amps[partner];
                    amps[state] = (h_inv_sqrt2 * (a.0 + b.0), h_inv_sqrt2 * (a.1 + b.1));
                    amps[partner] = (h_inv_sqrt2 * (a.0 - b.0), h_inv_sqrt2 * (a.1 - b.1));
                }
            }
        }

        Ok(Statevector::from_amplitudes(amps)
            .expect("FourierInv apply_sv produced invalid amplitudes"))
    }
}
