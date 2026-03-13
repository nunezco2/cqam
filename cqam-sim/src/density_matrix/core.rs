//! Core struct definition and element access methods for `DensityMatrix`.

use crate::complex::C64;

/// A density matrix representing an n-qubit quantum state.
///
/// Invariants (maintained by all public constructors and operations):
/// - `data.len() == dim * dim` where `dim = 2^num_qubits`
/// - `Tr(rho) = 1.0` (within floating-point tolerance)
/// - `rho` is Hermitian: `rho[i][j] = conj(rho[j][i])`
/// - `rho` is positive semi-definite
#[derive(Debug, Clone)]
pub struct DensityMatrix {
    pub(super) num_qubits: u8,
    pub(super) data: Vec<C64>,
}

// =============================================================================
// Element Access
// =============================================================================

impl DensityMatrix {
    /// Get the element at row i, column j.
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> C64 {
        let dim = self.dimension();
        self.data[i * dim + j]
    }

    /// Set the element at row i, column j.
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, val: C64) {
        let dim = self.dimension();
        self.data[i * dim + j] = val;
    }

    /// Number of qubits.
    #[inline]
    pub fn num_qubits(&self) -> u8 {
        self.num_qubits
    }

    /// Hilbert space dimension: 2^num_qubits.
    #[inline]
    pub fn dimension(&self) -> usize {
        1 << self.num_qubits
    }
}
