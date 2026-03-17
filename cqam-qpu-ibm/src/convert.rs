//! Convert a `cqam_core::native_ir::Circuit` into a `SafeQkCircuit`.
//!
//! Gate mapping (CQAM → Qiskit C API):
//!
//! | CQAM gate            | QkGate constant  | Value |
//! |----------------------|-----------------|-------|
//! | `NativeGate1::Sx`    | `QK_GATE_SX`    | 13    |
//! | `NativeGate1::X`     | `QK_GATE_X`     |  3    |
//! | `NativeGate1::Rz(θ)` | `QK_GATE_RZ`    | 10    |
//! | `NativeGate1::Id`    | `QK_GATE_I`     |  2    |
//! | `NativeGate2::Cx`    | `QK_GATE_CX`    | 22    |

use cqam_core::native_ir::{NativeGate1, NativeGate2, Op};

use crate::error::{check_exit_code, IbmError};
use crate::ffi;
use crate::safe::SafeQkCircuit;

/// Convert a `native_ir::Circuit` into an owned `SafeQkCircuit`.
///
/// The resulting circuit uses `circuit.num_physical_qubits` qubits and
/// one classical bit per `Observe` operation (up to `num_physical_qubits`).
pub fn native_to_qk(circuit: &cqam_core::native_ir::Circuit) -> Result<SafeQkCircuit, IbmError> {
    let num_q = circuit.num_physical_qubits;
    // Allocate one clbit per qubit (worst-case; unused clbits are harmless).
    let mut qk_circ = SafeQkCircuit::new(num_q, num_q)
        .ok_or(IbmError::NullPointer { context: "qk_circuit_new" })?;

    for op in &circuit.ops {
        match op {
            Op::Gate1q(g1) => {
                apply_gate1q(&mut qk_circ, &g1.gate, g1.qubit.0)?;
            }
            Op::Gate2q(g2) => {
                apply_gate2q(&mut qk_circ, &g2.gate, g2.qubit_a.0, g2.qubit_b.0)?;
            }
            Op::Measure(obs) => {
                let code = unsafe {
                    ffi::qk_circuit_measure(
                        qk_circ.as_mut_ptr(),
                        obs.qubit.0,
                        obs.clbit,
                    )
                };
                check_exit_code(code, "qk_circuit_measure")?;
            }
            Op::Reset(r) => {
                let code = unsafe {
                    ffi::qk_circuit_reset(qk_circ.as_mut_ptr(), r.qubit.0)
                };
                check_exit_code(code, "qk_circuit_reset")?;
            }
            Op::Barrier(b) => {
                let indices: Vec<u32> = b.qubits.iter().map(|q| q.0).collect();
                let code = unsafe {
                    ffi::qk_circuit_barrier(
                        qk_circ.as_mut_ptr(),
                        indices.as_ptr(),
                        indices.len() as u32,
                    )
                };
                check_exit_code(code, "qk_circuit_barrier")?;
            }
        }
    }

    Ok(qk_circ)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn apply_gate1q(
    circ: &mut SafeQkCircuit,
    gate: &NativeGate1,
    qubit: u32,
) -> Result<(), IbmError> {
    let qubits = [qubit];
    match gate {
        NativeGate1::Sx => {
            let code = unsafe {
                ffi::qk_circuit_gate(
                    circ.as_mut_ptr(),
                    ffi::QK_GATE_SX,
                    qubits.as_ptr(),
                    std::ptr::null(),
                )
            };
            check_exit_code(code, "qk_circuit_gate(SX)")
        }
        NativeGate1::X => {
            let code = unsafe {
                ffi::qk_circuit_gate(
                    circ.as_mut_ptr(),
                    ffi::QK_GATE_X,
                    qubits.as_ptr(),
                    std::ptr::null(),
                )
            };
            check_exit_code(code, "qk_circuit_gate(X)")
        }
        NativeGate1::Rz(theta) => {
            let params = [*theta];
            let code = unsafe {
                ffi::qk_circuit_gate(
                    circ.as_mut_ptr(),
                    ffi::QK_GATE_RZ,
                    qubits.as_ptr(),
                    params.as_ptr(),
                )
            };
            check_exit_code(code, "qk_circuit_gate(RZ)")
        }
        NativeGate1::Id => {
            let code = unsafe {
                ffi::qk_circuit_gate(
                    circ.as_mut_ptr(),
                    ffi::QK_GATE_I,
                    qubits.as_ptr(),
                    std::ptr::null(),
                )
            };
            check_exit_code(code, "qk_circuit_gate(I)")
        }
    }
}

fn apply_gate2q(
    circ: &mut SafeQkCircuit,
    gate: &NativeGate2,
    qubit_a: u32,
    qubit_b: u32,
) -> Result<(), IbmError> {
    let qubits = [qubit_a, qubit_b];
    match gate {
        NativeGate2::Cx => {
            let code = unsafe {
                ffi::qk_circuit_gate(
                    circ.as_mut_ptr(),
                    ffi::QK_GATE_CX,
                    qubits.as_ptr(),
                    std::ptr::null(),
                )
            };
            check_exit_code(code, "qk_circuit_gate(CX)")
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::native_ir::{
        ApplyGate1q, ApplyGate2q, Circuit, NativeGate1, NativeGate2, Observe, Op, PhysicalQubit,
    };

    fn bell_circuit() -> Circuit {
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Sx,
        }));
        c.ops.push(Op::Gate2q(ApplyGate2q {
            qubit_a: PhysicalQubit(0),
            qubit_b: PhysicalQubit(1),
            gate: NativeGate2::Cx,
        }));
        c.ops.push(Op::Measure(Observe {
            qubit: PhysicalQubit(0),
            clbit: 0,
        }));
        c.ops.push(Op::Measure(Observe {
            qubit: PhysicalQubit(1),
            clbit: 1,
        }));
        c
    }

    #[test]
    fn test_empty_circuit_converts() {
        let c = Circuit::new(3);
        let qk = native_to_qk(&c);
        assert!(qk.is_ok(), "empty circuit should convert: {:?}", qk.err());
        let qk = qk.unwrap();
        assert_eq!(qk.num_qubits(), 3);
    }

    #[test]
    fn test_bell_circuit_converts() {
        let c = bell_circuit();
        let qk = native_to_qk(&c);
        assert!(qk.is_ok(), "bell circuit should convert: {:?}", qk.err());
    }

    #[test]
    fn test_rz_gate_converts() {
        let mut c = Circuit::new(1);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Rz(std::f64::consts::PI / 2.0),
        }));
        assert!(native_to_qk(&c).is_ok());
    }

    #[test]
    fn test_identity_gate_converts() {
        let mut c = Circuit::new(1);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Id,
        }));
        assert!(native_to_qk(&c).is_ok());
    }
}
