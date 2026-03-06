//! Initialization kernel: produces the uniform superposition state H^n|0>.

use cqam_core::error::CqamError;
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;

/// Initialization kernel: produces the uniform superposition state H^n|0>.
///
/// Ignores the input density matrix's state; always returns the equal
/// superposition pure state for the same number of qubits.
pub struct Init;

impl Kernel for Init {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        Ok(DensityMatrix::new_uniform(input.num_qubits()))
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, String> {
        Ok(Statevector::new_uniform(input.num_qubits()))
    }
}
