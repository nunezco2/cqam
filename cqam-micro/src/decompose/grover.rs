//! Grover diffusion operator and GroverIter kernel decomposers.

use std::f64::consts::PI;
use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::{h, x, rz, cx, cz};
use super::diagonal::diagonal_to_gates;

// =============================================================================
// Kernel: Diffuse
// =============================================================================

/// Decompose a Toffoli (CCX) gate into the standard 6-CNOT form.
/// CCX(c0, c1, target) = 15 gates.
fn toffoli(c0: QWire, c1: QWire, target: QWire) -> Vec<Op> {
    use super::helpers::{t_gate, tdg};
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

/// Decompose a multi-controlled-Z gate on the given wires.
/// MCZ flips the phase of |1...1>.
pub(super) fn decompose_mcz(wires: &[QWire]) -> Vec<Op> {
    let n = wires.len();
    match n {
        0 => vec![],
        1 => {
            // Z gate on single qubit = phase flip on |1>
            vec![rz(wires[0], PI)]
        }
        2 => {
            // CZ gate
            vec![cz(wires[0], wires[1])]
        }
        3 => {
            // MCZ = H(target) . Toffoli(c0, c1, target) . H(target)
            let mut ops = vec![h(wires[2])];
            ops.extend(toffoli(wires[0], wires[1], wires[2]));
            ops.push(h(wires[2]));
            ops
        }
        _ => {
            // For n >= 4, use recursive decomposition via diagonal gates.
            // An n-controlled-Z = H(last) . n-controlled-X . H(last)
            // We use the diagonal decomposition for MCZ on m qubits.
            let mut ops = vec![h(wires[n - 1])];
            ops.extend(decompose_multi_cx(&wires[..n-1], wires[n - 1]));
            ops.push(h(wires[n - 1]));
            ops
        }
    }
}

/// Decompose a multi-controlled-X gate: controls = controls_wires, target = target.
/// For n controls:
/// - n=1: CX(ctrl, target)
/// - n=2: Toffoli(c0, c1, target)
/// - n>=3: Recursive decomposition using dirty ancilla approach.
pub(super) fn decompose_multi_cx(controls: &[QWire], target: QWire) -> Vec<Op> {
    let n = controls.len();
    match n {
        0 => vec![x(target)],
        1 => vec![cx(controls[0], target)],
        2 => toffoli(controls[0], controls[1], target),
        _ => {
            // MCX = H(tgt) . MCZ(controls + [tgt]) . H(tgt)
            // MCZ is implemented via the Gray-code diagonal decomposition:
            //   diagonal phases all-zero except phases[dim-1] = pi applies
            //   e^{i*pi}|1...1> = -|1...1>, which is the multi-controlled-Z.
            //
            // BUG FIX: the H gate wrapping was previously missing, causing
            // this to implement MCZ instead of MCX for n >= 3 controls.
            let mut ops = Vec::new();
            let all_wires: Vec<QWire> = controls.iter().copied().chain(std::iter::once(target)).collect();
            let m = all_wires.len();
            let dim = 1usize << m;

            // Build phase vector: all zeros except pi at index dim-1 (|1...1>)
            let mut phases = vec![0.0f64; dim];
            phases[dim - 1] = PI; // exp(i*pi) = -1, the MCZ phase kick

            // H(target) . MCZ . H(target) = MCX
            ops.push(h(target));
            ops.extend(diagonal_to_gates(&all_wires, &phases));
            ops.push(h(target));
            ops
        }
    }
}

/// Decompose the Diffuse (Grover diffusion) kernel.
///
/// D = H^n . X^n . MCZ . X^n . H^n
pub fn decompose_diffuse(wires: &[QWire], _params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    let n = wires.len();
    if n == 0 {
        return Ok(vec![]);
    }

    let mut ops = Vec::new();

    // Step 1: H on all wires
    for &w in wires {
        ops.push(h(w));
    }

    // Step 2: X on all wires
    for &w in wires {
        ops.push(x(w));
    }

    // Step 3: MCZ
    ops.extend(decompose_mcz(wires));

    // Step 4: X on all wires
    for &w in wires {
        ops.push(x(w));
    }

    // Step 5: H on all wires
    for &w in wires {
        ops.push(h(w));
    }

    Ok(ops)
}

// =============================================================================
// Kernel: GroverIter
// =============================================================================

/// Decompose the GroverIter kernel: Oracle + Diffusion.
pub fn decompose_grover(wires: &[QWire], params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    let n = wires.len();
    if n < 1 {
        return Err(MicroError::DecompositionFailed {
            kernel: "GroverIter".to_string(),
            detail: "requires >= 1 wire".to_string(),
        });
    }

    let dim = 1usize << n;

    // Extract targets
    let targets: Vec<usize> = match params {
        KernelParams::Int { param0, param1: _, cmem_data } => {
            if cmem_data.is_empty() {
                vec![*param0 as usize]
            } else {
                cmem_data.iter().map(|&v| v as usize).collect()
            }
        }
        _ => {
            return Err(MicroError::DecompositionFailed {
                kernel: "GroverIter".to_string(),
                detail: "expected Int params".to_string(),
            });
        }
    };

    if targets.is_empty() {
        return Err(MicroError::DecompositionFailed {
            kernel: "GroverIter".to_string(),
            detail: "no targets specified".to_string(),
        });
    }

    for &t in &targets {
        if t >= dim {
            return Err(MicroError::DecompositionFailed {
                kernel: "GroverIter".to_string(),
                detail: format!("target {} >= dimension {}", t, dim),
            });
        }
    }

    let mut ops = Vec::new();

    // Oracle phase: for each target, flip its sign using X + MCZ + X
    for &target in &targets {
        // Apply X to each qubit where the target bit is 0
        for (i, &wire) in wires.iter().enumerate() {
            let bit_pos = n - 1 - i; // big-endian: qubit i corresponds to bit n-1-i
            if (target >> bit_pos) & 1 == 0 {
                ops.push(x(wire));
            }
        }

        // MCZ on all wires
        ops.extend(decompose_mcz(wires));

        // Undo X gates
        for (i, &wire) in wires.iter().enumerate() {
            let bit_pos = n - 1 - i;
            if (target >> bit_pos) & 1 == 0 {
                ops.push(x(wire));
            }
        }
    }

    // Diffusion phase
    ops.extend(decompose_diffuse(wires, params)?);

    Ok(ops)
}
