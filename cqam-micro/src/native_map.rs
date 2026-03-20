//! Native gate mapping: translates standard gates to hardware-native gate sets.
//!
//! Currently implements the superconducting gate set: {SX, X, Rz, CX}.

use std::f64::consts::PI;

use cqam_core::circuit_ir;
use cqam_core::native_ir::{self, NativeGateSet, NativeGate1, NativeGate2,
    PhysicalQubit, Circuit, ApplyGate1q as NApplyGate1q, ApplyGate2q as NApplyGate2q};
use crate::error::MicroError;

/// Map a decomposed standard-gate MicroProgram to a native gate circuit.
///
/// Wire indices are mapped 1:1 to PhysicalQubit indices (routing has already
/// remapped wires if needed).
pub fn map_to_native(
    program: &circuit_ir::MicroProgram,
    gate_set: &NativeGateSet,
) -> Result<Circuit, MicroError> {
    match gate_set {
        NativeGateSet::Superconducting => map_superconducting(program),
        _ => Err(MicroError::UnsupportedGate {
            gate: format!("gate set {:?} not implemented in Phase 2", gate_set),
        }),
    }
}

/// Map to the IBM superconducting gate set: {SX, X, Rz, CX}.
fn map_superconducting(program: &circuit_ir::MicroProgram) -> Result<Circuit, MicroError> {
    let mut circuit = Circuit::new(program.num_wires);
    circuit.qubit_map = (0..program.num_wires)
        .map(PhysicalQubit)
        .collect();

    for op in &program.ops {
        match op {
            circuit_ir::Op::Gate1q(g) => {
                let q = PhysicalQubit(g.wire.0);
                let native_ops = map_gate1q_superconducting(q, &g.gate)?;
                circuit.ops.extend(native_ops);
            }
            circuit_ir::Op::Gate2q(g) => {
                let qa = PhysicalQubit(g.wire_a.0);
                let qb = PhysicalQubit(g.wire_b.0);
                let native_ops = map_gate2q_superconducting(qa, qb, &g.gate)?;
                circuit.ops.extend(native_ops);
            }
            circuit_ir::Op::Prep(_) => {
                // Physical reset assumed by hardware -- ignored
            }
            circuit_ir::Op::Measure(o) => {
                for (i, w) in o.wires.iter().enumerate() {
                    circuit.ops.push(native_ir::Op::Measure(native_ir::Observe {
                        qubit: PhysicalQubit(w.0),
                        clbit: i as u32,
                    }));
                }
            }
            circuit_ir::Op::Barrier(b) => {
                circuit.ops.push(native_ir::Op::Barrier(native_ir::Barrier {
                    qubits: b.wires.iter().map(|w| PhysicalQubit(w.0)).collect(),
                }));
            }
            circuit_ir::Op::Reset(r) => {
                circuit.ops.push(native_ir::Op::Reset(native_ir::QubitReset {
                    qubit: PhysicalQubit(r.wire.0),
                }));
            }
            circuit_ir::Op::MeasQubit { wire } => {
                circuit.ops.push(native_ir::Op::Measure(native_ir::Observe {
                    qubit: PhysicalQubit(wire.0),
                    clbit: wire.0,
                }));
            }
            circuit_ir::Op::Kernel(_) => {
                return Err(MicroError::UnsupportedGate {
                    gate: "Kernel op should have been decomposed".to_string(),
                });
            }
            circuit_ir::Op::CustomUnitary { .. } => {
                return Err(MicroError::UnsupportedGate {
                    gate: "CustomUnitary".to_string(),
                });
            }
            circuit_ir::Op::PrepProduct(_) => {
                return Err(MicroError::UnsupportedGate {
                    gate: "PrepProduct op should have been decomposed before native mapping".to_string(),
                });
            }
        }
    }

    // Calculate initial depth
    recalculate_depth(&mut circuit);

    Ok(circuit)
}

/// Map a single-qubit gate to superconducting native gates.
fn map_gate1q_superconducting(
    q: PhysicalQubit,
    gate: &circuit_ir::Gate1q,
) -> Result<Vec<native_ir::Op>, MicroError> {
    use circuit_ir::Gate1q;
    match gate {
        Gate1q::H => {
            // H = Rz(pi/2).SX.Rz(pi/2) up to global phase e^{-i*pi/4}.
            // This follows the IBM superconducting decomposition convention.
            // Using Rz(pi) instead would produce the wrong relative phase between
            // |0> and |1> components, breaking multi-gate circuits such as QFT.
            Ok(vec![
                g1(q, NativeGate1::Rz(PI / 2.0)),
                g1(q, NativeGate1::Sx),
                g1(q, NativeGate1::Rz(PI / 2.0)),
            ])
        }
        Gate1q::X => Ok(vec![g1(q, NativeGate1::X)]),
        Gate1q::Y => {
            // Y = Rz(pi).X up to global phase
            Ok(vec![
                g1(q, NativeGate1::Rz(PI)),
                g1(q, NativeGate1::X),
            ])
        }
        Gate1q::Z => Ok(vec![g1(q, NativeGate1::Rz(PI))]),
        Gate1q::S => Ok(vec![g1(q, NativeGate1::Rz(PI / 2.0))]),
        Gate1q::Sdg => Ok(vec![g1(q, NativeGate1::Rz(-PI / 2.0))]),
        Gate1q::T => Ok(vec![g1(q, NativeGate1::Rz(PI / 4.0))]),
        Gate1q::Tdg => Ok(vec![g1(q, NativeGate1::Rz(-PI / 4.0))]),
        Gate1q::Rx(p) => {
            let t = p.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "Rx".to_string(),
            })?;
            // Rx(t) = Rz(-pi/2).SX.Rz(pi-t).SX.Rz(-pi/2)
            Ok(vec![
                g1(q, NativeGate1::Rz(-PI / 2.0)),
                g1(q, NativeGate1::Sx),
                g1(q, NativeGate1::Rz(PI - t)),
                g1(q, NativeGate1::Sx),
                g1(q, NativeGate1::Rz(-PI / 2.0)),
            ])
        }
        Gate1q::Ry(p) => {
            let t = p.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "Ry".to_string(),
            })?;
            // Ry(t) = Rz(pi/2).SX.Rz(t-pi).SX.Rz(-pi/2)
            Ok(vec![
                g1(q, NativeGate1::Rz(PI / 2.0)),
                g1(q, NativeGate1::Sx),
                g1(q, NativeGate1::Rz(t - PI)),
                g1(q, NativeGate1::Sx),
                g1(q, NativeGate1::Rz(-PI / 2.0)),
            ])
        }
        Gate1q::Rz(p) => {
            let t = p.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "Rz".to_string(),
            })?;
            Ok(vec![g1(q, NativeGate1::Rz(t))])
        }
        Gate1q::U3(theta, phi, lambda) => {
            let t = theta.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "U3.theta".to_string(),
            })?;
            let p = phi.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "U3.phi".to_string(),
            })?;
            let l = lambda.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "U3.lambda".to_string(),
            })?;
            // U3(t,p,l) = Rz(l).SX.Rz(t+pi).SX.Rz(p+pi)
            Ok(vec![
                g1(q, NativeGate1::Rz(l)),
                g1(q, NativeGate1::Sx),
                g1(q, NativeGate1::Rz(t + PI)),
                g1(q, NativeGate1::Sx),
                g1(q, NativeGate1::Rz(p + PI)),
            ])
        }
        Gate1q::Custom(_) => Err(MicroError::UnsupportedGate {
            gate: "Custom 1q gate".to_string(),
        }),
    }
}

/// Map a two-qubit gate to superconducting native gates.
fn map_gate2q_superconducting(
    qa: PhysicalQubit,
    qb: PhysicalQubit,
    gate: &circuit_ir::Gate2q,
) -> Result<Vec<native_ir::Op>, MicroError> {
    use circuit_ir::Gate2q;
    match gate {
        Gate2q::Cx => Ok(vec![g2(qa, qb, NativeGate2::Cx)]),
        Gate2q::Cz => {
            // CZ = H(b).CX(a,b).H(b), where H = Rz(pi/2).SX.Rz(pi/2) (IBM convention).
            Ok(vec![
                g1(qb, NativeGate1::Rz(PI / 2.0)),
                g1(qb, NativeGate1::Sx),
                g1(qb, NativeGate1::Rz(PI / 2.0)),
                g2(qa, qb, NativeGate2::Cx),
                g1(qb, NativeGate1::Rz(PI / 2.0)),
                g1(qb, NativeGate1::Sx),
                g1(qb, NativeGate1::Rz(PI / 2.0)),
            ])
        }
        Gate2q::Swap => {
            // SWAP = CX(a,b).CX(b,a).CX(a,b)
            Ok(vec![
                g2(qa, qb, NativeGate2::Cx),
                g2(qb, qa, NativeGate2::Cx),
                g2(qa, qb, NativeGate2::Cx),
            ])
        }
        Gate2q::EchoCrossResonance => Err(MicroError::UnsupportedGate {
            gate: "EchoCrossResonance".to_string(),
        }),
        Gate2q::Custom(_) => Err(MicroError::UnsupportedGate {
            gate: "Custom 2q gate".to_string(),
        }),
    }
}

/// Helper: construct a native single-qubit gate op.
fn g1(q: PhysicalQubit, gate: NativeGate1) -> native_ir::Op {
    native_ir::Op::Gate1q(NApplyGate1q { qubit: q, gate })
}

/// Helper: construct a native two-qubit gate op.
fn g2(qa: PhysicalQubit, qb: PhysicalQubit, gate: NativeGate2) -> native_ir::Op {
    native_ir::Op::Gate2q(NApplyGate2q { qubit_a: qa, qubit_b: qb, gate })
}

/// Recalculate circuit depth.
pub(crate) fn recalculate_depth(circuit: &mut Circuit) {
    let n = circuit.num_physical_qubits as usize;
    if n == 0 {
        circuit.depth = 0;
        return;
    }
    let mut qubit_depth = vec![0u32; n];
    for op in &circuit.ops {
        match op {
            native_ir::Op::Gate1q(g) => {
                let idx = g.qubit.0 as usize;
                if idx < n {
                    qubit_depth[idx] += 1;
                }
            }
            native_ir::Op::Gate2q(g) => {
                let ia = g.qubit_a.0 as usize;
                let ib = g.qubit_b.0 as usize;
                if ia < n && ib < n {
                    let d = qubit_depth[ia].max(qubit_depth[ib]) + 1;
                    qubit_depth[ia] = d;
                    qubit_depth[ib] = d;
                }
            }
            _ => {}
        }
    }
    circuit.depth = qubit_depth.iter().copied().max().unwrap_or(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::circuit_ir::{MicroProgram, Op, QWire, ApplyGate1q, ApplyGate2q,
        Gate1q, Gate2q, Param, Observe, Barrier as CBarrier};
    use cqam_core::instruction::ObserveMode;

    fn make_program_with_gate1q(gate: Gate1q) -> MicroProgram {
        let mut mp = MicroProgram::new(1);
        mp.push(Op::Gate1q(ApplyGate1q { wire: QWire(0), gate }));
        mp
    }

    fn make_program_with_gate2q(gate: Gate2q) -> MicroProgram {
        let mut mp = MicroProgram::new(2);
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(0),
            wire_b: QWire(1),
            gate,
        }));
        mp
    }

    #[test]
    fn test_h_to_native() {
        let mp = make_program_with_gate1q(Gate1q::H);
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.ops.len(), 3); // Rz, SX, Rz
    }

    #[test]
    fn test_x_to_native() {
        let mp = make_program_with_gate1q(Gate1q::X);
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.ops.len(), 1);
    }

    #[test]
    fn test_rz_to_native() {
        let mp = make_program_with_gate1q(Gate1q::Rz(Param::Resolved(1.5)));
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.ops.len(), 1);
    }

    #[test]
    fn test_rx_to_native() {
        let mp = make_program_with_gate1q(Gate1q::Rx(Param::Resolved(1.0)));
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.ops.len(), 5); // Rz, SX, Rz, SX, Rz
    }

    #[test]
    fn test_ry_to_native() {
        let mp = make_program_with_gate1q(Gate1q::Ry(Param::Resolved(1.0)));
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.ops.len(), 5);
    }

    #[test]
    fn test_cx_to_native() {
        let mp = make_program_with_gate2q(Gate2q::Cx);
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.gate2q_count(), 1);
    }

    #[test]
    fn test_cz_to_native() {
        let mp = make_program_with_gate2q(Gate2q::Cz);
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        // H(b) + CX + H(b) = 3 + 1 + 3 = 7 ops
        assert_eq!(circuit.ops.len(), 7);
    }

    #[test]
    fn test_swap_to_native() {
        let mp = make_program_with_gate2q(Gate2q::Swap);
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.gate2q_count(), 3);
    }

    #[test]
    fn test_s_to_native() {
        let mp = make_program_with_gate1q(Gate1q::S);
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.ops.len(), 1);
    }

    #[test]
    fn test_t_to_native() {
        let mp = make_program_with_gate1q(Gate1q::T);
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.ops.len(), 1);
    }

    #[test]
    fn test_u3_to_native() {
        let mp = make_program_with_gate1q(Gate1q::U3(
            Param::Resolved(1.0),
            Param::Resolved(2.0),
            Param::Resolved(3.0),
        ));
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        assert_eq!(circuit.ops.len(), 5); // Rz, SX, Rz, SX, Rz
    }

    #[test]
    fn test_custom_gate_error() {
        let mp = make_program_with_gate1q(Gate1q::Custom(Box::new([
            cqam_core::complex::C64::ONE, cqam_core::complex::C64::ZERO,
            cqam_core::complex::C64::ZERO, cqam_core::complex::C64::ONE,
        ])));
        assert!(map_to_native(&mp, &NativeGateSet::Superconducting).is_err());
    }

    #[test]
    fn test_measure_mapping() {
        let mut mp = MicroProgram::new(2);
        mp.push(Op::Measure(Observe {
            wires: vec![QWire(0), QWire(1)],
            mode: ObserveMode::Dist,
            ctx0: 0,
            ctx1: 0,
        }));
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        let meas_count = circuit.ops.iter()
            .filter(|op| matches!(op, native_ir::Op::Measure(_)))
            .count();
        assert_eq!(meas_count, 2);
    }

    #[test]
    fn test_barrier_mapping() {
        let mut mp = MicroProgram::new(2);
        mp.push(Op::Barrier(CBarrier {
            wires: vec![QWire(0), QWire(1)],
        }));
        let circuit = map_to_native(&mp, &NativeGateSet::Superconducting).unwrap();
        let barrier_count = circuit.ops.iter()
            .filter(|op| matches!(op, native_ir::Op::Barrier(_)))
            .count();
        assert_eq!(barrier_count, 1);
    }
}
