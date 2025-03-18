// cqam-vm/src/simconfig.rs

#[derive(Debug, Clone)]
pub struct QuantumFidelityThreshold {
    pub min_superposition: f64,
    pub min_entanglement: f64,
}

impl Default for QuantumFidelityThreshold {
    fn default() -> Self {
        Self {
            min_superposition: 0.5,
            min_entanglement: 0.5,
        }
    }
}
