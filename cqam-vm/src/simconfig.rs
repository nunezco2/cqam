//! Quantum fidelity thresholds for interrupt generation.
//!
//! `QuantumFidelityThreshold` configures the minimum purity value below which
//! a quantum-error interrupt is raised, and the default number of qubits per
//! quantum register.

/// Fidelity thresholds that govern quantum-error interrupt generation.
///
/// After each `QKERNEL` or `QOBSERVE`, the VM computes the purity Tr(rho^2)
/// of the affected register. If purity falls below `min_purity`,
/// `int_quantum_err` is set in the PSW.
#[derive(Debug, Clone)]
pub struct QuantumFidelityThreshold {
    /// Minimum acceptable purity Tr(rho^2).
    ///
    /// When purity drops below this value after a quantum operation, the VM
    /// raises int_quantum_err. A value of 0.0 disables the check.
    /// Default: 0.0 (disabled; the runner wires SimConfig::fidelity_threshold).
    pub min_purity: f64,

    /// Default number of qubits per quantum register for `QPREP`.
    ///
    /// This value is used when no kernel-level qubit count is specified.
    /// Default: 2 (4-state distribution).
    pub default_qubits: u8,

    /// Force use of the density-matrix backend for all quantum registers.
    ///
    /// When `true`, `QPREP` always allocates a full density matrix even when
    /// a statevector would suffice. Default: `false`.
    pub force_density_matrix: bool,
}

impl Default for QuantumFidelityThreshold {
    fn default() -> Self {
        Self {
            min_purity: 0.0,
            default_qubits: 2, // backward compatible with current 4-state tests
            force_density_matrix: false,
        }
    }
}
