//! Integration tests for entanglement detection via the QuantumRegister API.
//!
//! These tests verify that `QuantumRegister::is_entangled()` correctly
//! delegates to the underlying Statevector or DensityMatrix backend.

use cqam_sim::density_matrix::DensityMatrix;
use cqam_sim::quantum_register::QuantumRegister;
use cqam_sim::statevector::Statevector;

#[test]
fn test_qr_is_entangled_pure_bell() {
    // Pure(Statevector) Bell state should be entangled.
    let qr = QuantumRegister::Pure(Statevector::new_bell());
    assert!(qr.is_entangled(),
        "Pure Bell state QuantumRegister should report entanglement");
}

#[test]
fn test_qr_is_entangled_pure_product() {
    // Pure(Statevector) |00> should NOT be entangled.
    let qr = QuantumRegister::Pure(Statevector::new_zero_state(2));
    assert!(!qr.is_entangled(),
        "Pure |00> QuantumRegister should not report entanglement");
}

#[test]
fn test_qr_is_entangled_mixed_bell() {
    // Mixed(DensityMatrix) Bell state should be entangled.
    let qr = QuantumRegister::Mixed(DensityMatrix::new_bell());
    assert!(qr.is_entangled(),
        "Mixed Bell state QuantumRegister should report entanglement");
}
