//! Quantum Fourier Transform (QFT) kernel.

use std::f64::consts::PI;
use cqam_core::error::CqamError;
use crate::complex::{self, C64, cx_scale, cx_exp_i, cx_mul};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;

/// Quantum Fourier Transform kernel (kernel_id = 2).
///
/// Constructs the QFT unitary:
///   QFT[j][k] = (1/sqrt(N)) * exp(2*pi*i*j*k/N)
pub struct Fourier;

impl Fourier {
    /// Build the QFT unitary matrix for a given dimension.
    fn build_unitary(dim: usize) -> Vec<C64> {
        let n_f64 = dim as f64;
        let norm = 1.0 / n_f64.sqrt();
        let mut unitary = vec![complex::ZERO; dim * dim];
        for j in 0..dim {
            for k in 0..dim {
                let angle = 2.0 * PI * (j as f64) * (k as f64) / n_f64;
                let entry = cx_exp_i(angle);
                unitary[j * dim + k] = cx_scale(norm, entry);
            }
        }
        unitary
    }
}

impl Kernel for Fourier {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let unitary = Self::build_unitary(input.dimension());
        let mut result = input.clone();
        result.apply_unitary(&unitary);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, String> {
        // Factored QFT via Hadamard + controlled phase rotations.
        // O(n^2 * 2^n) time, O(2^n) memory -- no unitary matrix needed.
        let n = input.num_qubits() as usize;
        let dim = input.dimension();
        let mut amps = input.amplitudes().to_vec();

        for j in 0..n {
            // Apply Hadamard to qubit j
            let bit_j = 1 << (n - 1 - j);
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

            // Apply controlled phase rotations: CR_k for k = j+1..n-1
            for k in (j + 1)..n {
                let m = k - j; // distance determines rotation angle
                let angle = 2.0 * PI / (1u64 << m) as f64;
                let bit_k = 1 << (n - 1 - k);
                let phase = cx_exp_i(angle);
                // Apply phase only when both qubit j and qubit k are |1>
                let mask = bit_j | bit_k;
                for (state, amp) in amps.iter_mut().enumerate() {
                    if (state & mask) == mask {
                        *amp = cx_mul(phase, *amp);
                    }
                }
            }
        }

        // Bit-reversal permutation on qubits
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

        Ok(Statevector::from_amplitudes(amps)
            .expect("Fourier apply_sv produced invalid amplitudes"))
    }
}
