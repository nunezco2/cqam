//! Permutation kernel decomposer.
//!
//! Decomposes an arbitrary permutation of basis states into a sequence of
//! standard gates via cycle decomposition into transpositions.

use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::x;
use super::params::extract_int_params;
use super::grover::decompose_multi_cx;

// =============================================================================
// Kernel: Permutation
// =============================================================================

/// Decompose a permutation kernel into gates.
///
/// Supports up to n <= 10 qubits. Uses cycle decomposition into
/// transpositions, then decomposes each transposition.
pub fn decompose_permutation(wires: &[QWire], params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    let n = wires.len();
    if n > 10 {
        return Err(MicroError::DecompositionFailed {
            kernel: "Permutation".to_string(),
            detail: format!("Permutation decomposition supports n <= 10, got {}", n),
        });
    }
    if n == 0 {
        return Ok(vec![]);
    }

    let (_, _, cmem_data) = extract_int_params(params, "Permutation")?;
    let dim = 1usize << n;
    if cmem_data.len() != dim {
        return Err(MicroError::DecompositionFailed {
            kernel: "Permutation".to_string(),
            detail: format!("expected {} entries, got {}", dim, cmem_data.len()),
        });
    }

    let table: Vec<usize> = cmem_data.iter().map(|&v| v as usize).collect();

    // Check if identity
    if table.iter().enumerate().all(|(i, &v)| v == i) {
        return Ok(vec![]);
    }

    // Special case: n=1
    if n == 1 {
        if table == [1, 0] {
            return Ok(vec![x(wires[0])]);
        }
        return Ok(vec![]);
    }

    // Decompose permutation into transpositions (from cycle decomposition)
    let transpositions = permutation_to_transpositions(&table);

    let mut ops = Vec::new();
    for (a, b) in transpositions {
        ops.extend(decompose_transposition(wires, n, a, b));
    }
    Ok(ops)
}

/// Decompose a permutation table into a list of transpositions.
fn permutation_to_transpositions(table: &[usize]) -> Vec<(usize, usize)> {
    let dim = table.len();
    let mut visited = vec![false; dim];
    let mut transpositions = Vec::new();

    for start in 0..dim {
        if visited[start] || table[start] == start {
            visited[start] = true;
            continue;
        }
        // Follow the cycle
        let mut cycle = Vec::new();
        let mut current = start;
        while !visited[current] {
            visited[current] = true;
            cycle.push(current);
            current = table[current];
        }
        // Decompose cycle into transpositions: (a0,a1)(a0,a2)...(a0,a_{k-1})
        for i in 1..cycle.len() {
            transpositions.push((cycle[0], cycle[i]));
        }
    }

    transpositions
}

/// Decompose a transposition of basis states |a> <-> |b> into gates.
///
/// Uses the approach: find differing bits, chain single-bit transpositions.
fn decompose_transposition(wires: &[QWire], n: usize, a: usize, b: usize) -> Vec<Op> {
    if a == b {
        return vec![];
    }
    let diff = a ^ b;
    let diff_bits: Vec<usize> = (0..n).filter(|&i| (diff >> i) & 1 == 1).collect();

    if diff_bits.len() == 1 {
        // Single-bit difference: multi-controlled-X
        return decompose_single_bit_transposition(wires, n, a, b, diff_bits[0]);
    }

    // Multi-bit difference: chain through intermediates.
    // Swap (a, b) where they differ in multiple bits.
    // Use (a, c)(c, b)(a, c) where c differs from a in only the lowest diff bit.
    let lowest_bit = diff_bits[0];
    let c = a ^ (1 << lowest_bit);

    let mut ops = Vec::new();
    ops.extend(decompose_transposition(wires, n, a, c));
    ops.extend(decompose_transposition(wires, n, c, b));
    ops.extend(decompose_transposition(wires, n, a, c));
    ops
}

/// Decompose a transposition where a and b differ in exactly one bit.
/// This is a multi-controlled-X gate on the differing bit, controlled by
/// the common bits.
fn decompose_single_bit_transposition(
    wires: &[QWire],
    n: usize,
    a: usize,
    _b: usize,
    target_bit: usize,
) -> Vec<Op> {
    // The target qubit is at bit position target_bit.
    // Controls: all other bits must match the common value (from a or b, they are the same
    // on non-target bits).
    let target_wire = n - 1 - target_bit; // big-endian mapping

    // Determine control conditions: for each non-target bit, if bit is 1 in a,
    // we need it to be 1 (direct control); if 0, we need X-control-X.
    let mut pre_x = Vec::new();
    let mut post_x = Vec::new();
    let mut control_wires = Vec::new();

    for bit in 0..n {
        if bit == target_bit {
            continue;
        }
        let wire_idx = n - 1 - bit;
        if (a >> bit) & 1 == 0 {
            // Need this qubit to be 0 for the transposition
            pre_x.push(x(wires[wire_idx]));
            post_x.push(x(wires[wire_idx]));
        }
        control_wires.push(wires[wire_idx]);
    }

    let mut ops = Vec::new();
    ops.extend(pre_x);
    ops.extend(decompose_multi_cx(&control_wires, wires[target_wire], None));
    ops.extend(post_x);
    ops
}
