//! Generic controlled-gate wrapper.
//!
//! Transforms a sequence of standard circuit IR ops into their controlled
//! versions by adding a control qubit. Each gate G becomes C(G) -- the gate
//! is applied only when the control qubit is |1>.

use std::f64::consts::PI;
use cqam_core::circuit_ir::{
    ApplyGate1q, ApplyGate2q, Barrier, Gate1q, Gate2q, Op, Param, QWire,
};
use crate::error::MicroError;
use super::helpers::{cx, h, t_gate, tdg};

// =============================================================================
// Public entry point
// =============================================================================

/// Add a control qubit to every gate in a sequence of ops.
///
/// For each gate G in `ops`, produces C(G) -- the controlled version
/// conditioned on `ctrl` being |1>. Non-gate ops (Barrier) pass through
/// unchanged. Prep, Measure, Reset, and Kernel ops are rejected -- they
/// must be decomposed to gates before calling this function.
///
/// # Errors
///
/// Returns `MicroError::DecompositionFailed` if the input contains ops
/// that cannot be controlled (Prep, Measure, Kernel, CustomUnitary).
pub fn add_control(ctrl: QWire, ops: &[Op]) -> Result<Vec<Op>, MicroError> {
    let mut result = Vec::new();
    for op in ops {
        match op {
            Op::Gate1q(g) => {
                result.extend(controlled_gate1q(ctrl, g.wire, &g.gate)?);
            }
            Op::Gate2q(g) => {
                result.extend(controlled_gate2q(ctrl, g.wire_a, g.wire_b, &g.gate)?);
            }
            Op::Barrier(b) => {
                // Barriers pass through -- include ctrl in the barrier wire set.
                let mut wires = b.wires.clone();
                if !wires.contains(&ctrl) {
                    wires.push(ctrl);
                }
                result.push(Op::Barrier(Barrier { wires }));
            }
            Op::Prep(_) | Op::Measure(_) | Op::Reset(_) | Op::MeasQubit { .. } => {
                return Err(MicroError::DecompositionFailed {
                    kernel: "add_control".to_string(),
                    detail: format!(
                        "cannot add control to non-unitary op: {:?}",
                        std::mem::discriminant(op)
                    ),
                });
            }
            Op::Kernel(_) => {
                return Err(MicroError::DecompositionFailed {
                    kernel: "add_control".to_string(),
                    detail: "Kernel op must be decomposed before add_control".to_string(),
                });
            }
            Op::CustomUnitary { .. } => {
                return Err(MicroError::DecompositionFailed {
                    kernel: "add_control".to_string(),
                    detail: "CustomUnitary must be decomposed before add_control".to_string(),
                });
            }
            Op::PrepProduct(_) => {
                return Err(MicroError::DecompositionFailed {
                    kernel: "add_control".to_string(),
                    detail: "PrepProduct must be decomposed before add_control".to_string(),
                });
            }
        }
    }
    Ok(result)
}

// =============================================================================
// Controlled single-qubit gates
// =============================================================================

/// Produce the controlled version of a single-qubit gate.
///
/// Returns the equivalent gate sequence for C(gate) with control=ctrl,
/// target=tgt. All decompositions produce only gates from the standard set
/// (H, X, T, Tdg, Rz, CX) -- no recursive controlled calls.
fn controlled_gate1q(ctrl: QWire, tgt: QWire, gate: &Gate1q) -> Result<Vec<Op>, MicroError> {
    match gate {
        // CX (CNOT) -- X on target conditioned on ctrl.
        Gate1q::X => Ok(vec![cx(ctrl, tgt)]),

        // CY -- decompose as Sdg(t) . CX(c,t) . S(t)
        Gate1q::Y => Ok(vec![
            Op::Gate1q(ApplyGate1q { wire: tgt, gate: Gate1q::Sdg }),
            cx(ctrl, tgt),
            Op::Gate1q(ApplyGate1q { wire: tgt, gate: Gate1q::S }),
        ]),

        // CZ -- natively a 2q gate in the IR.
        Gate1q::Z => Ok(vec![Op::Gate2q(ApplyGate2q {
            wire_a: ctrl,
            wire_b: tgt,
            gate: Gate2q::Cz,
        })]),

        // CH (controlled-Hadamard)
        //   Sdg(t) . H(t) . Tdg(t) . CX(c,t) . T(t) . H(t) . S(t)
        // This is the standard phase-exact decomposition.
        Gate1q::H => Ok(vec![
            Op::Gate1q(ApplyGate1q { wire: tgt, gate: Gate1q::Sdg }),
            h(tgt),
            tdg(tgt),
            cx(ctrl, tgt),
            t_gate(tgt),
            h(tgt),
            Op::Gate1q(ApplyGate1q { wire: tgt, gate: Gate1q::S }),
        ]),

        // CS (controlled-S = controlled-Rz(pi/2))
        Gate1q::S => Ok(controlled_rz_sequence(ctrl, tgt, PI / 2.0)),

        // CSdg (controlled-Sdg = controlled-Rz(-pi/2))
        Gate1q::Sdg => Ok(controlled_rz_sequence(ctrl, tgt, -PI / 2.0)),

        // CT (controlled-T = controlled-Rz(pi/4))
        Gate1q::T => Ok(controlled_rz_sequence(ctrl, tgt, PI / 4.0)),

        // CTdg (controlled-Tdg = controlled-Rz(-pi/4))
        Gate1q::Tdg => Ok(controlled_rz_sequence(ctrl, tgt, -PI / 4.0)),

        // CRz (controlled-Rz(theta))
        //   Rz(theta/2, t) . CX(c,t) . Rz(-theta/2, t) . CX(c,t) . Rz(theta/2, c)
        Gate1q::Rz(param) => {
            let theta = param.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "add_control: Rz".to_string(),
            })?;
            Ok(controlled_rz_sequence(ctrl, tgt, theta))
        }

        // CRx (controlled-Rx(theta))
        //   Rz(pi/2, t) . Ry(theta/2, t) . CX(c,t) . Ry(-theta/2, t) . CX(c,t) . Rz(-pi/2, t)
        Gate1q::Rx(param) => {
            let theta = param.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "add_control: Rx".to_string(),
            })?;
            Ok(vec![
                rz(tgt, PI / 2.0),
                Op::Gate1q(ApplyGate1q {
                    wire: tgt,
                    gate: Gate1q::Ry(Param::Resolved(theta / 2.0)),
                }),
                cx(ctrl, tgt),
                Op::Gate1q(ApplyGate1q {
                    wire: tgt,
                    gate: Gate1q::Ry(Param::Resolved(-theta / 2.0)),
                }),
                cx(ctrl, tgt),
                rz(tgt, -PI / 2.0),
            ])
        }

        // CRy (controlled-Ry(theta))
        //   Ry(theta/2, t) . CX(c,t) . Ry(-theta/2, t) . CX(c,t)
        Gate1q::Ry(param) => {
            let theta = param.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "add_control: Ry".to_string(),
            })?;
            Ok(vec![
                Op::Gate1q(ApplyGate1q {
                    wire: tgt,
                    gate: Gate1q::Ry(Param::Resolved(theta / 2.0)),
                }),
                cx(ctrl, tgt),
                Op::Gate1q(ApplyGate1q {
                    wire: tgt,
                    gate: Gate1q::Ry(Param::Resolved(-theta / 2.0)),
                }),
                cx(ctrl, tgt),
            ])
        }

        // CU3 (controlled-U3(theta, phi, lambda))
        //
        // Standard ABC decomposition (up to global phase on the target):
        //   Rz((lambda - phi)/2, t)
        //   CX(c, t)
        //   Rz(-(lambda + phi)/2, t) . Ry(-theta/2, t)
        //   CX(c, t)
        //   Ry(theta/2, t) . Rz(phi, t)
        Gate1q::U3(theta_p, phi_p, lambda_p) => {
            let theta = theta_p.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "add_control: U3.theta".to_string(),
            })?;
            let phi = phi_p.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "add_control: U3.phi".to_string(),
            })?;
            let lambda = lambda_p.value().ok_or_else(|| MicroError::UnresolvedParam {
                context: "add_control: U3.lambda".to_string(),
            })?;
            Ok(vec![
                rz(tgt, (lambda - phi) / 2.0),
                cx(ctrl, tgt),
                rz(tgt, -(lambda + phi) / 2.0),
                Op::Gate1q(ApplyGate1q {
                    wire: tgt,
                    gate: Gate1q::Ry(Param::Resolved(-theta / 2.0)),
                }),
                cx(ctrl, tgt),
                Op::Gate1q(ApplyGate1q {
                    wire: tgt,
                    gate: Gate1q::Ry(Param::Resolved(theta / 2.0)),
                }),
                rz(tgt, phi),
            ])
        }

        // Custom 1q gate -- cannot control without matrix decomposition.
        Gate1q::Custom(_) => Err(MicroError::DecompositionFailed {
            kernel: "add_control".to_string(),
            detail: "cannot add control to Custom 1q gate; decompose to U3 first".to_string(),
        }),
    }
}

// =============================================================================
// Controlled-Rz helper (shared by CRz, CS, CSdg, CT, CTdg)
// =============================================================================

/// Standard CRz(theta) decomposition: 2 Rz + 2 CX.
///
///   Rz(theta/2, t) . CX(c,t) . Rz(-theta/2, t) . CX(c,t)
///
/// Derivation: when ctrl=|0>, the two CX cancel and the Rz pair cancels, giving I.
/// When ctrl=|1>, the CX gates flip the target qubit twice and the Rz phases combine
/// to produce Rz(theta) on the target. The result is the CRz unitary (up to global
/// phase) without any phase correction on the control qubit.
fn controlled_rz_sequence(ctrl: QWire, tgt: QWire, theta: f64) -> Vec<Op> {
    vec![
        rz(tgt, theta / 2.0),
        cx(ctrl, tgt),
        rz(tgt, -theta / 2.0),
        cx(ctrl, tgt),
    ]
}

// =============================================================================
// Controlled two-qubit gates
// =============================================================================

/// Produce the controlled version of a two-qubit gate.
///
/// C(CX) = Toffoli(ctrl, a, b).
/// C(CZ) = CCZ(ctrl, a, b) = H(b) . Toffoli(ctrl, a, b) . H(b).
/// C(SWAP) = Fredkin gate = CX(b,a) . Toffoli(ctrl,a,b) . CX(b,a).
fn controlled_gate2q(
    ctrl: QWire,
    wire_a: QWire,
    wire_b: QWire,
    gate: &Gate2q,
) -> Result<Vec<Op>, MicroError> {
    match gate {
        Gate2q::Cx => Ok(toffoli(ctrl, wire_a, wire_b)),

        Gate2q::Cz => {
            let mut ops = vec![h(wire_b)];
            ops.extend(toffoli(ctrl, wire_a, wire_b));
            ops.push(h(wire_b));
            Ok(ops)
        }

        Gate2q::Swap => {
            let mut ops = Vec::new();
            ops.push(cx(wire_b, wire_a));
            ops.extend(toffoli(ctrl, wire_a, wire_b));
            ops.push(cx(wire_b, wire_a));
            Ok(ops)
        }

        Gate2q::EchoCrossResonance => Err(MicroError::DecompositionFailed {
            kernel: "add_control".to_string(),
            detail: "cannot add control to EchoCrossResonance".to_string(),
        }),

        Gate2q::Custom(_) => Err(MicroError::DecompositionFailed {
            kernel: "add_control".to_string(),
            detail: "cannot add control to Custom 2q gate".to_string(),
        }),
    }
}

// =============================================================================
// Toffoli decomposition (shared with grover.rs)
// =============================================================================

/// Decompose a Toffoli (CCX) gate into the standard 6-CNOT form.
///
/// CCX(c0, c1, target) = 15 gates: 6 CX + 2 H + 4 T/Tdg.
///
/// Circuit:
///   H(tgt)
///   CX(c1, tgt)   Tdg(tgt)
///   CX(c0, tgt)   T(tgt)
///   CX(c1, tgt)   Tdg(tgt)
///   CX(c0, tgt)   T(c1)   T(tgt)   H(tgt)
///   CX(c0, c1)    T(c0)   Tdg(c1)
///   CX(c0, c1)
///
/// Reference: Nielsen & Chuang, Fig. 4.9.
pub(super) fn toffoli(c0: QWire, c1: QWire, target: QWire) -> Vec<Op> {
    vec![
        h(target),
        cx(c1, target),
        tdg(target),
        cx(c0, target),
        t_gate(target),
        cx(c1, target),
        tdg(target),
        cx(c0, target),
        t_gate(c1),
        t_gate(target),
        h(target),
        cx(c0, c1),
        t_gate(c0),
        tdg(c1),
        cx(c0, c1),
    ]
}

// =============================================================================
// Local Rz helper (mirrors helpers::rz, available locally)
// =============================================================================

fn rz(wire: QWire, theta: f64) -> Op {
    Op::Gate1q(ApplyGate1q {
        wire,
        gate: Gate1q::Rz(Param::Resolved(theta)),
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::circuit_ir::{ApplyGate1q, ApplyGate2q, Gate1q, Gate2q, Op, QWire};
    use cqam_core::complex::C64;
    use super::super::tests::{gate_sequence_unitary, unitaries_equal_up_to_phase};

    fn make_cx(ctrl: QWire, tgt: QWire) -> Op {
        Op::Gate2q(ApplyGate2q { wire_a: ctrl, wire_b: tgt, gate: Gate2q::Cx })
    }

    fn make_h(wire: QWire) -> Op {
        Op::Gate1q(ApplyGate1q { wire, gate: Gate1q::H })
    }

    fn make_x(wire: QWire) -> Op {
        Op::Gate1q(ApplyGate1q { wire, gate: Gate1q::X })
    }

    // -------------------------------------------------------------------------
    // Controlled-X = CX
    // -------------------------------------------------------------------------

    #[test]
    fn test_controlled_x() {
        let ctrl = QWire(0);
        let tgt = QWire(1);
        let ops = vec![make_x(tgt)];
        let controlled = add_control(ctrl, &ops).unwrap();
        let u = gate_sequence_unitary(&controlled, 2);

        // Expected: CX matrix (big-endian: ctrl=qubit0, tgt=qubit1)
        let expected = vec![
            C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
            C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
            C64::ZERO, C64::ZERO, C64::ZERO, C64::ONE,
            C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
        ];
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-10),
            "controlled-X should equal CX"
        );
    }

    // -------------------------------------------------------------------------
    // Controlled-Z = CZ
    // -------------------------------------------------------------------------

    #[test]
    fn test_controlled_z() {
        let ctrl = QWire(0);
        let tgt = QWire(1);
        let ops = vec![Op::Gate1q(ApplyGate1q { wire: tgt, gate: Gate1q::Z })];
        let controlled = add_control(ctrl, &ops).unwrap();
        let u = gate_sequence_unitary(&controlled, 2);

        let expected = vec![
            C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
            C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
            C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
            C64::ZERO, C64::ZERO, C64::ZERO, C64(-1.0, 0.0),
        ];
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-10),
            "controlled-Z should equal CZ"
        );
    }

    // -------------------------------------------------------------------------
    // Controlled-H (CH)
    // -------------------------------------------------------------------------

    #[test]
    fn test_controlled_h() {
        let ctrl = QWire(0);
        let tgt = QWire(1);
        let ops = vec![make_h(tgt)];
        let controlled = add_control(ctrl, &ops).unwrap();
        let u = gate_sequence_unitary(&controlled, 2);

        // CH: acts as I when ctrl=|0>, H when ctrl=|1>
        let hv = std::f64::consts::FRAC_1_SQRT_2;
        let expected = vec![
            C64::ONE,  C64::ZERO,        C64::ZERO,       C64::ZERO,
            C64::ZERO, C64::ONE,         C64::ZERO,        C64::ZERO,
            C64::ZERO, C64::ZERO,        C64(hv, 0.0),    C64(hv, 0.0),
            C64::ZERO, C64::ZERO,        C64(hv, 0.0),    C64(-hv, 0.0),
        ];
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-10),
            "controlled-H unitary mismatch"
        );
    }

    // -------------------------------------------------------------------------
    // Controlled-Rz
    // -------------------------------------------------------------------------

    #[test]
    fn test_controlled_rz() {
        let ctrl = QWire(0);
        let tgt = QWire(1);
        let theta = std::f64::consts::PI / 2.0;
        let ops = vec![rz(tgt, theta)];
        let controlled = add_control(ctrl, &ops).unwrap();
        let u = gate_sequence_unitary(&controlled, 2);

        // CRz(pi/2): diagonal with phases (1, 1, e^{-i*pi/4}, e^{i*pi/4})
        let p = C64::exp_i(theta / 2.0);
        let m = C64::exp_i(-theta / 2.0);
        let expected = vec![
            C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
            C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
            C64::ZERO, C64::ZERO, m,         C64::ZERO,
            C64::ZERO, C64::ZERO, C64::ZERO, p,
        ];
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-10),
            "controlled-Rz(pi/2) unitary mismatch"
        );
    }

    // -------------------------------------------------------------------------
    // Controlled-CX = Toffoli
    // -------------------------------------------------------------------------

    #[test]
    fn test_controlled_cx_is_toffoli() {
        let ctrl = QWire(0);
        let a = QWire(1);
        let b = QWire(2);
        let ops = vec![make_cx(a, b)];
        let controlled = add_control(ctrl, &ops).unwrap();
        let u = gate_sequence_unitary(&controlled, 3);

        // Toffoli: 8x8 identity except |110> <-> |111>
        let dim = 8;
        let mut expected = vec![C64::ZERO; dim * dim];
        for i in 0..dim {
            expected[i * dim + i] = C64::ONE;
        }
        // |110> = index 6, |111> = index 7 (big-endian)
        expected[6 * dim + 6] = C64::ZERO;
        expected[7 * dim + 7] = C64::ZERO;
        expected[6 * dim + 7] = C64::ONE;
        expected[7 * dim + 6] = C64::ONE;

        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-10),
            "controlled-CX should equal Toffoli"
        );
    }

    // -------------------------------------------------------------------------
    // Barrier passthrough
    // -------------------------------------------------------------------------

    #[test]
    fn test_barrier_passthrough() {
        let ctrl = QWire(0);
        let ops = vec![Op::Barrier(Barrier {
            wires: vec![QWire(1), QWire(2)],
        })];
        let controlled = add_control(ctrl, &ops).unwrap();
        assert_eq!(controlled.len(), 1);
        if let Op::Barrier(b) = &controlled[0] {
            assert!(b.wires.contains(&ctrl));
            assert!(b.wires.contains(&QWire(1)));
            assert!(b.wires.contains(&QWire(2)));
        } else {
            panic!("expected Barrier");
        }
    }

    // -------------------------------------------------------------------------
    // Non-unitary ops must be rejected
    // -------------------------------------------------------------------------

    #[test]
    fn test_prep_rejected() {
        use cqam_core::circuit_ir::Prepare;
        use cqam_core::instruction::DistId;
        let ctrl = QWire(0);
        let ops = vec![Op::Prep(Prepare {
            wires: vec![QWire(1)],
            dist: DistId::Zero,
        })];
        assert!(add_control(ctrl, &ops).is_err());
    }

    #[test]
    fn test_kernel_rejected() {
        use cqam_core::circuit_ir::ApplyKernel;
        use cqam_core::instruction::KernelId;
        use cqam_core::quantum_backend::KernelParams;
        let ctrl = QWire(0);
        let ops = vec![Op::Kernel(ApplyKernel {
            wires: vec![QWire(1)],
            kernel: KernelId::Init,
            params: KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] },
        })];
        assert!(add_control(ctrl, &ops).is_err());
    }

    // -------------------------------------------------------------------------
    // Multi-gate sequence: add_control to [H(a), CX(a,b)] on 3 qubits.
    // When ctrl=|0>, should act as identity on the a,b subspace.
    // When ctrl=|1>, should act as H(a).CX(a,b) on the a,b subspace.
    // -------------------------------------------------------------------------

    #[test]
    fn test_controlled_sequence() {
        // The controlled version lives on 3 qubits: ctrl=QWire(0), a=QWire(1), b=QWire(2).
        let ctrl = QWire(0);
        let a = QWire(1);
        let b = QWire(2);
        // Original circuit: H(a), CX(a, b) on 3-qubit space (even though ctrl is idle).
        let ops = vec![make_h(a), make_cx(a, b)];

        // Reference: unitary of H(a).CX(a,b) computed on the full 3-qubit space (ctrl idle).
        // We compute this directly by letting ctrl=QWire(0) be the MSB (qubit 0) and seeing
        // that the top-left 4x4 block of the 3-qubit unitary should equal the 2-qubit unitary.
        // Build a 2-qubit reference using wire indices 0,1 to avoid out-of-bounds.
        let ref_ops = vec![
            make_h(QWire(0)),
            make_cx(QWire(0), QWire(1)),
        ];
        let ref_u = gate_sequence_unitary(&ref_ops, 2);

        // Controlled version on 3 qubits
        let controlled = add_control(ctrl, &ops).unwrap();
        let ctrl_u = gate_sequence_unitary(&controlled, 3);

        // Verify: top-left 4x4 block (ctrl=|0>) should be I_4.
        // In big-endian 3-qubit ordering, ctrl=|0> means qubit 0 = 0,
        // so indices 0..3 span {|000>,|001>,|010>,|011>}.
        let big = 8usize; // 2^3
        let sub = 4usize; // 2^2
        for i in 0..sub {
            for j in 0..sub {
                let val = ctrl_u[i * big + j];
                let expected = if i == j { C64::ONE } else { C64::ZERO };
                assert!(
                    (val - expected).norm() < 1e-10,
                    "ctrl=0 block [{i},{j}]: got {val:?}, expected {expected:?}"
                );
            }
        }

        // Bottom-right 4x4 block (ctrl=|1>) should be ref_u (up to global phase).
        // Indices 4..7 span {|100>,|101>,|110>,|111>}.
        let mut bottom_right = vec![C64::ZERO; sub * sub];
        for i in 0..sub {
            for j in 0..sub {
                bottom_right[i * sub + j] = ctrl_u[(i + sub) * big + (j + sub)];
            }
        }
        assert!(
            unitaries_equal_up_to_phase(&bottom_right, &ref_u, 1e-10),
            "ctrl=1 block should equal H(a).CX(a,b) unitary"
        );
    }

    // -------------------------------------------------------------------------
    // Toffoli gate count sanity
    // -------------------------------------------------------------------------

    #[test]
    fn test_toffoli_gate_count() {
        let ops = toffoli(QWire(0), QWire(1), QWire(2));
        assert_eq!(ops.len(), 15, "Toffoli should decompose into exactly 15 gates");
    }
}
