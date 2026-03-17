//! IBM backend calibration data.
//!
//! `IbmCalibrationData` is populated from IBM Quantum backend property
//! responses.  The Phase 5 implementation accepts the data as pre-parsed
//! Rust values; Phase 6 will add REST fetching.

use std::collections::HashMap;

use cqam_core::native_ir::{NativeGate2, Op};
use cqam_qpu::traits::CalibrationData;

// ---------------------------------------------------------------------------
// IbmCalibrationData
// ---------------------------------------------------------------------------

/// Calibration data for an IBM superconducting backend.
#[derive(Debug, Clone)]
pub struct IbmCalibrationData {
    /// Per-qubit T1 relaxation times (seconds).
    t1: Vec<f64>,
    /// Per-qubit T2 dephasing times (seconds).
    t2: Vec<f64>,
    /// Per-qubit single-qubit gate error rates.
    single_gate_error: Vec<f64>,
    /// Edge (a, b) → two-qubit gate error rate (keyed with a < b).
    two_gate_error: HashMap<(u32, u32), f64>,
    /// Per-qubit readout (measurement) error rates.
    readout_error: Vec<f64>,
    /// Typical single-qubit gate duration (seconds).
    single_gate_time_s: f64,
    /// Typical two-qubit gate duration (seconds).
    two_gate_time_s: f64,
}

impl IbmCalibrationData {
    /// Construct from raw arrays.  All `Vec`s must have length `num_qubits`.
    pub fn new(
        t1: Vec<f64>,
        t2: Vec<f64>,
        single_gate_error: Vec<f64>,
        two_gate_error: HashMap<(u32, u32), f64>,
        readout_error: Vec<f64>,
        single_gate_time_s: f64,
        two_gate_time_s: f64,
    ) -> Self {
        Self {
            t1,
            t2,
            single_gate_error,
            two_gate_error,
            readout_error,
            single_gate_time_s,
            two_gate_time_s,
        }
    }

    /// Build a synthetic calibration for testing.
    ///
    /// All error rates are typical IBM Falcon values.
    pub fn synthetic(num_qubits: u32) -> Self {
        let n = num_qubits as usize;
        Self {
            t1: vec![100e-6; n],
            t2: vec![80e-6; n],
            single_gate_error: vec![1e-3; n],
            two_gate_error: HashMap::new(),
            readout_error: vec![1e-2; n],
            single_gate_time_s: 35e-9,
            two_gate_time_s: 660e-9,
        }
    }

    fn edge_key(a: u32, b: u32) -> (u32, u32) {
        if a <= b { (a, b) } else { (b, a) }
    }
}

impl CalibrationData for IbmCalibrationData {
    fn t1(&self, qubit: u32) -> f64 {
        self.t1.get(qubit as usize).copied().unwrap_or(f64::NAN)
    }

    fn t2(&self, qubit: u32) -> f64 {
        self.t2.get(qubit as usize).copied().unwrap_or(f64::NAN)
    }

    fn single_gate_error(&self, qubit: u32) -> f64 {
        self.single_gate_error
            .get(qubit as usize)
            .copied()
            .unwrap_or(f64::NAN)
    }

    fn two_gate_error(&self, qubit_a: u32, qubit_b: u32) -> f64 {
        let key = Self::edge_key(qubit_a, qubit_b);
        self.two_gate_error.get(&key).copied().unwrap_or(f64::NAN)
    }

    fn readout_error(&self, qubit: u32) -> f64 {
        self.readout_error
            .get(qubit as usize)
            .copied()
            .unwrap_or(f64::NAN)
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
                Op::Gate1q(g) => {
                    fidelity *= 1.0 - self.single_gate_error(g.qubit.0);
                }
                Op::Gate2q(g) => {
                    let err = match g.gate {
                        NativeGate2::Cx => self.two_gate_error(g.qubit_a.0, g.qubit_b.0),
                    };
                    // Fall back to a typical CX error if not calibrated
                    let effective_err = if err.is_nan() { 1e-2 } else { err };
                    fidelity *= 1.0 - effective_err;
                }
                Op::Measure(m) => {
                    fidelity *= 1.0 - self.readout_error(m.qubit.0);
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
    fn test_synthetic_calibration() {
        let cal = IbmCalibrationData::synthetic(5);
        assert!((cal.t1(0) - 100e-6).abs() < 1e-12);
        assert!((cal.t2(4) - 80e-6).abs() < 1e-12);
        assert!((cal.single_gate_error(0) - 1e-3).abs() < 1e-12);
        assert!((cal.readout_error(2) - 1e-2).abs() < 1e-12);
    }

    #[test]
    fn test_out_of_range_qubit_returns_nan() {
        let cal = IbmCalibrationData::synthetic(2);
        assert!(cal.t1(99).is_nan());
        assert!(cal.readout_error(99).is_nan());
    }

    #[test]
    fn test_fidelity_empty_circuit() {
        let cal = IbmCalibrationData::synthetic(5);
        let c = Circuit::new(5);
        let f = cal.estimate_circuit_fidelity(&c);
        assert!((f - 1.0).abs() < 1e-12, "empty circuit should have fidelity 1.0");
    }

    #[test]
    fn test_fidelity_decreases_with_ops() {
        let cal = IbmCalibrationData::synthetic(2);
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Sx,
        }));
        c.ops.push(Op::Measure(Observe {
            qubit: PhysicalQubit(0),
            clbit: 0,
        }));
        let f = cal.estimate_circuit_fidelity(&c);
        assert!(f < 1.0);
        assert!(f > 0.0);
    }

    #[test]
    fn test_two_gate_error_symmetric() {
        let mut two = HashMap::new();
        two.insert((0, 1), 5e-3);
        let cal = IbmCalibrationData::new(
            vec![100e-6, 100e-6],
            vec![80e-6, 80e-6],
            vec![1e-3, 1e-3],
            two,
            vec![1e-2, 1e-2],
            35e-9,
            660e-9,
        );
        assert!((cal.two_gate_error(0, 1) - 5e-3).abs() < 1e-12);
        assert!((cal.two_gate_error(1, 0) - 5e-3).abs() < 1e-12);
    }
}
