//! IonQ backend calibration data.
//!
//! `IonQCalibrationData` is populated from IonQ backend characterization
//! responses. Trapped-ion calibration uses mean T1/T2 across all qubits
//! and uniform per-gate fidelity rather than per-qubit error maps (unlike
//! superconducting devices which have strongly site-dependent errors).
//!
//! The v0.4 characterization endpoint provides:
//! - T1/T2 coherence times (seconds) via `timing.t1` / `timing.t2`
//! - SPAM median fidelity via `fidelity.spam.median`
//! - Single/two-qubit gate times via `timing.1q` / `timing.2q`
//!
//! Gate error rates are not reported by the v0.4 characterization API;
//! we fall back to published Forte hardware values.

use cqam_core::native_ir::{NativeGate2, Op};
use cqam_qpu::traits::CalibrationData;

use crate::rest::CharacterizationResponse;

// ---------------------------------------------------------------------------
// IonQCalibrationData
// ---------------------------------------------------------------------------

/// Calibration data for an IonQ trapped-ion backend.
///
/// Unlike superconducting devices, IonQ reports mean T1/T2 across all qubits
/// and uniform single/two-qubit gate fidelity. Per-qubit variation is small
/// enough that a single value is representative.
#[derive(Debug, Clone)]
pub struct IonQCalibrationData {
    #[allow(dead_code)]
    num_qubits: u32,
    /// Mean T1 relaxation time across all qubits (seconds).
    t1_mean: f64,
    /// Mean T2 dephasing time across all qubits (seconds).
    t2_mean: f64,
    /// Mean single-qubit gate error rate (1 - fidelity).
    single_gate_error: f64,
    /// Mean two-qubit (MS) gate error rate (1 - fidelity).
    two_gate_error: f64,
    /// SPAM (state-prep-and-measurement) error rate.
    readout_error: f64,
    /// Single-qubit gate time (seconds).
    single_gate_time_s: f64,
    /// Two-qubit gate time (seconds).
    two_gate_time_s: f64,
}

impl IonQCalibrationData {
    /// Build synthetic calibration representative of IonQ Forte hardware.
    ///
    /// Values drawn from published IonQ Forte characterization data.
    pub fn synthetic(num_qubits: u32) -> Self {
        Self {
            num_qubits,
            t1_mean: 100.0,          // ~100 seconds for trapped-ion
            t2_mean: 1.0,            // ~1 second
            single_gate_error: 6e-4, // 99.94% 1Q fidelity
            two_gate_error: 6e-3,    // 99.4% 2Q fidelity (MS gate)
            readout_error: 3e-3,     // 99.7% SPAM fidelity
            single_gate_time_s: 1.35e-4, // 135 µs
            two_gate_time_s: 2.1e-4,     // 210 µs
        }
    }

    /// Construct from an IonQ v0.4 characterization response.
    ///
    /// Extracts T1/T2, gate timing, SPAM readout error, and gate fidelities
    /// from the characterization. Gate times of 0 (as sometimes returned by
    /// the API) fall back to published Forte hardware values. Gate error rates
    /// default to Forte published values when not provided.
    pub fn from_characterization_response(char_resp: &CharacterizationResponse) -> Self {
        let timing = char_resp.timing.as_ref();
        let fidelity = char_resp.fidelity.as_ref();

        let t1_mean = timing.and_then(|t| t.t1).unwrap_or(100.0);
        let t2_mean = timing.and_then(|t| t.t2).unwrap_or(1.0);

        // Gate times are sometimes 0 in the API response; treat 0 as absent.
        let single_gate_time_s = timing
            .and_then(|t| t.single_qubit)
            .filter(|&v| v > 0.0)
            .unwrap_or(1.35e-4);
        let two_gate_time_s = timing
            .and_then(|t| t.two_qubit)
            .filter(|&v| v > 0.0)
            .unwrap_or(2.1e-4);

        let readout_error = fidelity
            .and_then(|f| f.spam.as_ref())
            .and_then(|s| s.median)
            .map(|m| 1.0 - m)
            .unwrap_or(3e-3);

        let single_gate_error = fidelity
            .and_then(|f| f.single_qubit.as_ref())
            .and_then(|g| g.median)
            .map(|m| 1.0 - m)
            .unwrap_or(6e-4);

        let two_gate_error = fidelity
            .and_then(|f| f.two_qubit.as_ref())
            .and_then(|g| g.median)
            .map(|m| 1.0 - m)
            .unwrap_or(6e-3);

        Self {
            num_qubits: char_resp.qubits,
            t1_mean,
            t2_mean,
            single_gate_error,
            two_gate_error,
            readout_error,
            single_gate_time_s,
            two_gate_time_s,
        }
    }
}

impl CalibrationData for IonQCalibrationData {
    fn t1(&self, _qubit: u32) -> f64 {
        self.t1_mean
    }

    fn t2(&self, _qubit: u32) -> f64 {
        self.t2_mean
    }

    fn single_gate_error(&self, _qubit: u32) -> f64 {
        self.single_gate_error
    }

    fn two_gate_error(&self, _qubit_a: u32, _qubit_b: u32) -> f64 {
        self.two_gate_error
    }

    fn readout_error(&self, _qubit: u32) -> f64 {
        self.readout_error
    }

    fn single_gate_time(&self) -> f64 {
        self.single_gate_time_s
    }

    fn two_gate_time(&self) -> f64 {
        self.two_gate_time_s
    }

    fn estimate_circuit_fidelity(&self, circuit: &cqam_core::native_ir::Circuit) -> f64 {
        let mut fidelity = 1.0_f64;
        for op in &circuit.ops {
            match op {
                Op::Gate1q(_) => {
                    fidelity *= 1.0 - self.single_gate_error;
                }
                Op::Gate2q(g) => {
                    let err = match g.gate {
                        NativeGate2::Cx => self.two_gate_error,
                    };
                    fidelity *= 1.0 - err;
                }
                Op::Measure(_) => {
                    fidelity *= 1.0 - self.readout_error;
                }
                Op::Reset(_) | Op::Barrier(_) => {}
            }
        }
        fidelity.max(0.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::native_ir::{ApplyGate1q, Circuit, NativeGate1, Observe, Op, PhysicalQubit};

    #[test]
    fn test_synthetic_calibration_forte_values() {
        let cal = IonQCalibrationData::synthetic(36);
        assert!(cal.t1(0) > 1.0, "T1 should be order of seconds for trapped-ion");
        assert!(cal.t2(0) > 0.1);
        assert!(cal.single_gate_error(0) < 1e-3);
        assert!(cal.two_gate_error(0, 1) < 1e-2);
    }

    #[test]
    fn test_uniform_calibration_all_qubits_same() {
        let cal = IonQCalibrationData::synthetic(36);
        assert_eq!(cal.t1(0), cal.t1(35));
        assert_eq!(cal.two_gate_error(0, 1), cal.two_gate_error(12, 20));
    }

    #[test]
    fn test_fidelity_empty_circuit() {
        let cal = IonQCalibrationData::synthetic(5);
        let c = Circuit::new(5);
        assert!((cal.estimate_circuit_fidelity(&c) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_fidelity_decreases_with_ops() {
        let cal = IonQCalibrationData::synthetic(2);
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Sx,
        }));
        c.ops.push(Op::Measure(Observe { qubit: PhysicalQubit(0), clbit: 0 }));
        let f = cal.estimate_circuit_fidelity(&c);
        assert!(f < 1.0);
        assert!(f > 0.0);
    }

    #[test]
    fn test_from_characterization_response_real_forte1_values() {
        // Mirrors the real forte-1 characterization response (gate times are 0 in real API).
        use crate::rest::{CharFidelity, CharTiming, CharacterizationResponse, GateFidelity};
        let char_resp = CharacterizationResponse {
            id: "ffbc9da9-96cc-4f39-8715-ec6f038327d3".to_string(),
            date: Some("2026-04-20T00:00:00Z".to_string()),
            backend: "qpu.forte-1".to_string(),
            qubits: 36,
            fidelity: Some(CharFidelity {
                spam:         Some(GateFidelity { median: Some(0.9942) }),
                single_qubit: Some(GateFidelity { median: Some(0.9998) }),
                two_qubit:    Some(GateFidelity { median: Some(0.9952) }),
            }),
            timing: Some(CharTiming {
                t1: Some(100.0),
                t2: Some(1.0),
                single_qubit: Some(0.0), // real API returns 0; should fall back to default
                two_qubit:    Some(0.0),
            }),
        };
        let cal = IonQCalibrationData::from_characterization_response(&char_resp);

        assert!((cal.t1_mean - 100.0).abs() < 1e-9);
        assert!((cal.t2_mean - 1.0).abs() < 1e-9);
        // readout_error = 1 - 0.9942
        assert!((cal.readout_error - (1.0 - 0.9942)).abs() < 1e-9);
        // single_gate_error = 1 - 0.9998
        assert!((cal.single_gate_error - (1.0 - 0.9998)).abs() < 1e-9);
        // two_gate_error = 1 - 0.9952
        assert!((cal.two_gate_error - (1.0 - 0.9952)).abs() < 1e-9);
        // Gate times of 0 fall back to published defaults.
        assert!((cal.single_gate_time_s - 1.35e-4).abs() < 1e-15);
        assert!((cal.two_gate_time_s - 2.1e-4).abs() < 1e-15);
    }

    #[test]
    fn test_fidelity_includes_two_qubit_gates() {
        let cal = IonQCalibrationData::synthetic(2);
        let mut c = Circuit::new(2);
        // Only a 2q gate — verify two_gate_error is applied.
        use cqam_core::native_ir::{ApplyGate2q, NativeGate2};
        c.ops.push(Op::Gate2q(ApplyGate2q {
            qubit_a: PhysicalQubit(0),
            qubit_b: PhysicalQubit(1),
            gate: NativeGate2::Cx,
        }));
        let f = cal.estimate_circuit_fidelity(&c);
        let expected = 1.0 - cal.two_gate_error;
        assert!((f - expected).abs() < 1e-12, "2q fidelity: got {f}, expected {expected}");
    }

    #[test]
    fn test_fidelity_is_product_of_all_gate_errors() {
        let cal = IonQCalibrationData::synthetic(2);
        use cqam_core::native_ir::{ApplyGate2q, NativeGate2};
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::X }));
        c.ops.push(Op::Gate2q(ApplyGate2q {
            qubit_a: PhysicalQubit(0),
            qubit_b: PhysicalQubit(1),
            gate: NativeGate2::Cx,
        }));
        c.ops.push(Op::Measure(Observe { qubit: PhysicalQubit(0), clbit: 0 }));
        let f = cal.estimate_circuit_fidelity(&c);
        let expected = (1.0 - cal.single_gate_error)
            * (1.0 - cal.two_gate_error)
            * (1.0 - cal.readout_error);
        assert!((f - expected).abs() < 1e-12);
    }

    #[test]
    fn test_fidelity_floor_is_zero() {
        // Enough gates that naive product underflows — must clamp to 0.0, not go negative.
        let cal = IonQCalibrationData {
            num_qubits: 1,
            t1_mean: 100.0,
            t2_mean: 1.0,
            single_gate_error: 1.0, // worst possible: every gate destroys fidelity
            two_gate_error: 0.0,
            readout_error: 0.0,
            single_gate_time_s: 1.35e-4,
            two_gate_time_s: 2.1e-4,
        };
        let mut c = Circuit::new(1);
        for _ in 0..10 {
            c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::X }));
        }
        let f = cal.estimate_circuit_fidelity(&c);
        assert_eq!(f, 0.0, "fidelity must be clamped to 0.0, got {f}");
    }

    #[test]
    fn test_from_characterization_response_timing_only() {
        use crate::rest::{CharTiming, CharacterizationResponse};
        let char_resp = CharacterizationResponse {
            id: "t".into(),
            date: None,
            backend: "simulator".into(),
            qubits: 29,
            fidelity: None, // no fidelity — all defaults
            timing: Some(CharTiming { t1: Some(50.0), t2: Some(0.5), single_qubit: None, two_qubit: None }),
        };
        let cal = IonQCalibrationData::from_characterization_response(&char_resp);
        assert!((cal.t1_mean - 50.0).abs() < 1e-9);
        assert!((cal.t2_mean - 0.5).abs() < 1e-9);
        assert!((cal.readout_error - 3e-3).abs() < 1e-9); // default
        assert!((cal.single_gate_error - 6e-4).abs() < 1e-9); // default
    }

    #[test]
    fn test_from_characterization_response_fidelity_only() {
        use crate::rest::{CharFidelity, CharacterizationResponse, GateFidelity};
        let char_resp = CharacterizationResponse {
            id: "f".into(),
            date: None,
            backend: "simulator".into(),
            qubits: 29,
            fidelity: Some(CharFidelity {
                spam: Some(GateFidelity { median: Some(0.998) }),
                single_qubit: Some(GateFidelity { median: Some(0.9999) }),
                two_qubit: Some(GateFidelity { median: Some(0.994) }),
            }),
            timing: None, // no timing — all defaults
        };
        let cal = IonQCalibrationData::from_characterization_response(&char_resp);
        assert!((cal.t1_mean - 100.0).abs() < 1e-9); // default
        assert!((cal.t2_mean - 1.0).abs() < 1e-9); // default
        assert!((cal.readout_error - 0.002).abs() < 1e-9);
        assert!((cal.single_gate_error - 0.0001).abs() < 1e-9);
        assert!((cal.two_gate_error - 0.006).abs() < 1e-9);
        assert!((cal.single_gate_time_s - 1.35e-4).abs() < 1e-15); // default
    }

    #[test]
    fn test_from_characterization_response_missing_fields_use_defaults() {
        use crate::rest::CharacterizationResponse;
        let char_resp = CharacterizationResponse {
            id: "char-minimal".to_string(),
            date: None,
            backend: "simulator".to_string(),
            qubits: 29,
            fidelity: None,
            timing: None,
        };
        let cal = IonQCalibrationData::from_characterization_response(&char_resp);
        assert!((cal.t1_mean - 100.0).abs() < 1e-9);
        assert!((cal.t2_mean - 1.0).abs() < 1e-9);
        assert!((cal.readout_error - 3e-3).abs() < 1e-9);
        assert!((cal.single_gate_error - 6e-4).abs() < 1e-9);
        assert!((cal.two_gate_error - 6e-3).abs() < 1e-9);
        assert!((cal.single_gate_time_s - 1.35e-4).abs() < 1e-15);
        assert!((cal.two_gate_time_s - 2.1e-4).abs() < 1e-15);
    }

    #[test]
    fn test_two_gate_error_symmetric() {
        // IonQ trapped-ion has uniform all-to-all errors — operand order must not matter.
        let cal = IonQCalibrationData::synthetic(36);
        assert_eq!(cal.two_gate_error(0, 1), cal.two_gate_error(1, 0));
        assert_eq!(cal.two_gate_error(5, 12), cal.two_gate_error(12, 5));
        assert_eq!(cal.two_gate_error(0, 35), cal.two_gate_error(35, 0));
    }

    #[test]
    fn test_uniform_error_any_qubit_index() {
        // Unlike superconducting devices, IonQ reports site-independent errors.
        // Every qubit index must return the same value regardless of index.
        let cal = IonQCalibrationData::synthetic(36);
        let e0 = cal.single_gate_error(0);
        assert_eq!(cal.single_gate_error(10), e0);
        assert_eq!(cal.single_gate_error(35), e0);
        assert_eq!(cal.t1(0), cal.t1(35));
        assert_eq!(cal.t2(0), cal.t2(35));
        assert_eq!(cal.readout_error(0), cal.readout_error(35));
    }
}
