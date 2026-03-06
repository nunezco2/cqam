//! Quantum kernel trait for unitary density-matrix transformations.
//!
//! Each `Kernel` implementation constructs its unitary matrix U and applies
//! the transformation rho' = U rho U†. Kernels operate on the full register;
//! partial (qubit-selective) application is not currently supported.

use cqam_core::error::CqamError;
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;

/// A quantum kernel that transforms a density matrix via unitary evolution.
///
/// Each kernel constructs its unitary matrix U and applies rho' = U rho U^dagger.
/// Kernels operate on the full register (all qubits); partial application is not
/// supported (no qubit-level addressing within a register).
pub trait Kernel {
    /// Apply this kernel's unitary transformation to the input density matrix.
    ///
    /// # Errors
    ///
    /// Returns `Err(CqamError::TypeMismatch)` if the kernel parameters are
    /// incompatible with the input state (e.g., Grover target >= dimension,
    /// Entangle on a 1-qubit register).
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError>;

    /// Apply to a statevector (pure-state fast path).
    ///
    /// Default implementation returns an error indicating the kernel does not
    /// support statevector mode. Kernels that can operate on pure states
    /// should override this.
    fn apply_sv(&self, _input: &Statevector) -> Result<Statevector, String> {
        Err("kernel does not support statevector mode".to_string())
    }
}
