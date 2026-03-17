//! Circuit optimization passes for native gate circuits.
//!
//! Passes: gate cancellation, Rz merging, commutation through CX controls.
//! Applied in fixed-point iteration until no further reductions.

use std::f64::consts::PI;
use cqam_core::native_ir::{Circuit, Op, NativeGate1, NativeGate2, ApplyGate1q};

use crate::native_map::recalculate_depth;

/// Apply optimization passes to a native circuit in-place.
/// Passes are applied in order: cancellation, merging, commutation.
/// The sequence is repeated until no further reductions are made (fixed-point).
pub fn optimize(circuit: &mut Circuit) {
    // Run cancellation+merging in a fixed-point loop.
    // After convergence, run commutation, then one more round of
    // cancellation+merging to pick up newly exposed opportunities.
    loop {
        let before = circuit.ops.len();
        pass_cancellation(circuit);
        pass_merge_rz(circuit);
        if circuit.ops.len() == before {
            break;
        }
    }
    // Commutation pass, then one more round
    pass_commutation(circuit);
    loop {
        let before = circuit.ops.len();
        pass_cancellation(circuit);
        pass_merge_rz(circuit);
        if circuit.ops.len() == before {
            break;
        }
    }
    recalculate_depth(circuit);
}

/// Find the next gate that touches a given qubit, starting from position `start`.
/// Returns the index if found, or None.
fn next_gate_on_qubit(ops: &[Op], start: usize, qubit: u32) -> Option<usize> {
    (start..ops.len()).find(|&i| gate_touches_qubit(&ops[i], qubit))
}

/// Check if an op touches a given qubit.
fn gate_touches_qubit(op: &Op, qubit: u32) -> bool {
    match op {
        Op::Gate1q(g) => g.qubit.0 == qubit,
        Op::Gate2q(g) => g.qubit_a.0 == qubit || g.qubit_b.0 == qubit,
        Op::Barrier(b) => b.qubits.iter().any(|q| q.0 == qubit),
        Op::Measure(m) => m.qubit.0 == qubit,
        Op::Reset(r) => r.qubit.0 == qubit,
    }
}

/// Check if an op is a barrier (which blocks all optimizations).
fn is_barrier(op: &Op) -> bool {
    matches!(op, Op::Barrier(_))
}

/// Pass 1: Gate cancellation.
/// - X.X -> remove both
/// - SX.SX -> X
/// - CX(a,b).CX(a,b) -> remove both
fn pass_cancellation(circuit: &mut Circuit) {
    let mut to_remove = Vec::new();
    let mut to_replace = Vec::new();
    let ops = &circuit.ops;
    let len = ops.len();

    let mut i = 0;
    while i < len {
        if to_remove.contains(&i) {
            i += 1;
            continue;
        }

        match &ops[i] {
            Op::Gate1q(g) => {
                let qubit = g.qubit.0;
                match &g.gate {
                    NativeGate1::X => {
                        // Look for next gate on same qubit
                        if let Some(j) = next_gate_on_qubit(ops, i + 1, qubit) {
                            if to_remove.contains(&j) {
                                i += 1;
                                continue;
                            }
                            // Check no barrier between i and j
                            let has_barrier = (i+1..j).any(|k| is_barrier(&ops[k]));
                            if !has_barrier {
                                if let Op::Gate1q(g2) = &ops[j] {
                                    if matches!(g2.gate, NativeGate1::X) && g2.qubit.0 == qubit {
                                        to_remove.push(i);
                                        to_remove.push(j);
                                    }
                                }
                            }
                        }
                    }
                    NativeGate1::Sx => {
                        if let Some(j) = next_gate_on_qubit(ops, i + 1, qubit) {
                            if to_remove.contains(&j) {
                                i += 1;
                                continue;
                            }
                            let has_barrier = (i+1..j).any(|k| is_barrier(&ops[k]));
                            if !has_barrier {
                                if let Op::Gate1q(g2) = &ops[j] {
                                    if matches!(g2.gate, NativeGate1::Sx) && g2.qubit.0 == qubit {
                                        // SX.SX -> X
                                        to_replace.push((i, Op::Gate1q(ApplyGate1q {
                                            qubit: g.qubit,
                                            gate: NativeGate1::X,
                                        })));
                                        to_remove.push(j);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Op::Gate2q(g) => {
                if matches!(g.gate, NativeGate2::Cx) {
                    let qa = g.qubit_a.0;
                    let qb = g.qubit_b.0;
                    // Look for next CX on same pair
                    if let Some(j) = find_next_cx(ops, i + 1, qa, qb) {
                        if !to_remove.contains(&j) {
                            // Check no gate on either qubit between i and j
                            let blocked = (i+1..j).any(|k| {
                                gate_touches_qubit(&ops[k], qa) || gate_touches_qubit(&ops[k], qb)
                            });
                            if !blocked {
                                to_remove.push(i);
                                to_remove.push(j);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Apply replacements first
    for (idx, new_op) in &to_replace {
        circuit.ops[*idx] = new_op.clone();
    }

    // Remove marked ops (in reverse order to preserve indices)
    to_remove.sort();
    to_remove.dedup();
    for &idx in to_remove.iter().rev() {
        circuit.ops.remove(idx);
    }
}

/// Find next CX gate with same control and target.
fn find_next_cx(ops: &[Op], start: usize, ctrl: u32, tgt: u32) -> Option<usize> {
    ops.iter().enumerate().skip(start).find_map(|(i, op)| {
        if let Op::Gate2q(g) = op {
            if matches!(g.gate, NativeGate2::Cx) && g.qubit_a.0 == ctrl && g.qubit_b.0 == tgt {
                return Some(i);
            }
        }
        None
    })
}

/// Pass 2: Rz merging.
/// - Rz(a).Rz(b) -> Rz(a+b)
/// - Remove Rz if angle is 0 mod 2pi.
fn pass_merge_rz(circuit: &mut Circuit) {
    let mut to_remove = Vec::new();
    let len = circuit.ops.len();

    let mut i = 0;
    while i < len {
        if to_remove.contains(&i) {
            i += 1;
            continue;
        }

        if let Op::Gate1q(g) = &circuit.ops[i] {
            if let NativeGate1::Rz(angle_a) = g.gate {
                let qubit = g.qubit.0;
                if let Some(j) = next_gate_on_qubit(&circuit.ops, i + 1, qubit) {
                    if to_remove.contains(&j) {
                        i += 1;
                        continue;
                    }
                    let has_barrier = (i+1..j).any(|k| is_barrier(&circuit.ops[k]));
                    if !has_barrier {
                        if let Op::Gate1q(g2) = &circuit.ops[j] {
                            if let NativeGate1::Rz(angle_b) = g2.gate {
                                if g2.qubit.0 == qubit {
                                    let merged = angle_a + angle_b;
                                    // Check if effectively zero
                                    let normalized = merged.rem_euclid(2.0 * PI);
                                    if normalized.abs() < 1e-10 || (normalized - 2.0 * PI).abs() < 1e-10 {
                                        to_remove.push(i);
                                        to_remove.push(j);
                                    } else {
                                        circuit.ops[i] = Op::Gate1q(ApplyGate1q {
                                            qubit: g.qubit,
                                            gate: NativeGate1::Rz(merged),
                                        });
                                        to_remove.push(j);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    to_remove.sort();
    to_remove.dedup();
    for &idx in to_remove.iter().rev() {
        circuit.ops.remove(idx);
    }
}

/// Pass 3: Commutation.
/// Move Rz gates through CX control wires to enable further merging.
fn pass_commutation(circuit: &mut Circuit) {
    let mut i = 0;
    while i + 1 < circuit.ops.len() {
        // Check if ops[i] is Rz and ops[i+1] is CX where the Rz qubit is the CX control
        let should_swap = {
            if let (Op::Gate1q(g1), Op::Gate2q(g2)) = (&circuit.ops[i], &circuit.ops[i + 1]) {
                if let NativeGate1::Rz(_) = g1.gate {
                    if matches!(g2.gate, NativeGate2::Cx) {
                        // Rz commutes with CX on the control wire
                        g1.qubit.0 == g2.qubit_a.0
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_swap {
            circuit.ops.swap(i, i + 1);
            // Don't increment i so we can try to push further
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::native_ir::{Circuit, Op, NativeGate1, NativeGate2,
        ApplyGate1q, ApplyGate2q, PhysicalQubit, Barrier};

    fn make_circuit(ops: Vec<Op>) -> Circuit {
        let mut c = Circuit::new(4);
        c.ops = ops;
        c
    }

    fn rz_op(qubit: u32, angle: f64) -> Op {
        Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(qubit),
            gate: NativeGate1::Rz(angle),
        })
    }

    fn x_op(qubit: u32) -> Op {
        Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(qubit),
            gate: NativeGate1::X,
        })
    }

    fn sx_op(qubit: u32) -> Op {
        Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(qubit),
            gate: NativeGate1::Sx,
        })
    }

    fn cx_op(ctrl: u32, tgt: u32) -> Op {
        Op::Gate2q(ApplyGate2q {
            qubit_a: PhysicalQubit(ctrl),
            qubit_b: PhysicalQubit(tgt),
            gate: NativeGate2::Cx,
        })
    }

    fn barrier_op(qubits: Vec<u32>) -> Op {
        Op::Barrier(Barrier {
            qubits: qubits.into_iter().map(PhysicalQubit).collect(),
        })
    }

    #[test]
    fn test_cancel_x_x() {
        let mut c = make_circuit(vec![x_op(0), x_op(0)]);
        optimize(&mut c);
        assert_eq!(c.ops.len(), 0);
    }

    #[test]
    fn test_cancel_cx_cx() {
        let mut c = make_circuit(vec![cx_op(0, 1), cx_op(0, 1)]);
        optimize(&mut c);
        assert_eq!(c.ops.len(), 0);
    }

    #[test]
    fn test_merge_rz_rz() {
        let mut c = make_circuit(vec![rz_op(0, 1.0), rz_op(0, 2.0)]);
        optimize(&mut c);
        assert_eq!(c.ops.len(), 1);
        if let Op::Gate1q(g) = &c.ops[0] {
            if let NativeGate1::Rz(angle) = g.gate {
                assert!((angle - 3.0).abs() < 1e-10);
            } else {
                panic!("Expected Rz gate");
            }
        }
    }

    #[test]
    fn test_merge_rz_to_zero() {
        let mut c = make_circuit(vec![rz_op(0, PI), rz_op(0, -PI)]);
        optimize(&mut c);
        assert_eq!(c.ops.len(), 0, "Rz(pi)+Rz(-pi) should cancel");
    }

    #[test]
    fn test_merge_rz_to_2pi() {
        let mut c = make_circuit(vec![rz_op(0, PI), rz_op(0, PI)]);
        optimize(&mut c);
        assert_eq!(c.ops.len(), 0, "Rz(pi)+Rz(pi) = Rz(2pi) should be removed");
    }

    #[test]
    fn test_sx_sx_to_x() {
        let mut c = make_circuit(vec![sx_op(0), sx_op(0)]);
        optimize(&mut c);
        assert_eq!(c.ops.len(), 1);
        if let Op::Gate1q(g) = &c.ops[0] {
            assert!(matches!(g.gate, NativeGate1::X));
        } else {
            panic!("Expected X gate");
        }
    }

    #[test]
    fn test_commute_rz_through_cx_control() {
        // Rz on qubit 0 (CX control) should commute past CX(0,1)
        let mut c = make_circuit(vec![rz_op(0, 1.0), cx_op(0, 1), rz_op(0, 2.0)]);
        optimize(&mut c);
        // After commutation, Rz(1.0) and Rz(2.0) are adjacent -> merge to Rz(3.0)
        let rz_count = c.ops.iter().filter(|op| {
            matches!(op, Op::Gate1q(g) if matches!(g.gate, NativeGate1::Rz(_)))
        }).count();
        assert!(rz_count <= 1, "Expected Rz gates to merge after commutation, got {} Rz gates", rz_count);
    }

    #[test]
    fn test_no_commute_rz_through_cx_target() {
        // Rz on qubit 1 (CX target) should NOT commute
        let mut c = make_circuit(vec![rz_op(1, 1.0), cx_op(0, 1), rz_op(1, 2.0)]);
        let before = c.ops.len();
        optimize(&mut c);
        // Rz gates should remain separate (cannot merge through CX target)
        assert_eq!(c.ops.len(), before, "Rz should not commute through CX target");
    }

    #[test]
    fn test_barrier_blocks_cancellation() {
        let mut c = make_circuit(vec![
            x_op(0),
            barrier_op(vec![0]),
            x_op(0),
        ]);
        optimize(&mut c);
        // X.X should NOT cancel because barrier blocks
        assert!(c.ops.len() >= 3);
    }

    #[test]
    fn test_depth_recalculation() {
        let mut c = make_circuit(vec![
            rz_op(0, 1.0),
            sx_op(0),
            cx_op(0, 1),
            rz_op(1, 2.0),
        ]);
        optimize(&mut c);
        assert!(c.depth > 0);
    }

    #[test]
    fn test_preserve_unitary() {
        // Simple circuit: X, Rz, CX -- should not be modified
        let mut c = make_circuit(vec![
            x_op(0),
            rz_op(1, 1.5),
            cx_op(0, 1),
        ]);
        optimize(&mut c);
        assert_eq!(c.ops.len(), 3);
    }
}
