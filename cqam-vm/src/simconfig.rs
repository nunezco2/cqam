// cqam-vm/src/simconfig.rs

#[derive(Debug, Clone)]
pub struct QuantumFidelityThreshold {
    pub min_superposition: f64,
    pub min_entanglement: f64,
}

impl Default for QuantumFidelityThreshold {
    fn default() -> Self {
        Self {
            // Default to 0.0 (disabled) because the entanglement_metric
            // measures distribution concentration, not physical entanglement.
            // Normal quantum operations (Fourier, Grover) legitimately
            // concentrate distributions and would trigger false interrupts
            // at higher thresholds. Programs that want fidelity-based halting
            // should set explicit thresholds via configuration.
            min_superposition: 0.0,
            min_entanglement: 0.0,
        }
    }
}
