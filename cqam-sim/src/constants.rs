//! Centralized constants for the cqam-sim simulation crate.

// Re-export PAR_THRESHOLD from cqam-core (single source of truth).
pub use cqam_core::constants::PAR_THRESHOLD;

/// Tolerance for entanglement detection via single-qubit reduced purity.
/// If any qubit's reduced purity falls below `1.0 - EF_EPSILON`, the state
/// is considered entangled.
pub const EF_EPSILON: f64 = 1e-10;

/// Tolerance for superposition detection: amplitudes (or diagonal elements)
/// with probability below this threshold are treated as zero.
pub const SF_EPSILON: f64 = 1e-12;

/// Maximum number of qubits supported by the full density-matrix backend.
pub const MAX_QUBITS: u8 = 16;

/// Maximum number of qubits supported by the statevector backend.
pub const MAX_SV_QUBITS: u8 = 24;
