//! Quantum kernel trait for unitary density-matrix transformations.
//!
//! Each `Kernel` implementation constructs its unitary matrix U and applies
//! the transformation rho' = U rho U†. Kernels operate on the full register;
//! partial (qubit-selective) application is not currently supported.

use crate::density_matrix::DensityMatrix;

/// A quantum kernel that transforms a density matrix via unitary evolution.
///
/// Each kernel constructs its unitary matrix U and applies rho' = U rho U^dagger.
/// Kernels operate on the full register (all qubits); partial application is not
/// supported (no qubit-level addressing within a register).
pub trait Kernel {
    /// Apply this kernel's unitary transformation to the input density matrix.
    fn apply(&self, input: &DensityMatrix) -> DensityMatrix;
}
