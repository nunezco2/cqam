//! Rotate, PhaseShift, and ControlledU kernel decomposers.

use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::instruction::KernelId;
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::{rz, cx};
use super::params::{extract_float_param0, extract_complex_param0, extract_int_params};

// =============================================================================
// Kernel: Rotate
// =============================================================================

/// Decompose the Rotate kernel: Rz(theta * 2^{n-1-j}) on each qubit j.
pub fn decompose_rotate(wires: &[QWire], params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    let theta = extract_float_param0(params, "Rotate")?;
    let n = wires.len();
    let ops = wires.iter().enumerate()
        .map(|(j, &w)| rz(w, theta * (1u64 << (n - 1 - j)) as f64))
        .collect();
    Ok(ops)
}

// =============================================================================
// Kernel: PhaseShift
// =============================================================================

/// Decompose the PhaseShift kernel: same as Rotate with theta = |amplitude|.
pub fn decompose_phase_shift(wires: &[QWire], params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    let amplitude = extract_complex_param0(params, "PhaseShift")?;
    let theta = amplitude.norm();
    let n = wires.len();
    let ops = wires.iter().enumerate()
        .map(|(j, &w)| rz(w, theta * (1u64 << (n - 1 - j)) as f64))
        .collect();
    Ok(ops)
}

// =============================================================================
// Kernel: ControlledU
// =============================================================================

/// Decompose the ControlledU kernel.
///
/// Phase 2 MVP supports controlled-Rotate and controlled-PhaseShift
/// (the common sub-kernels for QPE). Other sub-kernels return an error.
pub fn decompose_controlled_u(wires: &[QWire], params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    if wires.len() < 2 {
        return Err(MicroError::DecompositionFailed {
            kernel: "ControlledU".to_string(),
            detail: format!("requires >= 2 wires, got {}", wires.len()),
        });
    }

    let (param0, param1, _cmem_data) = extract_int_params(params, "ControlledU")?;

    // Decode packed parameters from param0:
    // param0 = (control_qubit << 24) | (sub_kernel_id << 16) | (target_qubits << 8) | power
    let control_qubit = ((param0 >> 24) & 0xFF) as u8;
    let sub_kernel_id_raw = ((param0 >> 16) & 0xFF) as u8;
    let target_qubits_field = ((param0 >> 8) & 0xFF) as u8;
    let power = (param0 & 0xFF) as u32;

    let sub_kernel_id = KernelId::try_from(sub_kernel_id_raw).map_err(|_| {
        MicroError::DecompositionFailed {
            kernel: "ControlledU".to_string(),
            detail: format!("invalid sub-kernel ID {}", sub_kernel_id_raw),
        }
    })?;

    let n = wires.len();
    if (control_qubit as usize) >= n {
        return Err(MicroError::DecompositionFailed {
            kernel: "ControlledU".to_string(),
            detail: format!("control_qubit {} out of range for {} wires", control_qubit, n),
        });
    }

    let ctrl_wire = wires[control_qubit as usize];
    let effective_target_qubits = if target_qubits_field == 0 {
        (n - 1) as u8
    } else {
        target_qubits_field
    };

    // Target wires are the last `effective_target_qubits` wires, excluding control.
    let target_wires: Vec<QWire> = wires.iter()
        .enumerate()
        .filter(|&(i, _)| i != control_qubit as usize)
        .map(|(_, &w)| w)
        .collect();

    // Only take the last effective_target_qubits wires from the non-control wires
    let t = effective_target_qubits as usize;
    let actual_target_wires = if t >= target_wires.len() {
        &target_wires[..]
    } else {
        &target_wires[target_wires.len() - t..]
    };

    // Compute effective angle with power folding
    let scale = if power == 0 { 1.0 } else { (1u64 << power) as f64 };
    let sub_param = f64::from_bits(param1 as u64);

    match sub_kernel_id {
        KernelId::Rotate => {
            // Controlled-Rotate: each qubit j gets controlled-Rz(theta * 2^{t-1-j})
            let theta = sub_param * scale;
            let t_n = actual_target_wires.len();
            let mut ops = Vec::new();
            for (j, &tgt_wire) in actual_target_wires.iter().enumerate() {
                let angle = theta * (1u64 << (t_n - 1 - j)) as f64;
                // Controlled-Rz(angle) decomposition:
                // Rz(angle/2, target), CX(ctrl, target), Rz(-angle/2, target), CX(ctrl, target)
                // Plus Rz(angle/2, control) for exact phase.
                ops.push(rz(tgt_wire, angle / 2.0));
                ops.push(cx(ctrl_wire, tgt_wire));
                ops.push(rz(tgt_wire, -angle / 2.0));
                ops.push(cx(ctrl_wire, tgt_wire));
                ops.push(rz(ctrl_wire, angle / 2.0));
            }
            Ok(ops)
        }
        KernelId::PhaseShift => {
            // Same as controlled-Rotate but theta = |amplitude|
            // param1 encodes the real part, but for PhaseShift we need the norm.
            // Since the VM passes param_re as the real part via f64::to_bits,
            // and param_im isn't available through this encoding, use |param_re|
            // as approximation for the norm. This matches the common case where
            // the imaginary part is 0.
            let theta = sub_param.abs() * scale;
            let t_n = actual_target_wires.len();
            let mut ops = Vec::new();
            for (j, &tgt_wire) in actual_target_wires.iter().enumerate() {
                let angle = theta * (1u64 << (t_n - 1 - j)) as f64;
                ops.push(rz(tgt_wire, angle / 2.0));
                ops.push(cx(ctrl_wire, tgt_wire));
                ops.push(rz(tgt_wire, -angle / 2.0));
                ops.push(cx(ctrl_wire, tgt_wire));
                ops.push(rz(ctrl_wire, angle / 2.0));
            }
            Ok(ops)
        }
        _ => {
            Err(MicroError::DecompositionFailed {
                kernel: "ControlledU".to_string(),
                detail: format!(
                    "sub-kernel {:?} not supported in Phase 2 (only Rotate and PhaseShift)",
                    sub_kernel_id,
                ),
            })
        }
    }
}
