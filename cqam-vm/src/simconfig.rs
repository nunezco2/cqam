//! Quantum fidelity thresholds for interrupt generation.
//!
//! `QuantumFidelityThreshold` configures the minimum superposition and
//! entanglement values below which a quantum-error interrupt is raised,
//! and the default number of qubits per quantum register.

#[derive(Debug, Clone)]
pub struct QuantumFidelityThreshold {
    pub min_superposition: f64,
    pub min_entanglement: f64,
    /// Default number of qubits per quantum register.
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
