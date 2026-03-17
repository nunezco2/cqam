//! Error types for the cqam-micro compilation pipeline.

use std::fmt;
use cqam_core::error::CqamError;

/// Errors that can occur during micro-compilation (decomposition, routing,
/// native mapping, optimization, or caching).
#[derive(Debug)]
pub enum MicroError {
    /// A kernel could not be decomposed into the standard gate set.
    DecompositionFailed { kernel: String, detail: String },

    /// Routing could not place a two-qubit gate on adjacent physical qubits
    /// within the allowed SWAP budget.
    RoutingFailed { detail: String },

    /// A gate type has no mapping in the target native gate set.
    UnsupportedGate { gate: String },

    /// Wire index references a qubit outside the program's wire count.
    InvalidWire { wire: u32, num_wires: u32 },

    /// A parameter required for decomposition was symbolic (unresolved).
    UnresolvedParam { context: String },

    /// Cache key collision or structural mismatch during rebinding.
    CacheError { detail: String },
}

impl fmt::Display for MicroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MicroError::DecompositionFailed { kernel, detail } =>
                write!(f, "decomposition of kernel {} failed: {}", kernel, detail),
            MicroError::RoutingFailed { detail } =>
                write!(f, "routing failed: {}", detail),
            MicroError::UnsupportedGate { gate } =>
                write!(f, "unsupported gate in target native set: {}", gate),
            MicroError::InvalidWire { wire, num_wires } =>
                write!(f, "wire {} out of range (num_wires={})", wire, num_wires),
            MicroError::UnresolvedParam { context } =>
                write!(f, "unresolved symbolic parameter in {}", context),
            MicroError::CacheError { detail } =>
                write!(f, "circuit cache error: {}", detail),
        }
    }
}

impl std::error::Error for MicroError {}

impl From<MicroError> for CqamError {
    fn from(err: MicroError) -> Self {
        let operation = match &err {
            MicroError::DecompositionFailed { kernel, .. } =>
                format!("decompose/{}", kernel),
            MicroError::RoutingFailed { .. } => "routing".to_string(),
            MicroError::UnsupportedGate { gate } =>
                format!("native_map/{}", gate),
            MicroError::InvalidWire { .. } => "wire_validation".to_string(),
            MicroError::UnresolvedParam { .. } => "param_resolution".to_string(),
            MicroError::CacheError { .. } => "cache".to_string(),
        };
        CqamError::QpuUnsupportedOperation {
            operation,
            detail: format!("{}", err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_error_display() {
        let e = MicroError::DecompositionFailed {
            kernel: "Fourier".to_string(),
            detail: "too many qubits".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Fourier"));
        assert!(s.contains("too many qubits"));

        let e2 = MicroError::RoutingFailed { detail: "no path".to_string() };
        assert!(format!("{}", e2).contains("no path"));

        let e3 = MicroError::UnsupportedGate { gate: "CustomUnitary".to_string() };
        assert!(format!("{}", e3).contains("CustomUnitary"));

        let e4 = MicroError::InvalidWire { wire: 5, num_wires: 3 };
        assert!(format!("{}", e4).contains("5"));

        let e5 = MicroError::UnresolvedParam { context: "Rotate".to_string() };
        assert!(format!("{}", e5).contains("Rotate"));

        let e6 = MicroError::CacheError { detail: "mismatch".to_string() };
        assert!(format!("{}", e6).contains("mismatch"));
    }

    #[test]
    fn test_micro_error_into_cqam_error() {
        let e = MicroError::DecompositionFailed {
            kernel: "Fourier".to_string(),
            detail: "test".to_string(),
        };
        let ce: CqamError = e.into();
        let s = format!("{}", ce);
        assert!(s.contains("Fourier"));

        let e2 = MicroError::RoutingFailed { detail: "test".to_string() };
        let ce2: CqamError = e2.into();
        let s2 = format!("{}", ce2);
        assert!(s2.contains("routing"));
    }
}
