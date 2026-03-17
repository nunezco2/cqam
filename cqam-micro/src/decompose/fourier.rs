//! Fourier (QFT) and FourierInv (IQFT) kernel decomposers.

use std::f64::consts::PI;
use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::{h, swap, cx, rz};

/// Decompose the controlled-phase gate CP(theta) into 5 standard gates:
/// Rz(t/2) on target, CX(ctrl, tgt), Rz(-t/2) on target,
/// CX(ctrl, tgt), Rz(t/2) on control.
fn controlled_phase(ctrl: QWire, tgt: QWire, theta: f64) -> Vec<Op> {
    vec![
        rz(tgt, theta / 2.0),
        cx(ctrl, tgt),
        rz(tgt, -theta / 2.0),
        cx(ctrl, tgt),
        rz(ctrl, theta / 2.0),
    ]
}

/// Decompose the QFT kernel.
pub fn decompose_fourier(wires: &[QWire], _params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    let n = wires.len();
    let mut ops = Vec::new();

    for j in 0..n {
        ops.push(h(wires[j]));
        for k in (j + 1)..n {
            let m = k - j;
            let theta = 2.0 * PI / (1u64 << m) as f64;
            ops.extend(controlled_phase(wires[k], wires[j], theta));
        }
    }

    // Bit-reversal SWAPs
    for i in 0..(n / 2) {
        ops.push(swap(wires[i], wires[n - 1 - i]));
    }

    Ok(ops)
}

/// Decompose the inverse QFT kernel.
pub fn decompose_fourier_inv(wires: &[QWire], _params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    let n = wires.len();
    let mut ops = Vec::new();

    // Bit-reversal first
    for i in 0..(n / 2) {
        ops.push(swap(wires[i], wires[n - 1 - i]));
    }

    // Reverse QFT gates
    for j in (0..n).rev() {
        for k in ((j + 1)..n).rev() {
            let m = k - j;
            let theta = -2.0 * PI / (1u64 << m) as f64;
            ops.extend(controlled_phase(wires[k], wires[j], theta));
        }
        ops.push(h(wires[j]));
    }

    Ok(ops)
}
