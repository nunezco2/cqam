//! Diagonal rotation kernel (kernel_id = 5).
//!
//! Constructs a diagonal unitary where each basis state |k> acquires a
//! k-dependent phase: U[k][k] = exp(i * theta * k). Off-diagonal entries
//! are zero.
//!
//! Properties:
//! - Unitary: U^dagger U = I (diagonal with unit-modulus entries).
//! - Preserves diagonal probabilities (|U[k][k]|^2 = 1 for all k).
//! - Modifies off-diagonal coherences of the density matrix.
//! - theta = 0 => U = I (identity).
//! - theta = 2*pi/dim => primitive dim-th root of unity ramp.

use cqam_core::error::CqamError;
use crate::complex::{self, cx_exp_i, cx_mul};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use rayon::prelude::*;

const PAR_THRESHOLD: usize = 256;

/// Diagonal rotation kernel parameterized by angle theta.
///
/// Constructed by the QKERNELF executor with `theta = F[fctx0]`.
pub struct Rotate {
    /// Rotation angle in radians. No range restriction; exp(i*theta)
    /// naturally wraps around with period 2*pi.
    pub theta: f64,
}

impl Kernel for Rotate {
    /// Apply the diagonal rotation: rho' = U rho U^dagger.
    ///
    /// Constructs U as a dim x dim diagonal matrix with
    /// U[k][k] = cx_exp_i(self.theta * k), then delegates to
    /// DensityMatrix::apply_unitary.
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let dim = input.dimension();
        let mut unitary = vec![complex::ZERO; dim * dim];
        for k in 0..dim {
            let angle = self.theta * (k as f64);
            unitary[k * dim + k] = cx_exp_i(angle);
        }
        let mut result = input.clone();
        result.apply_unitary(&unitary);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, String> {
        let dim = input.dimension();
        let amps = input.amplitudes();
        let theta = self.theta;
        let result_amps: Vec<_> = if dim >= PAR_THRESHOLD {
            amps.par_iter().enumerate().map(|(k, &amp)| {
                let phase = cx_exp_i(theta * (k as f64));
                cx_mul(phase, amp)
            }).collect()
        } else {
            amps.iter().enumerate().map(|(k, &amp)| {
                let phase = cx_exp_i(theta * (k as f64));
                cx_mul(phase, amp)
            }).collect()
        };
        Ok(Statevector::from_amplitudes(result_amps)
            .expect("Rotate apply_sv produced invalid amplitudes"))
    }
}
