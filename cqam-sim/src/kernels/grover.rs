//! Grover iteration kernel: one oracle phase-flip followed by diffusion.

use crate::complex;
use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;

/// One Grover iteration kernel (kernel_id = 4).
///
/// G = D * O where:
/// - Oracle O: diagonal with -1 at target state, +1 elsewhere
/// - Diffusion D: 2|s><s| - I
///
/// Combined: G[j][k] = (2/N - delta_{j,k}) * (if k == target { -1 } else { 1 })
pub struct GroverIter {
    /// The marked (target) basis state whose phase is flipped by the oracle.
    pub target: u16,
}

impl Kernel for GroverIter {
    fn apply(&self, input: &DensityMatrix) -> DensityMatrix {
        let dim = input.dimension();
        let t = self.target as usize;
        assert!(t < dim, "Grover target {} exceeds dimension {}", t, dim);
        let n_f64 = dim as f64;

        // Compose G = D * O
        // D[j][m] = 2/N - delta_{j,m}
        // O[m][k] = delta_{m,k} * (if k == t { -1 } else { 1 })
        // G[j][k] = D[j][k] * O[k][k]  (since O is diagonal)
        let mut g = vec![complex::ZERO; dim * dim];
        for j in 0..dim {
            for k in 0..dim {
                let d_jk = 2.0 / n_f64 - if j == k { 1.0 } else { 0.0 };
                let o_kk = if k == t { -1.0 } else { 1.0 };
                g[j * dim + k] = (d_jk * o_kk, 0.0);
            }
        }

        let mut result = input.clone();
        result.apply_unitary(&g);
        result
    }
}
