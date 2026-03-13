//! Jacobi eigenvalue decomposition for complex Hermitian matrices.

use crate::complex::{C64, cx_add, cx_conj, cx_norm_sq, cx_scale, cx_mul};
use crate::constants::PAR_THRESHOLD;
use rayon::prelude::*;

/// Jacobi eigenvalue algorithm for complex Hermitian matrices.
///
/// Given a dim x dim Hermitian matrix (stored as flat Vec<C64> in row-major),
/// computes all eigenvalues by iterative Jacobi rotations that zero out
/// off-diagonal elements.
///
/// Returns eigenvalues sorted in descending order, with negative values
/// (from floating-point error) clamped to 0.
pub(super) fn jacobi_eigenvalues(
    matrix: &mut [C64],
    dim: usize,
    tolerance: f64,
    max_sweeps: usize,
) -> Vec<f64> {
    if dim == 0 {
        return Vec::new();
    }
    if dim == 1 {
        return vec![matrix[0].0.max(0.0)];
    }

    for _sweep in 0..max_sweeps {
        // Find the off-diagonal element with largest magnitude
        let (max_val, p, q) = if dim >= PAR_THRESHOLD {
            (0..dim).into_par_iter().map(|i| {
                let mut local_max = 0.0_f64;
                let mut local_p = i;
                let mut local_q = if i + 1 < dim { i + 1 } else { i };
                for j in (i + 1)..dim {
                    let mag = cx_norm_sq(matrix[i * dim + j]);
                    if mag > local_max {
                        local_max = mag;
                        local_p = i;
                        local_q = j;
                    }
                }
                (local_max, local_p, local_q)
            }).reduce(|| (0.0_f64, 0, 1), |a, b| if a.0 >= b.0 { a } else { b })
        } else {
            let mut max_val = 0.0_f64;
            let mut p = 0;
            let mut q = 1;
            for i in 0..dim {
                for j in (i + 1)..dim {
                    let mag = cx_norm_sq(matrix[i * dim + j]);
                    if mag > max_val {
                        max_val = mag;
                        p = i;
                        q = j;
                    }
                }
            }
            (max_val, p, q)
        };

        // Check convergence
        if max_val.sqrt() < tolerance {
            break;
        }

        // Apply Jacobi rotation to zero out element (p, q).
        // For Hermitian matrix: a_pq = conj(a_qp)
        let app = matrix[p * dim + p].0; // real (diagonal)
        let aqq = matrix[q * dim + q].0; // real (diagonal)
        let apq = matrix[p * dim + q];   // complex off-diagonal

        let apq_mag = cx_norm_sq(apq).sqrt();
        if apq_mag < 1e-30 {
            continue;
        }

        // Phase factor: apq = |apq| * e^{i*phi}
        let phase = (apq.0 / apq_mag, apq.1 / apq_mag); // e^{i*phi}
        let phase_conj = cx_conj(phase);                  // e^{-i*phi}

        // Now compute the 2x2 real Jacobi rotation for the matrix
        // [[app, |apq|], [|apq|, aqq]]
        let tau = (aqq - app) / (2.0 * apq_mag);
        let t = if tau.abs() < 1e-30 {
            1.0 // tau ~ 0, so t = 1
        } else {
            let sign = if tau >= 0.0 { 1.0 } else { -1.0 };
            sign / (tau.abs() + (1.0 + tau * tau).sqrt())
        };

        let c = 1.0 / (1.0 + t * t).sqrt(); // cos(theta)
        let s = t * c;                        // sin(theta)

        // The complex rotation is:
        // G[p,p] = c,  G[p,q] = s * e^{i*phi},  G[q,p] = -s * e^{-i*phi},  G[q,q] = c
        // After rotation: A' = G^H A G

        // Update diagonal elements
        matrix[p * dim + p] = (app - t * apq_mag, 0.0);
        matrix[q * dim + q] = (aqq + t * apq_mag, 0.0);
        // Zero out (p,q) and (q,p)
        matrix[p * dim + q] = (0.0, 0.0);
        matrix[q * dim + p] = (0.0, 0.0);

        // Update the rest of rows/columns p and q
        if dim >= PAR_THRESHOLD {
            let updates: Vec<(usize, C64, C64)> = (0..dim).into_par_iter()
                .filter(|&r| r != p && r != q)
                .map(|r| {
                    let arp = matrix[r * dim + p];
                    let arq = matrix[r * dim + q];
                    let new_rp = cx_add(
                        cx_scale(c, arp),
                        cx_scale(-s, cx_mul(phase_conj, arq)),
                    );
                    let new_rq = cx_add(
                        cx_scale(s, cx_mul(phase, arp)),
                        cx_scale(c, arq),
                    );
                    (r, new_rp, new_rq)
                }).collect();
            for (r, new_rp, new_rq) in updates {
                matrix[r * dim + p] = new_rp;
                matrix[r * dim + q] = new_rq;
                matrix[p * dim + r] = cx_conj(new_rp);
                matrix[q * dim + r] = cx_conj(new_rq);
            }
        } else {
            for r in 0..dim {
                if r == p || r == q {
                    continue;
                }
                let arp = matrix[r * dim + p];
                let arq = matrix[r * dim + q];
                let new_rp = cx_add(
                    cx_scale(c, arp),
                    cx_scale(-s, cx_mul(phase_conj, arq)),
                );
                let new_rq = cx_add(
                    cx_scale(s, cx_mul(phase, arp)),
                    cx_scale(c, arq),
                );
                matrix[r * dim + p] = new_rp;
                matrix[r * dim + q] = new_rq;
                matrix[p * dim + r] = cx_conj(new_rp);
                matrix[q * dim + r] = cx_conj(new_rq);
            }
        }
    }

    // Extract diagonal as eigenvalues, clamp negative, sort descending
    let mut eigenvalues: Vec<f64> = (0..dim)
        .map(|i| matrix[i * dim + i].0.max(0.0))
        .collect();
    eigenvalues.sort_by(|a, b| b.partial_cmp(a).unwrap());
    eigenvalues
}
