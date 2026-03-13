//! Measurement methods for `DensityMatrix`.

use super::DensityMatrix;
use crate::complex::C64;
use crate::constants::PAR_THRESHOLD;
use rand::Rng;
use rayon::prelude::*;

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
    pub fn measure_qubit_with_rng(&self, target: u8, rng: &mut impl Rng) -> (u8, DensityMatrix) {
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
                    result.data[i * dim + j] = C64::ZERO;
                }
            }
        }

        // Renormalize: rho' = projected / p(outcome)
        if p_outcome > 1e-30 {
            let inv_p = 1.0 / p_outcome;
            for entry in result.data.iter_mut() {
                *entry = entry.scale(inv_p);
            }
        }

        (outcome, result)
    }

    /// Measure a single qubit using thread-local RNG (non-reproducible).
    pub fn measure_qubit(&self, target: u8) -> (u8, DensityMatrix) {
        self.measure_qubit_with_rng(target, &mut rand::thread_rng())
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
            *entry = C64::ZERO;
        }
        collapsed.data[outcome * dim + outcome] = C64::ONE;

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
