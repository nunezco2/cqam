//! Grover iteration kernel: one oracle phase-flip followed by diffusion.

use cqam_core::error::CqamError;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use rayon::prelude::*;

use crate::constants::PAR_THRESHOLD;

/// One Grover iteration kernel (kernel_id = 4).
///
/// G = D * O where:
/// - Oracle O: diagonal with -1 at each target state, +1 elsewhere
/// - Diffusion D: 2|s><s| - I
///
/// When `extra_targets` is empty, behaves as a standard single-target
/// Grover iteration. When `extra_targets` contains additional values,
/// applies the oracle phase flip to ALL marked states simultaneously.
pub struct GroverIter {
    /// All marked basis states, precomputed as a set for O(1) lookup.
    target_set: std::collections::HashSet<usize>,
    /// All marked basis states as a vec (for external access).
    targets: Vec<u16>,
}

impl GroverIter {
    /// Single-target constructor (backward compatible).
    pub fn single(target: u16) -> Self {
        let targets = vec![target];
        let target_set = targets.iter().map(|&t| t as usize).collect();
        GroverIter { target_set, targets }
    }

    /// Multi-target constructor.
    pub fn multi(targets: Vec<u16>) -> Self {
        assert!(!targets.is_empty(), "multi-target Grover requires at least one target");
        let target_set = targets.iter().map(|&t| t as usize).collect();
        GroverIter { target_set, targets }
    }

    /// Return all target states.
    pub fn all_targets(&self) -> &[u16] {
        &self.targets
    }
}

impl Kernel for GroverIter {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        // Validate targets against dimension
        for &t in &self.targets {
            if (t as usize) >= dim {
                return Err(CqamError::TypeMismatch {
                    instruction: "QKERNEL/GROVER".to_string(),
                    detail: format!(
                        "Grover target {} exceeds dimension {}",
                        t, dim
                    ),
                });
            }
        }
        let target_set = &self.target_set;

        // Compose G = D * O
        let mut g = vec![C64::ZERO; dim * dim];
        for j in 0..dim {
            for k in 0..dim {
                let d_jk = 2.0 / n_f64 - if j == k { 1.0 } else { 0.0 };
                let o_kk = if target_set.contains(&k) { -1.0 } else { 1.0 };
                g[j * dim + k] = C64(d_jk * o_kk, 0.0);
            }
        }

        let mut result = input.clone();
        result.apply_unitary(&g);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, CqamError> {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        // Validate targets
        for &t in &self.targets {
            if (t as usize) >= dim {
                return Err(CqamError::TypeMismatch {
                    instruction: "QKERNEL/GROVER".to_string(),
                    detail: format!("Grover target {} exceeds dimension {}", t, dim),
                });
            }
        }
        let target_set = &self.target_set;

        // Step 1: Oracle - flip sign of target amplitudes (O(dim))
        let mut amps: Vec<C64> = input.amplitudes().to_vec();
        if dim >= PAR_THRESHOLD {
            amps.par_iter_mut().enumerate().for_each(|(k, amp)| {
                if target_set.contains(&k) {
                    *amp = -*amp;
                }
            });
        } else {
            for &t in target_set {
                amps[t] = -amps[t];
            }
        }

        // Step 2: Diffusion - D|psi> = 2*mean - psi_k (O(dim))
        let mean = if dim >= PAR_THRESHOLD {
            let sum = amps.par_iter().copied().reduce(|| C64::ZERO, |a, b| a + b);
            sum.scale(1.0 / n_f64)
        } else {
            let mut m = C64::ZERO;
            for amp in amps.iter().take(dim) {
                m += *amp;
            }
            m.scale(1.0 / n_f64)
        };

        let two_mean = mean.scale(2.0);
        if dim >= PAR_THRESHOLD {
            amps = amps.par_iter().map(|amp| {
                two_mean - *amp
            }).collect();
        } else {
            for amp in amps.iter_mut().take(dim) {
                *amp = two_mean - *amp;
            }
        }

        Ok(Statevector::from_amplitudes(amps)
            .expect("GroverIter apply_sv produced invalid amplitudes"))
    }
}
