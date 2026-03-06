//! Unified quantum register that can hold either a pure statevector or a
//! mixed-state density matrix.
//!
//! The VM auto-promotes from Pure to Mixed when a mixed-state operation
//! is performed (partial trace, decoherence, explicit mixing).

use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;
use crate::statevector::Statevector;
use crate::complex::{cx_mul, cx_conj};
use cqam_core::error::CqamError;
use cqam_core::quantum_state::QuantumState;

/// A quantum register that can hold either a pure statevector or a
/// mixed-state density matrix.
#[derive(Debug, Clone)]
pub enum QuantumRegister {
    /// Pure state: efficient O(2^n) representation.
    Pure(Statevector),
    /// Mixed state: full O(4^n) density matrix.
    Mixed(DensityMatrix),
}

// =============================================================================
// Existing methods
// =============================================================================

impl QuantumRegister {
    /// Get the number of qubits.
    pub fn num_qubits(&self) -> u8 {
        match self {
            QuantumRegister::Pure(sv) => sv.num_qubits(),
            QuantumRegister::Mixed(dm) => dm.num_qubits(),
        }
    }

    /// Get the dimension.
    pub fn dimension(&self) -> usize {
        match self {
            QuantumRegister::Pure(sv) => sv.dimension(),
            QuantumRegister::Mixed(dm) => dm.dimension(),
        }
    }

    /// Get the purity.
    pub fn purity(&self) -> f64 {
        match self {
            QuantumRegister::Pure(_) => 1.0,
            QuantumRegister::Mixed(dm) => dm.purity(),
        }
    }

    /// Get diagonal probabilities.
    pub fn diagonal_probabilities(&self) -> Vec<f64> {
        match self {
            QuantumRegister::Pure(sv) => sv.diagonal_probabilities(),
            QuantumRegister::Mixed(dm) => dm.diagonal_probabilities(),
        }
    }

    /// Promote to density matrix (if not already).
    pub fn to_density_matrix(&self) -> DensityMatrix {
        match self {
            QuantumRegister::Pure(sv) => sv.to_density_matrix(),
            QuantumRegister::Mixed(dm) => dm.clone(),
        }
    }

    /// Get a density matrix element (row, col).
    /// For Statevector: computes psi[row] * conj(psi[col]) on the fly.
    pub fn get_element(&self, row: usize, col: usize) -> C64 {
        match self {
            QuantumRegister::Pure(sv) => {
                cx_mul(sv.amplitude(row), cx_conj(sv.amplitude(col)))
            }
            QuantumRegister::Mixed(dm) => dm.get(row, col),
        }
    }

    /// Alias for get_element, for compatibility with DensityMatrix::get.
    pub fn get(&self, row: usize, col: usize) -> C64 {
        self.get_element(row, col)
    }

    /// Check validity of the quantum state.
    /// For Pure: always valid (statevector is normalized by construction).
    /// For Mixed: delegates to DensityMatrix::is_valid.
    pub fn is_valid(&self, tolerance: f64) -> bool {
        match self {
            QuantumRegister::Pure(_) => true,
            QuantumRegister::Mixed(dm) => dm.is_valid(tolerance),
        }
    }
}

// =============================================================================
// Construction methods
// =============================================================================

impl QuantumRegister {
    /// Create the zero state |0...0>.
    /// When force_dm is true, creates Mixed(DensityMatrix) instead of Pure(Statevector).
    pub fn new_zero_state(num_qubits: u8, force_dm: bool) -> Self {
        if force_dm {
            QuantumRegister::Mixed(DensityMatrix::new_zero_state(num_qubits))
        } else {
            QuantumRegister::Pure(Statevector::new_zero_state(num_qubits))
        }
    }

    /// Create the uniform superposition state.
    pub fn new_uniform(num_qubits: u8, force_dm: bool) -> Self {
        if force_dm {
            QuantumRegister::Mixed(DensityMatrix::new_uniform(num_qubits))
        } else {
            QuantumRegister::Pure(Statevector::new_uniform(num_qubits))
        }
    }

    /// Create the Bell state (|00> + |11>)/sqrt(2).
    pub fn new_bell(force_dm: bool) -> Self {
        if force_dm {
            QuantumRegister::Mixed(DensityMatrix::new_bell())
        } else {
            QuantumRegister::Pure(Statevector::new_bell())
        }
    }

    /// Create the GHZ state (|0...0> + |1...1>)/sqrt(2).
    pub fn new_ghz(num_qubits: u8, force_dm: bool) -> Self {
        if force_dm {
            QuantumRegister::Mixed(DensityMatrix::new_ghz(num_qubits))
        } else {
            QuantumRegister::Pure(Statevector::new_ghz(num_qubits))
        }
    }

    /// Construct from an explicit amplitude vector.
    /// Always Pure (callers encode classical data into a quantum state).
    pub fn from_amplitudes(amplitudes: Vec<C64>) -> Result<Self, String> {
        let sv = Statevector::from_amplitudes(amplitudes)?;
        Ok(QuantumRegister::Pure(sv))
    }
}

// =============================================================================
// Gate application methods
// =============================================================================

impl QuantumRegister {
    /// Apply a single-qubit gate to a specific qubit.
    pub fn apply_single_qubit_gate(&mut self, target: u8, gate: &[C64; 4]) {
        match self {
            QuantumRegister::Pure(sv) => sv.apply_single_qubit_gate(target, gate),
            QuantumRegister::Mixed(dm) => dm.apply_single_qubit_gate(target, gate),
        }
    }

    /// Apply a two-qubit gate to specific control and target qubits.
    pub fn apply_two_qubit_gate(&mut self, ctrl: u8, tgt: u8, gate: &[C64; 16]) {
        match self {
            QuantumRegister::Pure(sv) => sv.apply_two_qubit_gate(ctrl, tgt, gate),
            QuantumRegister::Mixed(dm) => dm.apply_two_qubit_gate(ctrl, tgt, gate),
        }
    }

    /// Apply a full-register unitary transformation.
    pub fn apply_unitary(&mut self, unitary: &[C64]) {
        match self {
            QuantumRegister::Pure(sv) => sv.apply_unitary(unitary),
            QuantumRegister::Mixed(dm) => dm.apply_unitary(unitary),
        }
    }
}

// =============================================================================
// Measurement methods
// =============================================================================

impl QuantumRegister {
    /// Measure a single qubit, returning (outcome, post-measurement register).
    pub fn measure_qubit(&self, target: u8) -> (u8, QuantumRegister) {
        match self {
            QuantumRegister::Pure(sv) => {
                let (out, sv2) = sv.measure_qubit(target);
                (out, QuantumRegister::Pure(sv2))
            }
            QuantumRegister::Mixed(dm) => {
                let (out, dm2) = dm.measure_qubit(target);
                (out, QuantumRegister::Mixed(dm2))
            }
        }
    }
}

// =============================================================================
// Tensor product
// =============================================================================

impl QuantumRegister {
    /// Tensor product of two registers.
    /// (Pure, Pure) -> Pure; any Mixed -> Mixed.
    pub fn tensor_product(&self, other: &QuantumRegister) -> QuantumRegister {
        match (self, other) {
            (QuantumRegister::Pure(a), QuantumRegister::Pure(b)) => {
                QuantumRegister::Pure(a.tensor_product(b))
            }
            (QuantumRegister::Mixed(a), QuantumRegister::Mixed(b)) => {
                QuantumRegister::Mixed(a.tensor_product(b))
            }
            (QuantumRegister::Pure(a), QuantumRegister::Mixed(b)) => {
                QuantumRegister::Mixed(a.to_density_matrix().tensor_product(b))
            }
            (QuantumRegister::Mixed(a), QuantumRegister::Pure(b)) => {
                QuantumRegister::Mixed(a.tensor_product(&b.to_density_matrix()))
            }
        }
    }
}

// =============================================================================
// Mixed-state specific (auto-promotion triggers)
// =============================================================================

impl QuantumRegister {
    /// Partial trace over subsystem B. Result is always Mixed.
    pub fn partial_trace_b(&self, num_qubits_a: u8) -> QuantumRegister {
        match self {
            QuantumRegister::Pure(sv) => {
                let dm = sv.to_density_matrix();
                QuantumRegister::Mixed(dm.partial_trace_b(num_qubits_a))
            }
            QuantumRegister::Mixed(dm) => {
                QuantumRegister::Mixed(dm.partial_trace_b(num_qubits_a))
            }
        }
    }

    /// Ensure this register is in Mixed representation.
    /// If Pure, promotes to Mixed(sv.to_density_matrix()).
    /// If already Mixed, no-op.
    pub fn ensure_mixed(&mut self) {
        if let QuantumRegister::Pure(sv) = self {
            *self = QuantumRegister::Mixed(sv.to_density_matrix());
        }
    }
}

// =============================================================================
// Kernel application
// =============================================================================

impl QuantumRegister {
    /// Apply a kernel, using the statevector fast path when possible.
    ///
    /// Returns a new QuantumRegister (not in-place, matches Kernel::apply
    /// return signature).
    pub fn apply_kernel(&self, kernel: &dyn Kernel) -> Result<QuantumRegister, CqamError> {
        match self {
            QuantumRegister::Pure(sv) => {
                match kernel.apply_sv(sv) {
                    Ok(sv2) => Ok(QuantumRegister::Pure(sv2)),
                    Err(_) => {
                        // Kernel doesn't support SV mode; promote and retry.
                        let dm = sv.to_density_matrix();
                        let dm2 = kernel.apply(&dm)?;
                        Ok(QuantumRegister::Mixed(dm2))
                    }
                }
            }
            QuantumRegister::Mixed(dm) => {
                let dm2 = kernel.apply(dm)?;
                Ok(QuantumRegister::Mixed(dm2))
            }
        }
    }
}

// =============================================================================
// QuantumState trait implementation (required for QMem<QuantumRegister>)
// =============================================================================

impl QuantumState for QuantumRegister {
    fn num_qubits(&self) -> u8 {
        QuantumRegister::num_qubits(self)
    }

    fn dimension(&self) -> usize {
        QuantumRegister::dimension(self)
    }

    fn diagonal_probabilities(&self) -> Vec<f64> {
        QuantumRegister::diagonal_probabilities(self)
    }

    fn purity(&self) -> f64 {
        QuantumRegister::purity(self)
    }
}
