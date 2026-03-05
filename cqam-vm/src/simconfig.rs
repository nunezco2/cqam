//! Quantum fidelity thresholds for interrupt generation.
//!
//! `QuantumFidelityThreshold` configures the minimum superposition and
//! entanglement values below which a quantum-error interrupt is raised,
//! and the default number of qubits per quantum register.

/// Fidelity thresholds that govern quantum-error interrupt generation.
///
/// After each `QKERNEL` or `QOBSERVE`, the VM computes the superposition and
/// entanglement metrics of the affected register. If either metric falls below
/// the corresponding threshold, `int_quantum_err` is set in the PSW.
#[derive(Debug, Clone)]
pub struct QuantumFidelityThreshold {
    /// Minimum acceptable superposition metric (0.0 = disabled).
    ///
    /// The superposition metric is the normalised Shannon entropy of the
    /// diagonal probability distribution: H(rho_diag) / log2(dim).
    pub min_superposition: f64,

    /// Minimum acceptable entanglement metric (0.0 = disabled).
    ///
    /// The entanglement metric is the von Neumann entropy of the reduced
    /// density matrix obtained by tracing out the second qubit subsystem.
    pub min_entanglement: f64,

    /// Default number of qubits per quantum register for `QPREP`.
    ///
    /// This value is used when no kernel-level qubit count is specified.
    /// Default: 2 (4-state distribution).
    pub default_qubits: u8,
}

impl Default for QuantumFidelityThreshold {
    fn default() -> Self {
        Self {
            min_superposition: 0.0,
            min_entanglement: 0.0,
            default_qubits: 2, // backward compatible with current 4-state tests
        }
    }
}
