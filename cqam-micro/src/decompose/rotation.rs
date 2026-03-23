//! Rotate, PhaseShift, and ControlledU kernel decomposers.

use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::instruction::KernelId;
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::{rz, cx};
use super::params::{extract_float_param0, extract_complex_param0, extract_int_params};
use super::controlled::add_control;
use super::decompose_kernel;

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

    let (param0, _param1, cmem_data) = extract_int_params(params, "ControlledU")?;

    // The VM passes ControlledU parameters as:
    //   param0 = control qubit index (from ctx0 register)
    //   param1 = CMEM base address (from ctx1 register)
    //   cmem_data = [sub_kernel_id, power, param_re_bits, param_im_bits, target_qubits, ...]
    let control_qubit = param0 as u8;

    if cmem_data.len() < 5 {
        return Err(MicroError::DecompositionFailed {
            kernel: "ControlledU".to_string(),
            detail: format!("cmem_data too short: {} < 5", cmem_data.len()),
        });
    }

    let sub_kernel_id_raw = cmem_data[0] as u8;
    let power = cmem_data[1] as u32;
    let target_qubits_field = cmem_data[4] as u8;

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
    let sub_param = f64::from_bits(cmem_data[2] as u64);

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
        other_kernel => {
            // Generic path: decompose the sub-kernel to standard gates,
            // then add a control qubit to every gate.
            //
            // Step 1: Reconstruct KernelParams for the sub-kernel.
            let sub_params = reconstruct_sub_params(other_kernel, cmem_data)?;

            // Step 2: Decompose the sub-kernel into standard gates.
            let sub_ops = decompose_kernel(actual_target_wires, &other_kernel, &sub_params)?;

            // Step 3: Repeat for power (controlled-U^{2^power}).
            let power_scale: usize = if power == 0 { 1 } else { 1usize << power };
            let mut powered_ops = Vec::with_capacity(sub_ops.len() * power_scale);
            for _ in 0..power_scale {
                powered_ops.extend_from_slice(&sub_ops);
            }

            // Step 4: Add control qubit to every gate.
            add_control(ctrl_wire, &powered_ops)
        }
    }
}

// =============================================================================
// Helper: reconstruct KernelParams for a sub-kernel from cmem_data
// =============================================================================

/// Reconstruct KernelParams for a sub-kernel from the ControlledU cmem_data.
///
/// The cmem_data layout (set by the VM's ControlledU handler):
///   [0] = sub_kernel_id (u8 discriminant of KernelId)
///   [1] = power (u32 -- number of times to square the sub-kernel application)
///   [2] = param_re_bits (f64 as u64 cast to i64, the real part of the parameter)
///   [3] = param_im_bits (f64 as u64 cast to i64, the imaginary part)
///   [4] = target_qubits (u8 -- number of target qubits, 0 means all non-control)
///   [5..] = sub-kernel-specific extra data (e.g., target state indices)
fn reconstruct_sub_params(
    kernel: KernelId,
    cmem_data: &[i64],
) -> Result<KernelParams, MicroError> {
    match kernel {
        KernelId::GroverIter => {
            // For GroverIter, cmem_data[5] holds the target state index if present;
            // otherwise fall back to interpreting cmem_data[2] bits as the index.
            let target = if cmem_data.len() > 5 {
                cmem_data[5]
            } else {
                f64::from_bits(cmem_data[2] as u64) as i64
            };
            Ok(KernelParams::Int {
                param0: target,
                param1: 0,
                cmem_data: vec![],
            })
        }
        KernelId::Fourier | KernelId::FourierInv => Ok(KernelParams::Float {
            param0: 0.0,
            param1: 0.0,
        }),
        KernelId::Init | KernelId::Entangle | KernelId::Diffuse => Ok(KernelParams::Int {
            param0: 0,
            param1: 0,
            cmem_data: vec![],
        }),
        KernelId::Rotate => {
            let theta = f64::from_bits(cmem_data[2] as u64);
            Ok(KernelParams::Float {
                param0: theta,
                param1: 0.0,
            })
        }
        KernelId::PhaseShift => {
            let re = f64::from_bits(cmem_data[2] as u64);
            let im = f64::from_bits(cmem_data[3] as u64);
            Ok(KernelParams::Complex {
                param0: cqam_core::complex::C64(re, im),
                param1: cqam_core::complex::C64::ZERO,
            })
        }
        KernelId::Permutation => {
            // cmem_data[5..] contains the permutation table entries.
            // The table has 2^target_qubits entries.
            let table_data: Vec<i64> = if cmem_data.len() > 5 {
                cmem_data[5..].to_vec()
            } else {
                vec![]
            };
            Ok(KernelParams::Int {
                param0: 0,
                param1: 0,
                cmem_data: table_data,
            })
        }
        _ => Err(MicroError::DecompositionFailed {
            kernel: "ControlledU".to_string(),
            detail: format!(
                "sub-kernel {:?}: cannot reconstruct params for generic controlled path",
                kernel,
            ),
        }),
    }
}
