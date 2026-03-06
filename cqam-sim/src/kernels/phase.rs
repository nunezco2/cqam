//! Phase shift kernel (kernel_id = 6).
//!
//! Constructs a diagonal unitary where the phase ramp rate is determined
//! by the modulus of a complex parameter: U[k][k] = exp(i * |z| * k).
//!
//! Properties:
//! - Identical structure to Rotate, but the angle is |z| = sqrt(re^2 + im^2).
//! - When z = (theta, 0.0), this is equivalent to Rotate{theta}.
//! - Preserves diagonal probabilities.

use cqam_core::error::CqamError;
use crate::complex::{cx_exp_i, cx_mul, cx_norm};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use rayon::prelude::*;

const PAR_THRESHOLD: usize = 256;

/// Phase shift kernel parameterized by a complex amplitude.
///
/// Constructed by the QKERNELZ executor with `amplitude = Z[zctx0]`.
/// The effective rotation angle is the modulus |amplitude|.
pub struct PhaseShift {
    /// Complex amplitude from the Z-file. The kernel uses |amplitude|
    /// as the phase ramp rate per basis state.
    pub amplitude: (f64, f64),
}

impl Kernel for PhaseShift {
    /// Apply the phase shift: rho' = U rho U^dagger.
    ///
    /// Computes angle = cx_norm(self.amplitude), then constructs U as a
    /// dim x dim diagonal matrix with U[k][k] = cx_exp_i(angle * k),
    /// and delegates to DensityMatrix::apply_unitary.
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let rate = cx_norm(self.amplitude);
        let dim = input.dimension();
        let phases: Vec<_> = (0..dim).map(|k| cx_exp_i(rate * (k as f64))).collect();
        let mut result = input.clone();
        result.apply_diagonal_unitary(&phases);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, String> {
        let dim = input.dimension();
        let rate = cx_norm(self.amplitude);
        let amps = input.amplitudes();
        let result_amps: Vec<_> = if dim >= PAR_THRESHOLD {
            amps.par_iter().enumerate().map(|(k, &amp)| {
                let phase = cx_exp_i(rate * (k as f64));
                cx_mul(phase, amp)
            }).collect()
        } else {
            amps.iter().enumerate().map(|(k, &amp)| {
                let phase = cx_exp_i(rate * (k as f64));
                cx_mul(phase, amp)
            }).collect()
        };
        Ok(Statevector::from_amplitudes(result_amps)
            .expect("PhaseShift apply_sv produced invalid amplitudes"))
    }
}
