//! Grover iteration kernel: one oracle phase-flip followed by diffusion.

use cqam_core::error::CqamError;
use crate::complex::{self, C64, cx_add, cx_scale};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use rayon::prelude::*;

const PAR_THRESHOLD: usize = 256;

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
    /// The primary marked (target) basis state whose phase is flipped by the oracle.
    pub target: u16,
    /// Additional marked basis states (for multi-target mode).
    pub extra_targets: Vec<u16>,
}

impl GroverIter {
    /// Single-target constructor (backward compatible).
    pub fn single(target: u16) -> Self {
        GroverIter { target, extra_targets: Vec::new() }
    }

    /// Multi-target constructor.
    pub fn multi(targets: Vec<u16>) -> Self {
        assert!(!targets.is_empty(), "multi-target Grover requires at least one target");
        GroverIter {
            target: targets[0],
            extra_targets: targets[1..].to_vec(),
        }
    }

    /// Return all target states.
    pub fn all_targets(&self) -> Vec<u16> {
        let mut all = vec![self.target];
        all.extend_from_slice(&self.extra_targets);
        all
    }
}

impl Kernel for GroverIter {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        // Validate and build target set for O(1) lookup
        let mut target_set = std::collections::HashSet::new();
        for &t in &self.all_targets() {
            let t_usize = t as usize;
            if t_usize >= dim {
                return Err(CqamError::TypeMismatch {
                    instruction: "QKERNEL/GROVER".to_string(),
                    detail: format!(
                        "Grover target {} exceeds dimension {}",
                        t_usize, dim
                    ),
                });
            }
            target_set.insert(t_usize);
        }

        // Compose G = D * O
        let mut g = vec![complex::ZERO; dim * dim];
        for j in 0..dim {
            for k in 0..dim {
                let d_jk = 2.0 / n_f64 - if j == k { 1.0 } else { 0.0 };
                let o_kk = if target_set.contains(&k) { -1.0 } else { 1.0 };
                g[j * dim + k] = (d_jk * o_kk, 0.0);
            }
        }

        let mut result = input.clone();
        result.apply_unitary(&g);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, String> {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        // Validate targets
        let target_set: std::collections::HashSet<usize> =
            self.all_targets().iter().map(|&t| {
                let t_usize = t as usize;
                if t_usize >= dim {
                    panic!("Grover target {} exceeds dimension {}", t_usize, dim);
                }
                t_usize
            }).collect();

        // Step 1: Oracle - flip sign of target amplitudes (O(dim))
        let mut amps: Vec<C64> = input.amplitudes().to_vec();
        if dim >= PAR_THRESHOLD {
            amps.par_iter_mut().enumerate().for_each(|(k, amp)| {
                if target_set.contains(&k) {
                    *amp = (-amp.0, -amp.1);
                }
            });
        } else {
            for &t in &target_set {
                amps[t] = (-amps[t].0, -amps[t].1);
            }
        }

        // Step 2: Diffusion - D|psi> = 2*mean - psi_k (O(dim))
        let mean = if dim >= PAR_THRESHOLD {
            let sum = amps.par_iter().copied().reduce(|| complex::ZERO, cx_add);
            cx_scale(1.0 / n_f64, sum)
        } else {
            let mut m = complex::ZERO;
            for amp in amps.iter().take(dim) {
                m = cx_add(m, *amp);
            }
            cx_scale(1.0 / n_f64, m)
        };

        let two_mean = cx_scale(2.0, mean);
        if dim >= PAR_THRESHOLD {
            amps = amps.par_iter().map(|amp| {
                (two_mean.0 - amp.0, two_mean.1 - amp.1)
            }).collect();
        } else {
            for amp in amps.iter_mut().take(dim) {
                *amp = (two_mean.0 - amp.0, two_mean.1 - amp.1);
            }
        }

        Ok(Statevector::from_amplitudes(amps)
            .expect("GroverIter apply_sv produced invalid amplitudes"))
    }
}
