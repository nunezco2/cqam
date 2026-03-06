//! Inverse Quantum Fourier Transform (IQFT) kernel.

use std::f64::consts::PI;
use cqam_core::error::CqamError;
use crate::complex::{C64, cx_exp_i, cx_mul, ZERO, ONE};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use rayon::prelude::*;

const PAR_THRESHOLD: usize = 256;

/// Inverse Quantum Fourier Transform kernel (kernel_id = 7).
///
/// Constructs the IQFT unitary (conjugate transpose of QFT):
///   IQFT[j][k] = (1/sqrt(N)) * exp(-2*pi*i*j*k/N)
pub struct FourierInv;

/// Build a controlled-phase 4x4 gate matrix for a given angle.
fn controlled_phase_gate(angle: f64) -> [C64; 16] {
    [
        ONE,  ZERO, ZERO, ZERO,
        ZERO, ONE,  ZERO, ZERO,
        ZERO, ZERO, ONE,  ZERO,
        ZERO, ZERO, ZERO, cx_exp_i(angle),
    ]
}

/// Hadamard gate as [C64; 4].
fn hadamard_gate() -> [C64; 4] {
    let h = std::f64::consts::FRAC_1_SQRT_2;
    [(h, 0.0), (h, 0.0), (h, 0.0), (-h, 0.0)]
}

impl Kernel for FourierInv {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        // Factored IQFT: reverse of QFT circuit.
        // bit-reversal, then for j = n-1..0: inverse CR gates, then H.
        let n = input.num_qubits() as usize;
        let mut result = input.clone();
        let h = hadamard_gate();

        // Bit-reversal via SWAP gates
        for i in 0..n / 2 {
            let j = n - 1 - i;
            if i != j {
                let swap: [C64; 16] = [
                    ONE,  ZERO, ZERO, ZERO,
                    ZERO, ZERO, ONE,  ZERO,
                    ZERO, ONE,  ZERO, ZERO,
                    ZERO, ZERO, ZERO, ONE,
                ];
                result.apply_two_qubit_gate(i as u8, j as u8, &swap);
            }
        }

        // Reverse order of QFT gates
        for j in (0..n).rev() {
            for k in ((j + 1)..n).rev() {
                let m = k - j;
                let angle = -2.0 * PI / (1u64 << m) as f64;
                let cp = controlled_phase_gate(angle);
                result.apply_two_qubit_gate(k as u8, j as u8, &cp);
            }
            result.apply_single_qubit_gate(j as u8, &h);
        }

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
                if dim >= PAR_THRESHOLD {
                    amps.par_iter_mut().enumerate().for_each(|(state, amp)| {
                        if (state & mask) == mask {
                            *amp = cx_mul(phase, *amp);
                        }
                    });
                } else {
                    for (state, amp) in amps.iter_mut().enumerate() {
                        if (state & mask) == mask {
                            *amp = cx_mul(phase, *amp);
                        }
                    }
                }
            }

            // Apply Hadamard to qubit j (self-inverse)
            let h_inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
            if dim >= PAR_THRESHOLD {
                let updates: Vec<(usize, C64, usize, C64)> = (0..dim)
                    .into_par_iter()
                    .filter(|&state| state & bit_j == 0)
                    .map(|state| {
                        let partner = state | bit_j;
                        let a = amps[state];
                        let b = amps[partner];
                        let new_s = (h_inv_sqrt2 * (a.0 + b.0), h_inv_sqrt2 * (a.1 + b.1));
                        let new_p = (h_inv_sqrt2 * (a.0 - b.0), h_inv_sqrt2 * (a.1 - b.1));
                        (state, new_s, partner, new_p)
                    })
                    .collect();
                for (s, new_s, p, new_p) in updates {
                    amps[s] = new_s;
                    amps[p] = new_p;
                }
            } else {
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
        }

        Ok(Statevector::from_amplitudes(amps)
            .expect("FourierInv apply_sv produced invalid amplitudes"))
    }
}
