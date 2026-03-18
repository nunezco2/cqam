//! Qubit routing: maps logical qubits to physical qubits with SWAP insertion.
//!
//! Implements a greedy SABRE-like algorithm using BFS shortest path for
//! SWAP placement.

use cqam_core::circuit_ir::{self, Op, QWire, ApplyGate2q, Gate2q};
use cqam_qpu::traits::{ConnectivityGraph, CalibrationData};
use crate::error::MicroError;

/// Result of the routing pass.
pub struct RoutingResult {
    /// Final virtual -> physical qubit mapping.
    pub virtual_to_physical: Vec<u32>,
    /// Number of SWAP gates inserted.
    pub swaps_inserted: u32,
}

/// Route a circuit onto a device connectivity graph.
///
/// For all-to-all connectivity, this is a no-op (zero SWAPs).
/// For constrained topologies, inserts SWAP gates to bring two-qubit
/// gate operands onto adjacent physical qubits.
pub fn route(
    program: &circuit_ir::MicroProgram,
    connectivity: &ConnectivityGraph,
    _calibration: Option<&dyn CalibrationData>,
) -> Result<(circuit_ir::MicroProgram, RoutingResult), MicroError> {
    let n = program.num_wires as usize;

    // Check if all-to-all (common case for simulators)
    let is_all_to_all = is_fully_connected(connectivity, n);

    if is_all_to_all || n <= 1 {
        // No routing needed
        let result = RoutingResult {
            virtual_to_physical: (0..n as u32).collect(),
            swaps_inserted: 0,
        };
        return Ok((program.clone(), result));
    }

    // Initialize mappings
    let mut v2p: Vec<u32> = (0..n as u32).collect();
    let mut p2v: Vec<u32> = (0..n as u32).collect();
    let mut swaps_inserted = 0u32;

    let mut out = circuit_ir::MicroProgram::new(program.num_wires);
    out.wire_map = program.wire_map.clone();

    for op in &program.ops {
        match op {
            Op::Gate2q(g) => {
                let va = g.wire_a.0 as usize;
                let vb = g.wire_b.0 as usize;
                let pa = v2p[va];
                let pb = v2p[vb];

                if connectivity.are_connected(pa, pb) {
                    // Directly executable: remap and emit
                    out.push(Op::Gate2q(ApplyGate2q {
                        wire_a: QWire(pa),
                        wire_b: QWire(pb),
                        gate: g.gate.clone(),
                    }));
                } else {
                    // Need SWAPs: find shortest path
                    let path = connectivity.shortest_path(pa, pb);
                    if path.len() < 2 {
                        return Err(MicroError::RoutingFailed {
                            detail: format!(
                                "no path between physical qubits {} and {}", pa, pb
                            ),
                        });
                    }

                    // Insert SWAPs along the path to bring pa adjacent to pb
                    for i in 0..(path.len() - 2) {
                        let p_from = path[i];
                        let p_to = path[i + 1];

                        // Find virtual qubits at these physical positions
                        let v_from = p2v[p_from as usize];
                        let v_to = p2v[p_to as usize];

                        // Emit SWAP using physical qubit wires
                        out.push(Op::Gate2q(ApplyGate2q {
                            wire_a: QWire(p_from),
                            wire_b: QWire(p_to),
                            gate: Gate2q::Swap,
                        }));

                        // Update mappings
                        v2p[v_from as usize] = p_to;
                        v2p[v_to as usize] = p_from;
                        p2v[p_from as usize] = v_to;
                        p2v[p_to as usize] = v_from;

                        swaps_inserted += 1;
                    }

                    // Now pa's virtual qubit is at path[path.len()-2], adjacent to pb
                    let new_pa = v2p[va];
                    let new_pb = v2p[vb];
                    debug_assert!(connectivity.are_connected(new_pa, new_pb));

                    out.push(Op::Gate2q(ApplyGate2q {
                        wire_a: QWire(new_pa),
                        wire_b: QWire(new_pb),
                        gate: g.gate.clone(),
                    }));
                }
            }
            Op::Gate1q(g) => {
                let v = g.wire.0 as usize;
                let p = v2p[v];
                out.push(Op::Gate1q(circuit_ir::ApplyGate1q {
                    wire: QWire(p),
                    gate: g.gate.clone(),
                }));
            }
            Op::Measure(o) => {
                let mapped_wires: Vec<QWire> = o.wires.iter()
                    .map(|w| QWire(v2p[w.0 as usize]))
                    .collect();
                out.push(Op::Measure(circuit_ir::Observe {
                    wires: mapped_wires,
                    mode: o.mode,
                    ctx0: o.ctx0,
                    ctx1: o.ctx1,
                }));
            }
            Op::Barrier(b) => {
                let mapped_wires: Vec<QWire> = b.wires.iter()
                    .map(|w| QWire(v2p[w.0 as usize]))
                    .collect();
                out.push(Op::Barrier(circuit_ir::Barrier { wires: mapped_wires }));
            }
            Op::Reset(r) => {
                let p = v2p[r.wire.0 as usize];
                out.push(Op::Reset(circuit_ir::Reset { wire: QWire(p) }));
            }
            Op::MeasQubit { wire } => {
                let p = v2p[wire.0 as usize];
                out.push(Op::MeasQubit { wire: QWire(p) });
            }
            Op::Prep(pr) => {
                let mapped_wires: Vec<QWire> = pr.wires.iter()
                    .map(|w| QWire(v2p[w.0 as usize]))
                    .collect();
                out.push(Op::Prep(circuit_ir::Prepare {
                    wires: mapped_wires,
                    dist: pr.dist,
                }));
            }
            Op::Kernel(_) => {
                return Err(MicroError::RoutingFailed {
                    detail: "Kernel ops must be decomposed before routing".to_string(),
                });
            }
            Op::CustomUnitary { .. } => {
                return Err(MicroError::UnsupportedGate {
                    gate: "CustomUnitary in routing".to_string(),
                });
            }
        }
    }

    let result = RoutingResult {
        virtual_to_physical: v2p,
        swaps_inserted,
    };
    Ok((out, result))
}

/// Check if the connectivity graph is fully connected (all-to-all).
///
/// For n == 0 or n == 1 there are no edges required; always return true.
/// Guarding here prevents a `usize` underflow when n == 0.
fn is_fully_connected(connectivity: &ConnectivityGraph, n: usize) -> bool {
    if n <= 1 {
        return true;
    }
    let expected = n * (n - 1) / 2;
    connectivity.num_edges() >= expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::circuit_ir::{MicroProgram, Op, QWire, ApplyGate1q, ApplyGate2q,
        Gate1q, Gate2q};

    #[test]
    fn test_route_all_to_all_no_swaps() {
        let mut mp = MicroProgram::new(3);
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(0), wire_b: QWire(2), gate: Gate2q::Cx,
        }));
        let conn = ConnectivityGraph::all_to_all(3);
        let (routed, result) = route(&mp, &conn, None).unwrap();
        assert_eq!(result.swaps_inserted, 0);
        assert_eq!(routed.ops.len(), 1);
    }

    #[test]
    fn test_route_linear_adjacent_cx() {
        let mut mp = MicroProgram::new(3);
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(0), wire_b: QWire(1), gate: Gate2q::Cx,
        }));
        let conn = ConnectivityGraph::linear(3);
        let (_, result) = route(&mp, &conn, None).unwrap();
        assert_eq!(result.swaps_inserted, 0);
    }

    #[test]
    fn test_route_linear_distant_cx() {
        let mut mp = MicroProgram::new(3);
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(0), wire_b: QWire(2), gate: Gate2q::Cx,
        }));
        let conn = ConnectivityGraph::linear(3);
        let (_, result) = route(&mp, &conn, None).unwrap();
        assert!(result.swaps_inserted >= 1,
            "Expected at least 1 SWAP, got {}", result.swaps_inserted);
    }

    #[test]
    fn test_route_preserves_single_qubit_gates() {
        let mut mp = MicroProgram::new(3);
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0), gate: Gate1q::H,
        }));
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(2), gate: Gate1q::X,
        }));
        let conn = ConnectivityGraph::linear(3);
        let (routed, result) = route(&mp, &conn, None).unwrap();
        assert_eq!(result.swaps_inserted, 0);
        assert_eq!(routed.ops.len(), 2);
    }

    #[test]
    fn test_route_empty_program() {
        let mp = MicroProgram::new(3);
        let conn = ConnectivityGraph::linear(3);
        let (routed, result) = route(&mp, &conn, None).unwrap();
        assert_eq!(result.swaps_inserted, 0);
        assert!(routed.ops.is_empty());
    }

    #[test]
    fn test_route_no_two_qubit_gates() {
        let mut mp = MicroProgram::new(3);
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0), gate: Gate1q::H,
        }));
        let conn = ConnectivityGraph::linear(3);
        let (_, result) = route(&mp, &conn, None).unwrap();
        assert_eq!(result.swaps_inserted, 0);
    }

    #[test]
    fn test_route_multiple_distant_cx() {
        let mut mp = MicroProgram::new(4);
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(0), wire_b: QWire(3), gate: Gate2q::Cx,
        }));
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(1), wire_b: QWire(3), gate: Gate2q::Cx,
        }));
        let conn = ConnectivityGraph::linear(4);
        let (_, result) = route(&mp, &conn, None).unwrap();
        assert!(result.swaps_inserted >= 1);
    }
}
