//! DiagonalUnitary kernel decomposer.
//!
//! Implements the Walsh-Hadamard Rz+CNOT staircase decomposition for
//! diagonal unitary matrices.

use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::{rz, cx};
use super::params::extract_int_params;

// =============================================================================
// Kernel: DiagonalUnitary
// =============================================================================

/// Decompose a diagonal unitary using the Walsh-Hadamard Rz+CNOT staircase.
///
/// Limited to n <= 4 qubits in Phase 2. Returns an error for larger sizes.
pub fn decompose_diagonal_unitary(wires: &[QWire], params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    let n = wires.len();
    if n > 4 {
        return Err(MicroError::DecompositionFailed {
            kernel: "DiagonalUnitary".to_string(),
            detail: format!("Phase 2 supports n <= 4, got {}", n),
        });
    }
    if n == 0 {
        return Ok(vec![]);
    }

    let (_, _, cmem_data) = extract_int_params(params, "DiagonalUnitary")?;
    let dim = 1usize << n;
    if cmem_data.len() != 2 * dim {
        return Err(MicroError::DecompositionFailed {
            kernel: "DiagonalUnitary".to_string(),
            detail: format!("expected {} cmem entries, got {}", 2 * dim, cmem_data.len()),
        });
    }

    let phases: Vec<f64> = (0..dim).map(|k| {
        let re = f64::from_bits(cmem_data[2 * k] as u64);
        let im = f64::from_bits(cmem_data[2 * k + 1] as u64);
        im.atan2(re)
    }).collect();

    Ok(diagonal_to_gates(wires, &phases))
}

/// Convert a phase vector into an Rz+CNOT gate sequence implementing
/// the diagonal unitary diag(e^{i*phases[0]}, ..., e^{i*phases[2^n-1]}).
///
/// Uses recursive demultiplexing. Returns (ops, global_phase) where
/// global_phase is the common phase factor that was factored out.
///
/// Big-endian convention: qubit 0 (wires[0]) is MSB.
pub(super) fn diagonal_to_gates(wires: &[QWire], phases: &[f64]) -> Vec<Op> {
    let (ops, _global_phase) = diagonal_to_gates_inner(wires, phases);
    ops
}

/// Inner recursive function that returns (ops, global_phase).
/// The global phase is the average of all input phases, which is factored
/// out and must be applied by the caller (via Rz on the control qubit).
fn diagonal_to_gates_inner(wires: &[QWire], phases: &[f64]) -> (Vec<Op>, f64) {
    let n = wires.len();
    if n == 0 {
        return (vec![], 0.0);
    }

    let dim = 1usize << n;
    let global_phase: f64 = phases.iter().sum::<f64>() / dim as f64;

    // Base case: single qubit.
    // Rz(theta) = diag(e^{-i*theta/2}, e^{i*theta/2}).
    // theta = phases[1] - phases[0].
    // Global phase = (phases[0] + phases[1]) / 2.
    if n == 1 {
        let theta = phases[1] - phases[0];
        let mut ops = Vec::new();
        if theta.abs() > 1e-15 {
            ops.push(rz(wires[0], theta));
        }
        return (ops, global_phase);
    }

    // Recursive case: n qubits.
    // Split phases into upper half (q0=0) and lower half (q0=1).
    let half = 1 << (n - 1);
    let phi_upper = &phases[..half];    // q0 = 0
    let phi_lower = &phases[half..];    // q0 = 1

    // Compute sum (common) and difference (conditional) phase vectors.
    let sum_phases: Vec<f64> = (0..half).map(|k| (phi_upper[k] + phi_lower[k]) / 2.0).collect();
    let diff_phases: Vec<f64> = (0..half).map(|k| (phi_lower[k] - phi_upper[k]) / 2.0).collect();

    let sub_wires = &wires[1..]; // qubits 1..n-1

    let mut ops = Vec::new();

    // 1. Recursively decompose the sum phases on sub-qubits.
    let (sum_ops, _sum_global) = diagonal_to_gates_inner(sub_wires, &sum_phases);
    ops.extend(sum_ops);

    // 2. CX(q0, q_{n-1}) to entangle with the control qubit.
    ops.push(cx(wires[0], wires[n - 1]));

    // 3. Recursively decompose the diff phases on sub-qubits.
    //    The CX flips the last sub-qubit when q0=1, so the diff circuit
    //    sees state |k XOR 1> instead of |k>. To compensate, we reorder
    //    the diff phases: diff_reordered[k] = diff[k XOR 1].
    let reordered_diff: Vec<f64> = (0..half).map(|k| diff_phases[k ^ 1]).collect();
    let (diff_ops, diff_global) = diagonal_to_gates_inner(sub_wires, &reordered_diff);
    ops.extend(diff_ops);

    // 4. Apply the global phase of the diff decomposition on q0.
    //    This global phase is not truly global -- it's the average of the
    //    diff phases, which only applies when q0=1 (via the CX sandwich).
    //    Rz(2*diff_global) on q0 adds +diff_global when q0=1 and
    //    -diff_global when q0=0.
    //    But we want: +diff_global when q0=1, 0 when q0=0.
    //    So we use Rz(2*diff_global) on q0, which gives us the desired
    //    conditional phase (the -diff_global on q0=0 becomes part of
    //    *our* global phase that the parent handles).
    if diff_global.abs() > 1e-15 {
        ops.push(rz(wires[0], 2.0 * diff_global));
    }

    // 5. CX(q0, q_{n-1}) to undo the entanglement.
    ops.push(cx(wires[0], wires[n - 1]));

    (ops, global_phase)
}
