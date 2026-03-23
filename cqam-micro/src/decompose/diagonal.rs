//! DiagonalUnitary kernel decomposer.
//!
//! Implements the Walsh-Hadamard Rz+CNOT synthesis for diagonal unitary matrices.

use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::{rz, cx};
use super::params::extract_int_params;

// =============================================================================
// Kernel: DiagonalUnitary
// =============================================================================

/// Decompose a diagonal unitary using the Walsh-Hadamard Rz+CNOT synthesis.
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
/// Uses the Walsh-Hadamard synthesis. This is exact up to a global phase.
///
/// Convention: wires[0] is the MSB qubit. phases[k] is indexed where
/// the MSB of k corresponds to wires[0] and LSB to wires[n-1].
///
/// The implementation may introduce an overall global phase (uniform
/// phase on all basis states), which is physically unobservable.
pub(super) fn diagonal_to_gates(wires: &[QWire], phases: &[f64]) -> Vec<Op> {
    let n = wires.len();
    if n == 0 || phases.is_empty() {
        return vec![];
    }
    let dim = 1usize << n;
    assert_eq!(phases.len(), dim, "phases length must equal 2^n");

    // Compute Walsh-Hadamard coefficients alpha[k] such that:
    //   phases[j] = sum_k alpha[k] * (-1)^{popcount(j & k)}
    //
    // Each alpha[k] (for k >= 1) corresponds to the operator
    //   exp(i * alpha[k] * Z_{b0} otimes ... otimes Z_{bm-1})
    // where {b0,...,bm-1} are the bits set in k.
    //
    // Implementation: for each k with bits {b0,...,bm-1}, pick the
    // lowest bit as "target" and the rest as "sources". Apply CX from
    // each source to the target (this XORs the parity of all set bits
    // onto the target), apply Rz(-2*alpha[k]) on the target, then undo
    // the CX gates. This is self-inverse, so the undo is the same set
    // of CX gates in the same order.
    //
    // Rz convention: Rz(t) = diag(e^{-it/2}, e^{+it/2}).
    // For a target qubit holding parity p = parity(j & k) in {0,1}:
    //   p=0 → phase e^{-i*(-alpha)} = e^{+i*alpha} = exp(i*alpha*(-1)^0). ✓
    //   p=1 → phase e^{+i*(-alpha)} = e^{-i*alpha} = exp(i*alpha*(-1)^1). ✓
    // So Rz(-2*alpha) correctly implements exp(i*alpha*(-1)^{parity}).
    let alpha = wht_coefficients(phases);
    direct_parity_synthesis(wires, &alpha)
}

/// Compute Walsh-Hadamard coefficients.
/// alpha[k] = (1/dim) * sum_j phases[j] * (-1)^{popcount(j & k)}
fn wht_coefficients(phases: &[f64]) -> Vec<f64> {
    let dim = phases.len();
    let mut alpha = phases.to_vec();
    // In-place fast Walsh-Hadamard transform (butterfly).
    let mut h = 1;
    while h < dim {
        for i in (0..dim).step_by(h * 2) {
            for j in i..(i + h) {
                let x = alpha[j];
                let y = alpha[j + h];
                alpha[j] = x + y;
                alpha[j + h] = x - y;
            }
        }
        h *= 2;
    }
    // Normalize.
    let inv_dim = 1.0 / dim as f64;
    for a in alpha.iter_mut() {
        *a *= inv_dim;
    }
    alpha
}

/// Direct parity synthesis of diagonal unitary from WHT coefficients.
///
/// For each k from 1..dim with nonzero alpha[k]:
///   1. Let target_bit = lowest set bit of k, sources = all other set bits of k.
///      Big-endian mapping: bit b → wires[n-1-b].
///   2. Apply CX(source → target) for each source bit (computes parity onto target).
///   3. Apply Rz(-2*alpha[k]) on target_wire.
///   4. Apply the same CX gates again to undo (CX is self-inverse).
///
/// This is O(n * 2^n) CX gates but is guaranteed correct: the target qubit
/// always holds the exact parity of the bits in k, regardless of qubit state
/// modifications from prior steps, because we explicitly compute it fresh.
fn direct_parity_synthesis(wires: &[QWire], alpha: &[f64]) -> Vec<Op> {
    let n = wires.len();
    let dim = 1usize << n;
    let mut ops = Vec::new();

    for k in 1..dim {
        let angle = -2.0 * alpha[k];
        if angle.abs() <= 1e-15 {
            continue;
        }

        // Collect all bit positions set in k (LSB=0).
        let bits: Vec<usize> = (0..n).filter(|&b| (k >> b) & 1 == 1).collect();

        // Target = lowest set bit of k; sources = the rest.
        let target_bit = bits[0];
        let source_bits = &bits[1..];

        // Big-endian: bit b → wires[n-1-b]
        let target_wire = wires[n - 1 - target_bit];

        // Build CX(source → target) for each source bit.
        let cx_ops: Vec<Op> = source_bits.iter()
            .map(|&sb| cx(wires[n - 1 - sb], target_wire))
            .collect();

        // Apply CX gates to compute parity, then Rz, then CX again to undo.
        ops.extend_from_slice(&cx_ops);
        ops.push(rz(target_wire, angle));
        ops.extend_from_slice(&cx_ops);
    }

    ops
}
