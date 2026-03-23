//! Permutation kernel decomposer.
//!
//! Decomposes an arbitrary permutation of basis states into a sequence of
//! standard gates via cycle decomposition into transpositions.
//!
//! For structured permutations — full cyclic shifts or coin-conditioned cyclic
//! shifts (the quantum-walk pattern) — an O(p^2) increment/decrement circuit
//! is emitted instead of the generic O(2^n) transposition path.

use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::{x, cx};
use super::params::extract_int_params;
use super::grover::decompose_multi_cx;

// =============================================================================
// Permutation structure classification
// =============================================================================

/// Classification of the mathematical structure of a permutation.
#[derive(Debug, PartialEq)]
enum PermutationStructure {
    /// Every element maps to itself (identity).
    Identity,
    /// Full cyclic shift: sigma(k) = (k + amount).rem_euclid(dim) for all k.
    CyclicShift { amount: i64 },
    /// Coin-conditioned cyclic shift (quantum walk pattern).
    ///
    /// The state space splits into two halves by the MSB (the "coin" qubit).
    /// Within each half, the lower bits undergo an independent cyclic shift.
    ///
    /// - `shift_0`: shift applied in the coin=0 sector (first half).
    /// - `shift_1`: shift applied in the coin=1 sector (second half).
    CoinConditionedShift { shift_0: i64, shift_1: i64 },
    /// No special structure detected; fall through to generic decomposition.
    Arbitrary,
}

/// Classify the mathematical structure of a permutation table.
///
/// Checks for, in order:
/// 1. Identity
/// 2. Full cyclic shift (+1 or -1)
/// 3. Coin-conditioned cyclic shift (quantum walk pattern, 1-bit coin = MSB)
/// 4. Arbitrary (no structure recognised)
fn classify_permutation(table: &[usize], n: usize) -> PermutationStructure {
    let dim = 1usize << n;
    debug_assert_eq!(table.len(), dim);

    // --- Identity ---
    if table.iter().enumerate().all(|(i, &v)| v == i) {
        return PermutationStructure::Identity;
    }

    // --- Full cyclic shift ---
    // Check +1 mod dim
    if table.iter().enumerate().all(|(k, &v)| v == (k + 1) % dim) {
        return PermutationStructure::CyclicShift { amount: 1 };
    }
    // Check -1 mod dim
    if table.iter().enumerate().all(|(k, &v)| v == (k + dim - 1) % dim) {
        return PermutationStructure::CyclicShift { amount: -1 };
    }

    // --- Coin-conditioned cyclic shift (1-bit coin = MSB) ---
    // The table splits into two equal halves; each half is an independent
    // cyclic shift on the lower (n-1) bits, preserving the MSB.
    if n >= 2 {
        let half = dim / 2;

        // For sector 0, base=0, position pos=0..half-1.
        // sigma(pos) should equal (pos + shift) mod half.
        // Derive the canonical shift from sigma(0): shift_canon = sigma(0) mod half.
        let shift_0_canon = (table[0] as isize).rem_euclid(half as isize);

        let sector0_ok = (0..half).all(|k| {
            table[k] == ((k as isize + shift_0_canon).rem_euclid(half as isize)) as usize
        });

        if sector0_ok {
            // Now check sector 1 (base = half)
            let shift_1_raw = table[half] as isize - half as isize;
            let shift_1_canon = shift_1_raw.rem_euclid(half as isize);

            let sector1_ok = (0..half).all(|pos| {
                let k = half + pos;
                let expected_pos =
                    ((pos as isize + shift_1_canon).rem_euclid(half as isize)) as usize;
                table[k] == half + expected_pos
            });

            if sector1_ok {
                // Convert canonical shifts (0..half) to signed shifts (-half/2..half/2)
                // so that +1 is "increment" and -1 is "decrement".
                let to_signed = |canon: isize| -> i64 {
                    if canon <= (half as isize) / 2 {
                        canon as i64
                    } else {
                        canon as i64 - half as i64
                    }
                };
                let s0 = to_signed(shift_0_canon);
                let s1 = to_signed(shift_1_canon);
                return PermutationStructure::CoinConditionedShift {
                    shift_0: s0,
                    shift_1: s1,
                };
            }
        }
    }

    PermutationStructure::Arbitrary
}

// =============================================================================
// Increment / decrement circuits
// =============================================================================

/// Emit the increment (+1 mod 2^n) circuit on the given wires.
///
/// Convention: `wires[0]` = MSB, `wires[n-1]` = LSB (big-endian).
///
/// Gate cascade (Vedral / Nielsen-Chuang ripple-carry):
/// ```text
/// for i in 0..n-1:
///     MCX(controls = wires[i+1..n-1], target = wires[i])
/// X(wires[n-1])   // LSB always flips
/// ```
///
/// Bit `wires[i]` flips when all less-significant bits `wires[i+1..n-1]` are 1,
/// i.e. carry has propagated all the way up to position i.
fn increment_circuit(wires: &[QWire], ancilla: Option<QWire>) -> Vec<Op> {
    let n = wires.len();
    let mut ops = Vec::new();

    for i in 0..n.saturating_sub(1) {
        let controls: Vec<QWire> = wires[i + 1..n].iter().copied().collect();
        ops.extend(decompose_multi_cx(&controls, wires[i], ancilla));
    }
    // LSB always flips
    if n > 0 {
        ops.push(x(wires[n - 1]));
    }

    ops
}

/// Emit the decrement (-1 mod 2^n) circuit: adjoint of the increment.
///
/// Since every gate in the increment circuit is self-adjoint (MCX, X),
/// the decrement is the increment sequence in reverse order.
fn decrement_circuit(wires: &[QWire], ancilla: Option<QWire>) -> Vec<Op> {
    let n = wires.len();
    let mut ops = Vec::new();

    // Reverse of increment: X(LSB) first, then MCX cascade in reverse
    if n > 0 {
        ops.push(x(wires[n - 1]));
    }
    for i in (0..n.saturating_sub(1)).rev() {
        let controls: Vec<QWire> = wires[i + 1..n].iter().copied().collect();
        ops.extend(decompose_multi_cx(&controls, wires[i], ancilla));
    }

    ops
}

/// Emit a controlled increment (+1 mod 2^p) with `coin` as an extra control.
///
/// Every gate in the increment circuit gets `coin` prepended to its control list.
fn controlled_increment(coin: QWire, pos_wires: &[QWire], ancilla: Option<QWire>) -> Vec<Op> {
    let p = pos_wires.len();
    let mut ops = Vec::new();

    for i in 0..p.saturating_sub(1) {
        let mut controls: Vec<QWire> = vec![coin];
        controls.extend_from_slice(&pos_wires[i + 1..p]);
        ops.extend(decompose_multi_cx(&controls, pos_wires[i], ancilla));
    }
    // Coin-controlled X on LSB
    if p > 0 {
        ops.push(cx(coin, pos_wires[p - 1]));
    }

    ops
}

/// Emit a controlled decrement (-1 mod 2^p) with `coin` as an extra control.
///
/// Adjoint of controlled increment: same cascade in reverse order.
fn controlled_decrement(coin: QWire, pos_wires: &[QWire], ancilla: Option<QWire>) -> Vec<Op> {
    let p = pos_wires.len();
    let mut ops = Vec::new();

    // Coin-controlled X on LSB first
    if p > 0 {
        ops.push(cx(coin, pos_wires[p - 1]));
    }
    for i in (0..p.saturating_sub(1)).rev() {
        let mut controls: Vec<QWire> = vec![coin];
        controls.extend_from_slice(&pos_wires[i + 1..p]);
        ops.extend(decompose_multi_cx(&controls, pos_wires[i], ancilla));
    }

    ops
}

/// Emit the coin-conditioned shift operator.
///
/// `wires[0]` = coin (MSB), `wires[1..]` = position bits.
/// When coin=0: apply a cyclic shift by `shift_0` on the position register.
/// When coin=1: apply a cyclic shift by `shift_1` on the position register.
///
/// Currently handles shift_0, shift_1 in {-1, 0, +1} only; anything else
/// is silently skipped (the caller's classifier guarantees ±1 for walk patterns).
fn coin_conditioned_shift_circuit(
    wires: &[QWire],
    shift_0: i64,
    shift_1: i64,
    ancilla: Option<QWire>,
) -> Vec<Op> {
    let coin = wires[0];
    let pos = &wires[1..];
    let mut ops = Vec::new();

    // Coin=0 sector: X(coin), controlled-shift, X(coin)
    if shift_0 != 0 {
        ops.push(x(coin));
        match shift_0 {
            1  => ops.extend(controlled_increment(coin, pos, ancilla)),
            -1 => ops.extend(controlled_decrement(coin, pos, ancilla)),
            _  => {} // general shifts not yet implemented; fall through handled by caller
        }
        ops.push(x(coin));
    }

    // Coin=1 sector: controlled-shift directly (coin=1 is the natural state)
    if shift_1 != 0 {
        match shift_1 {
            1  => ops.extend(controlled_increment(coin, pos, ancilla)),
            -1 => ops.extend(controlled_decrement(coin, pos, ancilla)),
            _  => {}
        }
    }

    ops
}

// =============================================================================
// Generic decomposition (cycle -> transpositions)
// =============================================================================

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

/// Decompose a transposition of basis states |a⟩ ↔ |b⟩ into gates.
///
/// Uses the approach: find differing bits, chain single-bit transpositions.
fn decompose_transposition(
    wires: &[QWire],
    n: usize,
    a: usize,
    b: usize,
    ancilla: Option<QWire>,
) -> Vec<Op> {
    if a == b {
        return vec![];
    }
    let diff = a ^ b;
    let diff_bits: Vec<usize> = (0..n).filter(|&i| (diff >> i) & 1 == 1).collect();

    if diff_bits.len() == 1 {
        // Single-bit difference: multi-controlled-X
        return decompose_single_bit_transposition(wires, n, a, b, diff_bits[0], ancilla);
    }

    // Multi-bit difference: chain through intermediates.
    // Swap (a, b) where they differ in multiple bits.
    // Use (a, c)(c, b)(a, c) where c differs from a in only the lowest diff bit.
    let lowest_bit = diff_bits[0];
    let c = a ^ (1 << lowest_bit);

    let mut ops = Vec::new();
    ops.extend(decompose_transposition(wires, n, a, c, ancilla));
    ops.extend(decompose_transposition(wires, n, c, b, ancilla));
    ops.extend(decompose_transposition(wires, n, a, c, ancilla));
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
    ancilla: Option<QWire>,
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
    ops.extend(decompose_multi_cx(&control_wires, wires[target_wire], ancilla));
    ops.extend(post_x);
    ops
}

/// Generic decomposition via cycle -> transposition -> single-bit transposition.
fn decompose_generic(
    wires: &[QWire],
    n: usize,
    table: &[usize],
    ancilla: Option<QWire>,
) -> Result<Vec<Op>, MicroError> {
    let transpositions = permutation_to_transpositions(table);
    let mut ops = Vec::new();
    for (a, b) in transpositions {
        ops.extend(decompose_transposition(wires, n, a, b, ancilla));
    }
    Ok(ops)
}

// =============================================================================
// Public API
// =============================================================================

/// Decompose a permutation kernel into gates.
///
/// Supports up to n <= 10 qubits.  Uses structural classification first:
/// - Cyclic shift patterns → O(n^2) increment/decrement circuit.
/// - Coin-conditioned cyclic shifts (quantum walk) → O(n^2) controlled shift.
/// - Arbitrary → cycle decomposition into transpositions (generic path).
///
/// The `ancilla` wire (if provided) is passed to all internal MCX calls,
/// enabling the V-chain O(n) Toffoli decomposition instead of the exponential
/// diagonal fallback.
pub fn decompose_permutation(
    wires: &[QWire],
    params: &KernelParams,
    ancilla: Option<QWire>,
) -> Result<Vec<Op>, MicroError> {
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

    // Special case: n=1
    if n == 1 {
        if table == [1, 0] {
            return Ok(vec![x(wires[0])]);
        }
        return Ok(vec![]);
    }

    // Classify and dispatch
    match classify_permutation(&table, n) {
        PermutationStructure::Identity => Ok(vec![]),

        PermutationStructure::CyclicShift { amount } => match amount {
            1  => Ok(increment_circuit(wires, ancilla)),
            -1 => Ok(decrement_circuit(wires, ancilla)),
            _  => decompose_generic(wires, n, &table, ancilla),
        },

        PermutationStructure::CoinConditionedShift { shift_0, shift_1 } => {
            // Only handle ±1 shifts with the optimised circuit.
            // For other shift amounts, fall back to generic decomposition.
            let both_simple = matches!(shift_0, -1 | 0 | 1) && matches!(shift_1, -1 | 0 | 1);
            if both_simple {
                Ok(coin_conditioned_shift_circuit(wires, shift_0, shift_1, ancilla))
            } else {
                decompose_generic(wires, n, &table, ancilla)
            }
        }

        PermutationStructure::Arbitrary => decompose_generic(wires, n, &table, ancilla),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::circuit_ir::QWire;
    use cqam_core::complex::C64;
    use super::super::tests::{gate_sequence_unitary, unitaries_equal_up_to_phase};

    // -----------------------------------------------------------------------
    // Helper: count CX gates in a Vec<Op>
    // -----------------------------------------------------------------------

    fn count_cx(ops: &[Op]) -> usize {
        use cqam_core::circuit_ir::{Gate2q, ApplyGate2q};
        ops.iter().filter(|op| {
            matches!(op, Op::Gate2q(ApplyGate2q { gate: Gate2q::Cx, .. }))
        }).count()
    }

    // -----------------------------------------------------------------------
    // Helper: build permutation matrix for a given table (no ancilla involved)
    // -----------------------------------------------------------------------

    /// Expected permutation matrix: column k has a 1 at row table[k].
    fn perm_matrix(table: &[usize]) -> Vec<C64> {
        let dim = table.len();
        let mut m = vec![C64::ZERO; dim * dim];
        for (k, &target) in table.iter().enumerate() {
            m[target * dim + k] = C64::ONE;
        }
        m
    }

    // -----------------------------------------------------------------------
    // Helpers for projecting ancilla out of a unitary
    // -----------------------------------------------------------------------

    /// Compute the circuit unitary on n+1 qubits (last = ancilla), then project
    /// onto the ancilla=|0⟩ subspace to recover the n-qubit action.
    fn projected_unitary(ops: &[Op], n: usize) -> Vec<C64> {
        let total = n + 1;
        let u = gate_sequence_unitary(ops, total as u8);
        let dim_full = 1usize << total;
        let dim_n    = 1usize << n;
        // Ancilla = QWire(n) is the last qubit (LSB in big-endian).
        // ancilla=0 → even-indexed basis states (last bit = 0).
        let indices: Vec<usize> = (0..dim_full).filter(|i| i % 2 == 0).collect();
        let mut proj = vec![C64::ZERO; dim_n * dim_n];
        for (ri, &row) in indices.iter().enumerate() {
            for (ci, &col) in indices.iter().enumerate() {
                proj[ri * dim_n + ci] = u[row * dim_full + col];
            }
        }
        proj
    }

    // -----------------------------------------------------------------------
    // Classification tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_identity() {
        let table = vec![0, 1, 2, 3, 4, 5, 6, 7];
        assert_eq!(classify_permutation(&table, 3), PermutationStructure::Identity);
    }

    #[test]
    fn test_classify_increment_3q() {
        // +1 mod 8
        let table = vec![1, 2, 3, 4, 5, 6, 7, 0];
        assert_eq!(
            classify_permutation(&table, 3),
            PermutationStructure::CyclicShift { amount: 1 }
        );
    }

    #[test]
    fn test_classify_decrement_3q() {
        // -1 mod 8
        let table = vec![7, 0, 1, 2, 3, 4, 5, 6];
        assert_eq!(
            classify_permutation(&table, 3),
            PermutationStructure::CyclicShift { amount: -1 }
        );
    }

    #[test]
    fn test_classify_coin_conditioned_3q() {
        // 1 coin + 2 position (n=3, dim=8)
        // Coin=0: decrement (-1 mod 4): [3, 0, 1, 2]
        // Coin=1: increment (+1 mod 4): [5, 6, 7, 4]
        let table = vec![3, 0, 1, 2, 5, 6, 7, 4];
        match classify_permutation(&table, 3) {
            PermutationStructure::CoinConditionedShift { shift_0, shift_1 } => {
                assert_eq!(shift_0, -1, "shift_0 should be -1");
                assert_eq!(shift_1,  1, "shift_1 should be +1");
            }
            other => panic!("Expected CoinConditionedShift, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_coin_conditioned_4q_walk() {
        // 1 coin + 3 position (n=4, dim=16)
        // Coin=0: decrement (-1 mod 8): [7,0,1,2,3,4,5,6]
        // Coin=1: increment (+1 mod 8): [9,10,11,12,13,14,15,8]
        let table = vec![7usize, 0, 1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14, 15, 8];
        match classify_permutation(&table, 4) {
            PermutationStructure::CoinConditionedShift { shift_0, shift_1 } => {
                assert_eq!(shift_0, -1);
                assert_eq!(shift_1,  1);
            }
            other => panic!("Expected CoinConditionedShift, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_arbitrary() {
        // Arbitrary permutation: not a cyclic shift or coin-conditioned shift
        let table = vec![3usize, 0, 2, 1, 4, 5, 6, 7];
        assert_eq!(classify_permutation(&table, 3), PermutationStructure::Arbitrary);
    }

    // -----------------------------------------------------------------------
    // Increment / decrement circuit correctness (3q, no ancilla needed for small n)
    // -----------------------------------------------------------------------

    #[test]
    fn test_increment_circuit_2q() {
        // +1 mod 4: [1,2,3,0]
        let wires: Vec<QWire> = (0..2u32).map(QWire).collect();
        let ops = increment_circuit(&wires, None);
        let u = gate_sequence_unitary(&ops, 2);
        let expected = perm_matrix(&[1, 2, 3, 0]);
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-9),
            "increment_circuit 2q unitary mismatch"
        );
    }

    #[test]
    fn test_decrement_circuit_2q() {
        // -1 mod 4: [3,0,1,2]
        let wires: Vec<QWire> = (0..2u32).map(QWire).collect();
        let ops = decrement_circuit(&wires, None);
        let u = gate_sequence_unitary(&ops, 2);
        let expected = perm_matrix(&[3, 0, 1, 2]);
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-9),
            "decrement_circuit 2q unitary mismatch"
        );
    }

    #[test]
    fn test_increment_circuit_3q() {
        // +1 mod 8: [1,2,3,4,5,6,7,0]
        let wires: Vec<QWire> = (0..3u32).map(QWire).collect();
        let ops = increment_circuit(&wires, None);
        let u = gate_sequence_unitary(&ops, 3);
        let expected = perm_matrix(&[1, 2, 3, 4, 5, 6, 7, 0]);
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-9),
            "increment_circuit 3q unitary mismatch"
        );
    }

    #[test]
    fn test_decrement_circuit_3q() {
        // -1 mod 8: [7,0,1,2,3,4,5,6]
        let wires: Vec<QWire> = (0..3u32).map(QWire).collect();
        let ops = decrement_circuit(&wires, None);
        let u = gate_sequence_unitary(&ops, 3);
        let expected = perm_matrix(&[7, 0, 1, 2, 3, 4, 5, 6]);
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-9),
            "decrement_circuit 3q unitary mismatch"
        );
    }

    #[test]
    fn test_increment_circuit_4q_with_ancilla() {
        // +1 mod 16, with ancilla at QWire(4)
        let wires: Vec<QWire> = (0..4u32).map(QWire).collect();
        let ancilla = QWire(4);
        let ops = increment_circuit(&wires, Some(ancilla));
        let proj = projected_unitary(&ops, 4);
        let expected: Vec<usize> = (1..16).chain(std::iter::once(0)).collect();
        let expected_mat = perm_matrix(&expected);
        assert!(
            unitaries_equal_up_to_phase(&proj, &expected_mat, 1e-9),
            "increment_circuit 4q (with ancilla) unitary mismatch"
        );
    }

    #[test]
    fn test_decrement_circuit_4q_with_ancilla() {
        // -1 mod 16, with ancilla at QWire(4)
        let wires: Vec<QWire> = (0..4u32).map(QWire).collect();
        let ancilla = QWire(4);
        let ops = decrement_circuit(&wires, Some(ancilla));
        let proj = projected_unitary(&ops, 4);
        let expected: Vec<usize> = std::iter::once(15).chain(0..15).collect();
        let expected_mat = perm_matrix(&expected);
        assert!(
            unitaries_equal_up_to_phase(&proj, &expected_mat, 1e-9),
            "decrement_circuit 4q (with ancilla) unitary mismatch"
        );
    }

    // -----------------------------------------------------------------------
    // Coin-conditioned shift correctness (3q = 1 coin + 2 position)
    // -----------------------------------------------------------------------

    #[test]
    fn test_coin_conditioned_shift_3q() {
        // 1 coin + 2 position (n=3)
        // Coin=0: decrement (-1 mod 4): table[0..4] = [3,0,1,2]
        // Coin=1: increment (+1 mod 4): table[4..8] = [5,6,7,4]
        let wires: Vec<QWire> = (0..3u32).map(QWire).collect();
        let ops = coin_conditioned_shift_circuit(&wires, -1, 1, None);
        let u = gate_sequence_unitary(&ops, 3);
        let expected = perm_matrix(&[3, 0, 1, 2, 5, 6, 7, 4]);
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-9),
            "coin_conditioned_shift 3q unitary mismatch"
        );
    }

    #[test]
    fn test_coin_conditioned_shift_4q_with_ancilla() {
        // 1 coin + 3 position (n=4), ancilla at QWire(4)
        // Coin=0: decrement (-1 mod 8): [7,0,1,2,3,4,5,6]
        // Coin=1: increment (+1 mod 8): [9,10,11,12,13,14,15,8]
        let wires: Vec<QWire> = (0..4u32).map(QWire).collect();
        let ancilla = QWire(4);
        let ops = coin_conditioned_shift_circuit(&wires, -1, 1, Some(ancilla));
        let proj = projected_unitary(&ops, 4);
        let expected_table = vec![7usize, 0, 1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14, 15, 8];
        let expected_mat = perm_matrix(&expected_table);
        assert!(
            unitaries_equal_up_to_phase(&proj, &expected_mat, 1e-9),
            "coin_conditioned_shift 4q unitary mismatch"
        );
    }

    // -----------------------------------------------------------------------
    // Gate count regression: 10-qubit walk shift should be << 5,000 CX
    // -----------------------------------------------------------------------

    #[test]
    fn test_cyclic_shift_10q_gate_count() {
        use cqam_core::quantum_backend::KernelParams;

        // 1 coin + 9 position (n=10, dim=1024)
        // Coin=0: decrement (-1 mod 512): sigma(k) = (k-1) mod 512 for k=0..511
        // Coin=1: increment (+1 mod 512): sigma(k) = 512 + (k-512+1) mod 512 for k=512..1023
        let dim = 1024usize;
        let half = 512usize;
        let table: Vec<i64> = (0..dim).map(|k| {
            if k < half {
                ((k + half - 1) % half) as i64
            } else {
                (half + (k - half + 1) % half) as i64
            }
        }).collect();
        let wires: Vec<QWire> = (0..10u32).map(QWire).collect();
        let ancilla = QWire(10);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: table };
        let ops = decompose_permutation(&wires, &params, Some(ancilla)).unwrap();
        let cx_count = count_cx(&ops);
        assert!(
            cx_count < 5_000,
            "10q walk shift should use < 5,000 CX, got {cx_count}"
        );
    }
}
