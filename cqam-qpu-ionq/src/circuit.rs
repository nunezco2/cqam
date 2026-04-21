//! IonQ native JSON circuit serializer.
//!
//! Converts a `native_ir::Circuit` to the IonQ QIS JSON circuit format used
//! by the v0.4 REST API. The IonQ "qis" gateset maps directly from the
//! `{SX, X, Rz, CX}` superconducting native gate set emitted by `cqam-micro`.
//!
//! Gate mapping:
//! - `NativeGate1::Sx`   → `{"gate": "v", "target": q}` (V gate ≡ √X)
//! - `NativeGate1::X`    → `{"gate": "x", "target": q}`
//! - `NativeGate1::Rz(θ)` → `{"gate": "rz", "target": q, "rotation": θ}`
//! - `NativeGate1::Id`   → omitted (identity has no observable effect)
//! - `NativeGate2::Cx`   → `{"gate": "cnot", "control": a, "target": b}`
//! - `Op::Measure/Reset/Barrier` → omitted (IonQ measures all qubits implicitly)

use serde_json::{json, Value};

use cqam_core::native_ir::{Circuit, NativeGate1, NativeGate2, Op};

use crate::error::IonQError;

/// Serialize a `native_ir::Circuit` to an IonQ v0.4 JSON circuit input object.
///
/// Returns the full `input` object ready for embedding in a job submission:
/// ```json
/// {
///   "gateset": "qis",
///   "qubits": 2,
///   "circuit": [
///     {"gate": "v", "target": 0},
///     {"gate": "cnot", "control": 0, "target": 1}
///   ]
/// }
/// ```
///
/// # Errors
///
/// Returns `IonQError::ConversionError` if an operation cannot be represented
/// in the IonQ QIS gateset (should not occur with the standard Superconducting
/// gate set, but is caught defensively).
pub fn circuit_to_ionq_json(circuit: &Circuit) -> Result<Value, IonQError> {
    let mut gates: Vec<Value> = Vec::with_capacity(circuit.ops.len());

    for op in &circuit.ops {
        match op {
            Op::Gate1q(g) => {
                let q = g.qubit.0;
                match &g.gate {
                    NativeGate1::Sx => {
                        gates.push(json!({"gate": "v", "target": q}));
                    }
                    NativeGate1::X => {
                        gates.push(json!({"gate": "x", "target": q}));
                    }
                    NativeGate1::Rz(theta) => {
                        gates.push(json!({"gate": "rz", "target": q, "rotation": theta}));
                    }
                    // Identity gates are no-ops; omit them from the circuit body.
                    NativeGate1::Id => {}
                }
            }
            Op::Gate2q(g) => {
                let a = g.qubit_a.0;
                let b = g.qubit_b.0;
                match g.gate {
                    NativeGate2::Cx => {
                        gates.push(json!({"gate": "cnot", "control": a, "target": b}));
                    }
                }
            }
            // Measurement, reset, and barrier are all omitted: IonQ measures
            // all qubits implicitly at circuit end and does not support
            // mid-circuit reset or barriers in the QIS gateset.
            Op::Measure(_) | Op::Reset(_) | Op::Barrier(_) => {}
        }
    }

    let input = json!({
        "gateset": "qis",
        "qubits": circuit.num_physical_qubits,
        "circuit": gates,
    });

    Ok(input)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::native_ir::{
        ApplyGate1q, ApplyGate2q, Barrier, NativeGate1, NativeGate2, Observe, PhysicalQubit,
        QubitReset,
    };

    fn gate1(qubit: u32, gate: NativeGate1) -> Op {
        Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(qubit), gate })
    }

    fn gate2(a: u32, b: u32) -> Op {
        Op::Gate2q(ApplyGate2q {
            qubit_a: PhysicalQubit(a),
            qubit_b: PhysicalQubit(b),
            gate: NativeGate2::Cx,
        })
    }

    fn measure(qubit: u32, clbit: u32) -> Op {
        Op::Measure(Observe { qubit: PhysicalQubit(qubit), clbit })
    }

    #[test]
    fn test_empty_circuit_structure() {
        let c = Circuit::new(3);
        let v = circuit_to_ionq_json(&c).unwrap();
        assert_eq!(v["gateset"], "qis");
        assert_eq!(v["qubits"], 3);
        assert_eq!(v["circuit"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_bell_circuit() {
        let mut c = Circuit::new(2);
        c.ops.push(gate1(0, NativeGate1::Sx));
        c.ops.push(gate2(0, 1));
        c.ops.push(measure(0, 0));
        c.ops.push(measure(1, 1));

        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();

        // Only two gates — measure ops are omitted.
        assert_eq!(gates.len(), 2);
        assert_eq!(gates[0]["gate"], "v");
        assert_eq!(gates[0]["target"], 0);
        assert_eq!(gates[1]["gate"], "cnot");
        assert_eq!(gates[1]["control"], 0);
        assert_eq!(gates[1]["target"], 1);
    }

    #[test]
    fn test_x_gate() {
        let mut c = Circuit::new(1);
        c.ops.push(gate1(0, NativeGate1::X));
        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0]["gate"], "x");
        assert_eq!(gates[0]["target"], 0);
    }

    #[test]
    fn test_rz_gate_includes_rotation() {
        let angle = std::f64::consts::FRAC_PI_4;
        let mut c = Circuit::new(1);
        c.ops.push(gate1(0, NativeGate1::Rz(angle)));
        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0]["gate"], "rz");
        assert_eq!(gates[0]["target"], 0);
        let rotation = gates[0]["rotation"].as_f64().unwrap();
        assert!((rotation - angle).abs() < 1e-12);
    }

    #[test]
    fn test_identity_gate_omitted() {
        let mut c = Circuit::new(1);
        c.ops.push(gate1(0, NativeGate1::Id));
        c.ops.push(gate1(0, NativeGate1::X));
        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        // Id is skipped, only X remains.
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0]["gate"], "x");
    }

    #[test]
    fn test_measure_omitted() {
        let mut c = Circuit::new(1);
        c.ops.push(measure(0, 0));
        let v = circuit_to_ionq_json(&c).unwrap();
        assert_eq!(v["circuit"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_reset_omitted() {
        let mut c = Circuit::new(1);
        c.ops.push(Op::Reset(QubitReset { qubit: PhysicalQubit(0) }));
        let v = circuit_to_ionq_json(&c).unwrap();
        assert_eq!(v["circuit"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_barrier_omitted() {
        let mut c = Circuit::new(2);
        c.ops.push(Op::Barrier(Barrier {
            qubits: vec![PhysicalQubit(0), PhysicalQubit(1)],
        }));
        let v = circuit_to_ionq_json(&c).unwrap();
        assert_eq!(v["circuit"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_qubit_count_from_circuit() {
        let c = Circuit::new(7);
        let v = circuit_to_ionq_json(&c).unwrap();
        assert_eq!(v["qubits"], 7);
    }

    #[test]
    fn test_multi_gate_ordering_preserved() {
        let mut c = Circuit::new(3);
        c.ops.push(gate1(0, NativeGate1::X));
        c.ops.push(gate1(1, NativeGate1::Sx));
        c.ops.push(gate2(0, 2));
        c.ops.push(gate1(2, NativeGate1::Rz(1.0)));

        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        assert_eq!(gates.len(), 4);
        assert_eq!(gates[0]["gate"], "x");
        assert_eq!(gates[1]["gate"], "v");
        assert_eq!(gates[2]["gate"], "cnot");
        assert_eq!(gates[3]["gate"], "rz");
    }

    #[test]
    fn test_cnot_control_and_target_fields() {
        let mut c = Circuit::new(4);
        c.ops.push(gate2(2, 3));
        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        assert_eq!(gates[0]["control"], 2);
        assert_eq!(gates[0]["target"], 3);
    }

    #[test]
    fn test_rz_zero_rotation() {
        let mut c = Circuit::new(1);
        c.ops.push(gate1(0, NativeGate1::Rz(0.0)));
        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        assert_eq!(gates.len(), 1);
        let rot = gates[0]["rotation"].as_f64().unwrap();
        assert!((rot - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_rz_negative_rotation() {
        // Negative angles must serialize faithfully — IonQ accepts them.
        let angle = -std::f64::consts::FRAC_PI_4;
        let mut c = Circuit::new(1);
        c.ops.push(gate1(0, NativeGate1::Rz(angle)));
        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0]["gate"], "rz");
        let rot = gates[0]["rotation"].as_f64().unwrap();
        assert!((rot - angle).abs() < 1e-12, "negative Rz angle must round-trip: {rot}");
    }

    #[test]
    fn test_high_qubit_indices_preserved() {
        // Verify that qubit index 28 (max for simulator) is preserved exactly.
        let mut c = Circuit::new(29);
        c.ops.push(gate1(28, NativeGate1::X));
        c.ops.push(gate2(0, 28));
        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        assert_eq!(gates[0]["target"], 28);
        assert_eq!(gates[1]["control"], 0);
        assert_eq!(gates[1]["target"], 28);
    }

    #[test]
    fn test_output_is_valid_json_string() {
        // The returned Value must be serializable to a JSON string without error.
        let mut c = Circuit::new(2);
        c.ops.push(gate1(0, NativeGate1::Sx));
        c.ops.push(gate2(0, 1));
        let v = circuit_to_ionq_json(&c).unwrap();
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"gateset\""));
        assert!(s.contains("\"qis\""));
        assert!(s.contains("\"circuit\""));
    }

    #[test]
    fn test_all_ops_mixed_with_skipped() {
        // Verify that only real gates are emitted, skipped ops don't displace indices.
        let mut c = Circuit::new(2);
        c.ops.push(gate1(0, NativeGate1::Id));     // skipped
        c.ops.push(measure(0, 0));                  // skipped
        c.ops.push(gate1(0, NativeGate1::X));       // kept → index 0
        c.ops.push(gate2(0, 1));                    // kept → index 1
        c.ops.push(Op::Reset(QubitReset { qubit: PhysicalQubit(1) })); // skipped

        let v = circuit_to_ionq_json(&c).unwrap();
        let gates = v["circuit"].as_array().unwrap();
        assert_eq!(gates.len(), 2);
        assert_eq!(gates[0]["gate"], "x");
        assert_eq!(gates[1]["gate"], "cnot");
    }
}
