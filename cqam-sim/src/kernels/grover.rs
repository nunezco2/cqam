//! Grover iteration kernel: one oracle phase-flip followed by diffusion.

use crate::complex;
use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;

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
    fn apply(&self, input: &DensityMatrix) -> DensityMatrix {
        let dim = input.dimension();
        let n_f64 = dim as f64;

        // Build target set for O(1) lookup
        let target_set: std::collections::HashSet<usize> =
            self.all_targets().iter().map(|&t| {
                let t_usize = t as usize;
                assert!(t_usize < dim, "Grover target {} exceeds dimension {}", t_usize, dim);
                t_usize
            }).collect();

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
        result
    }
}
