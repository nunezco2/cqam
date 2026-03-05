//! Abstract quantum state trait for QMEM decoupling.
//!
//! This trait lives in `cqam-core` so that `QMem<Q: QuantumState>` can be
//! defined without importing the concrete `DensityMatrix` from `cqam-sim`.
//! The `DensityMatrix` implementation of this trait lives in `cqam-sim`.

/// Trait abstracting the quantum state stored in QMEM slots.
///
/// # Bound rationale
///
/// - `Debug`: required because `QMem` derives `Debug`.
/// - `Clone`: required because `QLoad` clones a QMEM slot into a Q register,
///   and `QStore` clones a Q register into QMEM.
///
/// # Implementors
///
/// - `cqam_sim::density_matrix::DensityMatrix` (the production implementation)
/// - Test mocks may implement this with trivial logic.
pub trait QuantumState: std::fmt::Debug + Clone {
    /// Number of qubits this state represents.
    fn num_qubits(&self) -> u8;

    /// Hilbert space dimension: 2^num_qubits.
    fn dimension(&self) -> usize;

    /// Return the diagonal measurement probabilities.
    ///
    /// The k-th entry is the Born-rule probability of measuring basis state |k>.
    /// The returned Vec has length `self.dimension()`.
    fn diagonal_probabilities(&self) -> Vec<f64>;

    /// Purity metric: Tr(rho^2) for density matrices, 1.0 for pure states.
    ///
    /// Range: [1/dim, 1.0] where dim = 2^num_qubits.
    /// - 1.0 means pure state.
    /// - 1/dim means maximally mixed.
    fn purity(&self) -> f64;
}
