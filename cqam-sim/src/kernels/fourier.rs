//! Quantum Fourier Transform (QFT) kernel.

use std::f64::consts::PI;
use cqam_core::error::CqamError;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use rayon::prelude::*;

use crate::constants::PAR_THRESHOLD;

/// Quantum Fourier Transform kernel (kernel_id = 2).
///
/// Constructs the QFT unitary:
///   QFT[j][k] = (1/sqrt(N)) * exp(2*pi*i*j*k/N)
pub struct Fourier;

/// Build a controlled-phase 4x4 gate matrix for a given angle.
/// Applies exp(i*angle) only to the |11> state.
fn controlled_phase_gate(angle: f64) -> [C64; 16] {
    [
        C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
        C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
        C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
        C64::ZERO, C64::ZERO, C64::ZERO, C64::exp_i(angle),
    ]
}

/// Hadamard gate as [C64; 4].
fn hadamard_gate() -> [C64; 4] {
    let h = std::f64::consts::FRAC_1_SQRT_2;
    [C64(h, 0.0), C64(h, 0.0), C64(h, 0.0), C64(-h, 0.0)]
}

impl Kernel for Fourier {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        // Factored QFT via Hadamard + controlled phase rotations + bit-reversal.
        // Uses O(n^2) gate applications at O(dim) each = O(n^2 * dim) total,
        // instead of O(dim^3) for the full unitary path.
        let n = input.num_qubits() as usize;
        let mut result = input.clone();
        let h = hadamard_gate();

        for j in 0..n {
            result.apply_single_qubit_gate(j as u8, &h);
            for k in (j + 1)..n {
                let m = k - j;
                let angle = 2.0 * PI / (1u64 << m) as f64;
                let cp = controlled_phase_gate(angle);
                result.apply_two_qubit_gate(k as u8, j as u8, &cp);
            }
        }

        // Bit-reversal via SWAP gates on the density matrix
        for i in 0..n / 2 {
            let j = n - 1 - i;
            if i != j {
                // SWAP gate
                let swap: [C64; 16] = [
                    C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
                    C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
                    C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
                    C64::ZERO, C64::ZERO, C64::ZERO, C64::ONE,
                ];
                result.apply_two_qubit_gate(i as u8, j as u8, &swap);
            }
        }

        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, CqamError> {
        // Factored QFT via Hadamard + controlled phase rotations.
        // O(n^2 * 2^n) time, O(2^n) memory -- no unitary matrix needed.
        let n = input.num_qubits() as usize;
        let dim = input.dimension();
        let mut amps = input.amplitudes().to_vec();

        for j in 0..n {
            // Apply Hadamard to qubit j
            let bit_j = 1 << (n - 1 - j);
            let h_inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
            if dim >= PAR_THRESHOLD {
                // Collect paired updates in parallel, then apply.
                let updates: Vec<(usize, C64, usize, C64)> = (0..dim)
                    .into_par_iter()
                    .filter(|&state| state & bit_j == 0)
                    .map(|state| {
                        let partner = state | bit_j;
                        let a = amps[state];
                        let b = amps[partner];
                        let new_s = C64(h_inv_sqrt2 * (a.0 + b.0), h_inv_sqrt2 * (a.1 + b.1));
                        let new_p = C64(h_inv_sqrt2 * (a.0 - b.0), h_inv_sqrt2 * (a.1 - b.1));
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
                        amps[state] = C64(h_inv_sqrt2 * (a.0 + b.0), h_inv_sqrt2 * (a.1 + b.1));
                        amps[partner] = C64(h_inv_sqrt2 * (a.0 - b.0), h_inv_sqrt2 * (a.1 - b.1));
                    }
                }
            }

            // Apply controlled phase rotations: CR_k for k = j+1..n-1
            for k in (j + 1)..n {
                let m = k - j; // distance determines rotation angle
                let angle = 2.0 * PI / (1u64 << m) as f64;
                let bit_k = 1 << (n - 1 - k);
                let phase = C64::exp_i(angle);
                // Apply phase only when both qubit j and qubit k are |1>
                let mask = bit_j | bit_k;
                if dim >= PAR_THRESHOLD {
                    amps.par_iter_mut().enumerate().for_each(|(state, amp)| {
                        if (state & mask) == mask {
                            *amp = phase * *amp;
                        }
                    });
                } else {
                    for (state, amp) in amps.iter_mut().enumerate() {
                        if (state & mask) == mask {
                            *amp = phase * *amp;
                        }
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
