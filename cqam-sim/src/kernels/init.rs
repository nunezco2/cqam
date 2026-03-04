// cqam-sim/src/kernels/init.rs
//
// Phase 2: Initialization kernel operating on DensityMatrix.

use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;

/// Initialization kernel: produces the uniform superposition state H^n|0>.
///
/// Ignores the input density matrix's state; always returns the equal
/// superposition pure state for the same number of qubits.
pub struct Init;

impl Kernel for Init {
    fn apply(&self, input: &DensityMatrix) -> DensityMatrix {
        DensityMatrix::new_uniform(input.num_qubits())
    }
}
