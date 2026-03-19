//! IBM backend calibration data.
//!
//! `IbmCalibrationData` is populated from IBM Quantum backend property
//! responses.  The Phase 5 implementation accepts the data as pre-parsed
//! Rust values; Phase 6 will add REST fetching.

use std::collections::HashMap;

use cqam_core::native_ir::{NativeGate2, Op};
use cqam_qpu::traits::CalibrationData;

use crate::error::IbmError;
use crate::rest::BackendProperties;

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

// ---------------------------------------------------------------------------
// Unit conversion helper
// ---------------------------------------------------------------------------

/// Convert a value to seconds based on its unit string.
///
/// Returns the value unchanged if unit is `"s"`, `None`, or unrecognized.
fn to_seconds(value: f64, unit: &Option<String>) -> f64 {
    match unit.as_deref() {
        Some("us") | Some("\u{00b5}s") => value * 1e-6,
        Some("ms") => value * 1e-3,
        Some("ns") => value * 1e-9,
        _ => value, // "s", None, or unrecognized
    }
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

    /// Parse IBM backend properties into calibration data.
    ///
    /// Extracts per-qubit T1, T2, and readout_error from the `qubits` array,
    /// and gate_error / gate_length from the `gates` array.
    ///
    /// Missing properties default to `f64::NAN`.  T1/T2 values are normalized
    /// to seconds using the reported unit.  Gate times use the last `sx`/`cx`
    /// value seen; if none appear, synthetic defaults (35 ns / 660 ns) are
    /// retained.
    ///
    /// # Errors
    ///
    /// Returns `IbmError::CalibrationError` if the properties structure is
    /// fundamentally malformed (currently infallible, but the signature
    /// reserves space for future validation).
    pub fn from_ibm_properties(
        props: &BackendProperties,
        num_qubits: u32,
    ) -> Result<Self, IbmError> {
        let n = num_qubits as usize;
        let mut t1 = vec![f64::NAN; n];
        let mut t2 = vec![f64::NAN; n];
        let mut readout_error = vec![f64::NAN; n];
        let mut single_gate_error = vec![f64::NAN; n];
        let mut two_gate_error: HashMap<(u32, u32), f64> = HashMap::new();

        // Synthetic defaults; overwritten if IBM data contains gate_length.
        let mut single_gate_time_s = 35e-9_f64;
        let mut two_gate_time_s = 660e-9_f64;

        // --- Per-qubit properties -------------------------------------------
        for (qubit_idx, qubit_props) in props.qubits.iter().enumerate() {
            if qubit_idx >= n {
                break;
            }
            for prop in qubit_props {
                match prop.name.as_str() {
                    "T1" => {
                        t1[qubit_idx] = to_seconds(prop.value, &prop.unit);
                    }
                    "T2" => {
                        t2[qubit_idx] = to_seconds(prop.value, &prop.unit);
                    }
                    "readout_error" => {
                        readout_error[qubit_idx] = prop.value;
                    }
                    _ => {}
                }
            }
        }

        // --- Gate properties ------------------------------------------------
        //
        // Track whether we have set single_gate_error per qubit so that we
        // can implement the sx > x > id fallback priority.
        let mut sg_source: Vec<u8> = vec![0; n]; // 0=none, 1=id, 2=x, 3=sx

        for gate_prop in &props.gates {
            for param in &gate_prop.parameters {
                match (gate_prop.gate.as_str(), param.name.as_str()) {
                    // -- Single-qubit gate error (sx > x > id priority) ------
                    ("sx", "gate_error") => {
                        if let Some(&q) = gate_prop.qubits.first() {
                            let qi = q as usize;
                            if qi < n {
                                single_gate_error[qi] = param.value;
                                sg_source[qi] = 3;
                            }
                        }
                    }
                    ("x", "gate_error") => {
                        if let Some(&q) = gate_prop.qubits.first() {
                            let qi = q as usize;
                            if qi < n && sg_source[qi] < 2 {
                                single_gate_error[qi] = param.value;
                                sg_source[qi] = 2;
                            }
                        }
                    }
                    ("id", "gate_error") => {
                        if let Some(&q) = gate_prop.qubits.first() {
                            let qi = q as usize;
                            if qi < n && sg_source[qi] < 1 {
                                single_gate_error[qi] = param.value;
                                sg_source[qi] = 1;
                            }
                        }
                    }

                    // -- Single-qubit gate length ----------------------------
                    ("sx" | "x", "gate_length") => {
                        single_gate_time_s =
                            to_seconds(param.value, &param.unit);
                    }

                    // -- Two-qubit gate error --------------------------------
                    ("cx" | "ecr", "gate_error") => {
                        if gate_prop.qubits.len() == 2 {
                            let key = Self::edge_key(
                                gate_prop.qubits[0],
                                gate_prop.qubits[1],
                            );
                            two_gate_error.insert(key, param.value);
                        }
                    }

                    // -- Two-qubit gate length -------------------------------
                    ("cx" | "ecr", "gate_length") => {
                        two_gate_time_s =
                            to_seconds(param.value, &param.unit);
                    }

                    _ => {}
                }
            }
        }

        Ok(Self::new(
            t1,
            t2,
            single_gate_error,
            two_gate_error,
            readout_error,
            single_gate_time_s,
            two_gate_time_s,
        ))
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

    // --- from_ibm_properties tests -------------------------------------------

    use crate::rest::BackendProperties;

    const IBM_PROPERTIES_JSON: &str = r#"{
        "qubits": [
            [
                {"name": "T1", "value": 0.000123, "unit": "s"},
                {"name": "T2", "value": 0.000098, "unit": "s"},
                {"name": "readout_error", "value": 0.012},
                {"name": "frequency", "value": 5.1e9, "unit": "GHz"}
            ],
            [
                {"name": "T1", "value": 0.000110, "unit": "s"},
                {"name": "T2", "value": 0.000085, "unit": "s"},
                {"name": "readout_error", "value": 0.015}
            ],
            [
                {"name": "T1", "value": 0.000095, "unit": "s"},
                {"name": "T2", "value": 0.000075, "unit": "s"},
                {"name": "readout_error", "value": 0.018}
            ]
        ],
        "gates": [
            {
                "gate": "sx",
                "qubits": [0],
                "parameters": [
                    {"name": "gate_error", "value": 0.00035},
                    {"name": "gate_length", "value": 3.5556e-8, "unit": "s"}
                ]
            },
            {
                "gate": "sx",
                "qubits": [1],
                "parameters": [
                    {"name": "gate_error", "value": 0.00042}
                ]
            },
            {
                "gate": "sx",
                "qubits": [2],
                "parameters": [
                    {"name": "gate_error", "value": 0.00028}
                ]
            },
            {
                "gate": "cx",
                "qubits": [0, 1],
                "parameters": [
                    {"name": "gate_error", "value": 0.0078},
                    {"name": "gate_length", "value": 6.6e-7, "unit": "s"}
                ]
            },
            {
                "gate": "cx",
                "qubits": [1, 2],
                "parameters": [
                    {"name": "gate_error", "value": 0.0092},
                    {"name": "gate_length", "value": 7.1e-7, "unit": "s"}
                ]
            }
        ],
        "last_update_date": "2026-03-19T12:00:00Z"
    }"#;

    #[test]
    fn test_from_ibm_properties_basic() {
        let props: BackendProperties =
            serde_json::from_str(IBM_PROPERTIES_JSON).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 3).unwrap();

        // T1 values (in seconds)
        assert!((cal.t1(0) - 0.000123).abs() < 1e-12);
        assert!((cal.t1(1) - 0.000110).abs() < 1e-12);
        assert!((cal.t1(2) - 0.000095).abs() < 1e-12);

        // T2 values
        assert!((cal.t2(0) - 0.000098).abs() < 1e-12);
        assert!((cal.t2(1) - 0.000085).abs() < 1e-12);

        // Readout error
        assert!((cal.readout_error(0) - 0.012).abs() < 1e-12);
        assert!((cal.readout_error(1) - 0.015).abs() < 1e-12);

        // Single-gate error (from sx)
        assert!((cal.single_gate_error(0) - 0.00035).abs() < 1e-12);
        assert!((cal.single_gate_error(1) - 0.00042).abs() < 1e-12);

        // Two-gate error (cx)
        assert!((cal.two_gate_error(0, 1) - 0.0078).abs() < 1e-12);
        assert!((cal.two_gate_error(1, 2) - 0.0092).abs() < 1e-12);
        // Symmetric access
        assert!((cal.two_gate_error(1, 0) - 0.0078).abs() < 1e-12);

        // Gate times — last cx gate_length seen is 7.1e-7
        assert!((cal.two_gate_time() - 7.1e-7).abs() < 1e-15);
        // sx gate_length is 3.5556e-8
        assert!((cal.single_gate_time() - 3.5556e-8).abs() < 1e-15);
    }

    #[test]
    fn test_from_ibm_properties_missing_values() {
        // Qubit 1 has no T1, qubit 2 is entirely absent.
        let json = r#"{
            "qubits": [
                [
                    {"name": "T1", "value": 0.0001, "unit": "s"},
                    {"name": "T2", "value": 0.00008, "unit": "s"},
                    {"name": "readout_error", "value": 0.01}
                ],
                [
                    {"name": "T2", "value": 0.00007, "unit": "s"},
                    {"name": "readout_error", "value": 0.02}
                ]
            ],
            "gates": [],
            "last_update_date": "2026-03-19T00:00:00Z"
        }"#;

        let props: BackendProperties = serde_json::from_str(json).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 3).unwrap();

        // Qubit 0: all present
        assert!((cal.t1(0) - 0.0001).abs() < 1e-12);
        assert!((cal.readout_error(0) - 0.01).abs() < 1e-12);

        // Qubit 1: T1 missing -> NaN
        assert!(cal.t1(1).is_nan());
        assert!((cal.t2(1) - 0.00007).abs() < 1e-12);

        // Qubit 2: absent from qubits array -> NaN
        assert!(cal.t1(2).is_nan());
        assert!(cal.t2(2).is_nan());
        assert!(cal.readout_error(2).is_nan());

        // No gates -> single_gate_error all NaN
        assert!(cal.single_gate_error(0).is_nan());
        assert!(cal.single_gate_error(1).is_nan());

        // Gate times fall back to synthetic defaults
        assert!((cal.single_gate_time() - 35e-9).abs() < 1e-15);
        assert!((cal.two_gate_time() - 660e-9).abs() < 1e-15);
    }

    #[test]
    fn test_from_ibm_properties_unit_conversion() {
        let json = r#"{
            "qubits": [
                [
                    {"name": "T1", "value": 123.0, "unit": "us"},
                    {"name": "T2", "value": 98000.0, "unit": "ns"}
                ]
            ],
            "gates": [
                {
                    "gate": "sx",
                    "qubits": [0],
                    "parameters": [
                        {"name": "gate_error", "value": 0.001},
                        {"name": "gate_length", "value": 35.556, "unit": "ns"}
                    ]
                }
            ]
        }"#;

        let props: BackendProperties = serde_json::from_str(json).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 1).unwrap();

        // 123 us -> 123e-6 s
        assert!((cal.t1(0) - 123e-6).abs() < 1e-12);
        // 98000 ns -> 98e-6 s
        assert!((cal.t2(0) - 98e-6).abs() < 1e-12);
        // 35.556 ns -> 3.5556e-8 s
        assert!((cal.single_gate_time() - 3.5556e-8).abs() < 1e-15);
    }

    #[test]
    fn test_from_ibm_properties_gate_error_priority() {
        // Qubit 0 has sx, x, and id.  Qubit 1 has only x and id.
        // Qubit 2 has only id.
        let json = r#"{
            "qubits": [
                [{"name": "T1", "value": 0.0001, "unit": "s"}],
                [{"name": "T1", "value": 0.0001, "unit": "s"}],
                [{"name": "T1", "value": 0.0001, "unit": "s"}]
            ],
            "gates": [
                {"gate": "id", "qubits": [0], "parameters": [{"name": "gate_error", "value": 0.0001}]},
                {"gate": "x",  "qubits": [0], "parameters": [{"name": "gate_error", "value": 0.0005}]},
                {"gate": "sx", "qubits": [0], "parameters": [{"name": "gate_error", "value": 0.0003}]},
                {"gate": "id", "qubits": [1], "parameters": [{"name": "gate_error", "value": 0.0002}]},
                {"gate": "x",  "qubits": [1], "parameters": [{"name": "gate_error", "value": 0.0006}]},
                {"gate": "id", "qubits": [2], "parameters": [{"name": "gate_error", "value": 0.0004}]}
            ]
        }"#;

        let props: BackendProperties = serde_json::from_str(json).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 3).unwrap();

        // Qubit 0: sx wins (0.0003), not x (0.0005) or id (0.0001)
        assert!((cal.single_gate_error(0) - 0.0003).abs() < 1e-12);
        // Qubit 1: x wins (0.0006), not id (0.0002)
        assert!((cal.single_gate_error(1) - 0.0006).abs() < 1e-12);
        // Qubit 2: id fallback (0.0004)
        assert!((cal.single_gate_error(2) - 0.0004).abs() < 1e-12);
    }

    #[test]
    fn test_from_ibm_properties_ecr_gate() {
        // Some IBM Eagle/Heron backends use ECR instead of CX.
        let json = r#"{
            "qubits": [
                [{"name": "T1", "value": 0.0001, "unit": "s"}],
                [{"name": "T1", "value": 0.0001, "unit": "s"}]
            ],
            "gates": [
                {
                    "gate": "ecr",
                    "qubits": [0, 1],
                    "parameters": [
                        {"name": "gate_error", "value": 0.0065},
                        {"name": "gate_length", "value": 5.3e-7, "unit": "s"}
                    ]
                }
            ]
        }"#;

        let props: BackendProperties = serde_json::from_str(json).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 2).unwrap();

        assert!((cal.two_gate_error(0, 1) - 0.0065).abs() < 1e-12);
        assert!((cal.two_gate_time() - 5.3e-7).abs() < 1e-15);
    }

    #[test]
    fn test_from_ibm_properties_empty_gates() {
        let json = r#"{
            "qubits": [
                [
                    {"name": "T1", "value": 0.0001, "unit": "s"},
                    {"name": "readout_error", "value": 0.01}
                ]
            ],
            "gates": []
        }"#;

        let props: BackendProperties = serde_json::from_str(json).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 1).unwrap();

        // Per-qubit data present
        assert!((cal.t1(0) - 0.0001).abs() < 1e-12);
        assert!((cal.readout_error(0) - 0.01).abs() < 1e-12);

        // No gate data -> NaN for single-gate error, synthetic defaults for times
        assert!(cal.single_gate_error(0).is_nan());
        assert!((cal.single_gate_time() - 35e-9).abs() < 1e-15);
        assert!((cal.two_gate_time() - 660e-9).abs() < 1e-15);
    }

    #[test]
    fn test_from_ibm_properties_fewer_qubits_in_json() {
        // Device has 5 qubits but properties only report 2.
        let json = r#"{
            "qubits": [
                [{"name": "T1", "value": 0.0001, "unit": "s"}],
                [{"name": "T1", "value": 0.0002, "unit": "s"}]
            ],
            "gates": []
        }"#;

        let props: BackendProperties = serde_json::from_str(json).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 5).unwrap();

        assert!((cal.t1(0) - 0.0001).abs() < 1e-12);
        assert!((cal.t1(1) - 0.0002).abs() < 1e-12);
        // Qubits 2-4: NaN (no data in properties)
        assert!(cal.t1(2).is_nan());
        assert!(cal.t1(3).is_nan());
        assert!(cal.t1(4).is_nan());
    }

    #[test]
    fn test_to_seconds_unit_conversion() {
        // Test the to_seconds helper directly via round-trip through from_ibm_properties
        // by using ms unit
        let json = r#"{
            "qubits": [
                [
                    {"name": "T1", "value": 0.123, "unit": "ms"},
                    {"name": "T2", "value": 0.098, "unit": "ms"}
                ]
            ],
            "gates": []
        }"#;

        let props: BackendProperties = serde_json::from_str(json).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 1).unwrap();

        // 0.123 ms -> 0.123e-3 s = 123e-6 s
        assert!((cal.t1(0) - 123e-6).abs() < 1e-12);
        // 0.098 ms -> 98e-6 s
        assert!((cal.t2(0) - 98e-6).abs() < 1e-12);
    }

    #[test]
    fn test_to_seconds_unicode_microseconds() {
        // Test the Unicode µ (U+00B5) microsecond unit
        let json = "{\"qubits\": [[{\"name\": \"T1\", \"value\": 123.0, \"unit\": \"\u{00b5}s\"}]], \"gates\": []}";

        let props: BackendProperties = serde_json::from_str(json).unwrap();
        let cal = IbmCalibrationData::from_ibm_properties(&props, 1).unwrap();

        // 123 µs -> 123e-6 s
        assert!((cal.t1(0) - 123e-6).abs() < 1e-12);
    }
}
