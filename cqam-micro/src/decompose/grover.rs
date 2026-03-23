//! Grover diffusion operator and GroverIter kernel decomposers.

use std::f64::consts::PI;
use cqam_core::circuit_ir::{Op, QWire};
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;
use super::helpers::{h, x, rz, cx, cz, t_gate, tdg};
use super::controlled::toffoli;
use super::diagonal::diagonal_to_gates;

// =============================================================================
// Building blocks
// =============================================================================

/// RCCX: relative-phase Toffoli (Maslov 2016).
///
/// Implements CCX up to relative phases on |01,*⟩ and |10,*⟩ subspaces.
/// Uses 3 CX gates (vs 6 for standard Toffoli). The relative phases cancel
/// in specific compute-uncompute patterns with CLEAN ancillae.
///
/// Gate sequence (9 gates: 2 H, 2 T, 2 Tdg, 3 CX):
///   H(target), T(target),
///   CX(c1, target), Tdg(target),
///   CX(c0, target), T(target),
///   CX(c1, target), Tdg(target),
///   H(target)
fn rccx(c0: QWire, c1: QWire, target: QWire) -> Vec<Op> {
    vec![
        h(target),
        t_gate(target),
        cx(c1, target),
        tdg(target),
        cx(c0, target),
        t_gate(target),
        cx(c1, target),
        tdg(target),
        h(target),
    ]
}

/// "Action gadget" — the first half of RCCX (used by Iten dirty-ancilla algorithm).
///
/// action_gadget(c0, c1, target):
///   H(target), T(target), CX(c0, target), Tdg(target), CX(c1, target)
///
/// Together with reset_gadget, this constitutes one RCCX:
///   action_gadget + reset_gadget = RCCX
fn action_gadget(c0: QWire, c1: QWire, target: QWire) -> Vec<Op> {
    vec![
        h(target),
        t_gate(target),
        cx(c0, target),
        tdg(target),
        cx(c1, target),
    ]
}

/// "Reset gadget" — the second half of RCCX (used by Iten dirty-ancilla algorithm).
///
/// reset_gadget(c0, c1, target):
///   CX(c1, target), T(target), CX(c0, target), Tdg(target), H(target)
fn reset_gadget(c0: QWire, c1: QWire, target: QWire) -> Vec<Op> {
    vec![
        cx(c1, target),
        t_gate(target),
        cx(c0, target),
        tdg(target),
        h(target),
    ]
}

/// MCX with dirty ancillae (Iten et al. 2016, Lemma 8 / Qiskit synth_mcx_n_dirty_i15).
///
/// Decomposes MCX with m = controls.len() controls using m-2 dirty ancilla
/// qubits (`dirty` slice, any initial state including superposition; restored
/// after use).
///
/// Requires: controls.len() >= 2.
///
/// For m == 2: standard Toffoli.
/// For m == 3: 4-Toffoli construction (24 CX), correct for dirty ancilla.
/// For m >= 4: double-pass action/reset cascade (Iten 2016), ~8m CX.
///
/// The algorithm uses TWO passes over the cascade. Each pass:
///   1. Toffoli(c[m-1], d[m-3], target)    — fires when controls[-1] AND dirty[-1] set
///   2. action_gadget cascade (reverse order)
///   3. RCCX(c[0], c[1], d[0])
///   4. reset_gadget cascade (forward order)
///
/// The two passes together produce the exact MCX action despite dirty ancillae.
fn mcx_dirty(controls: &[QWire], target: QWire, dirty: &[QWire]) -> Vec<Op> {
    let m = controls.len();
    debug_assert!(m >= 2, "mcx_dirty requires >= 2 controls");

    match m {
        2 => {
            // Standard Toffoli — always exact.
            toffoli(controls[0], controls[1], target)
        }
        3 => {
            // 4-Toffoli construction for 1 dirty ancilla.
            // Net effect: target XOR AND(c0,c1,c2) regardless of dirty state.
            //   Toffoli(c0, c1, d0)  → d0 = d0 XOR AND(c0,c1)
            //   Toffoli(c2, d0, t)   → t = t XOR AND(c2, d0_new)
            //   Toffoli(c0, c1, d0)  → restore d0
            //   Toffoli(c2, d0, t)   → t XOR AND(c2, d0_orig) again
            //   net: t XOR (c2 AND AND(c0,c1) XOR c2 AND d0) XOR (c2 AND d0)
            //      = t XOR c2 AND AND(c0,c1) = t XOR AND(c0,c1,c2) ✓
            debug_assert!(dirty.len() >= 1, "mcx_dirty(m=3) needs 1 dirty ancilla");
            let d0 = dirty[0];
            let mut ops = Vec::new();
            ops.extend(toffoli(controls[0], controls[1], d0));
            ops.extend(toffoli(controls[2], d0, target));
            ops.extend(toffoli(controls[0], controls[1], d0));
            ops.extend(toffoli(controls[2], d0, target));
            ops
        }
        _ => {
            // m >= 4: Iten 2016 double-pass algorithm.
            // Uses m-2 dirty ancillae (in dirty[0..m-3]).
            debug_assert!(
                dirty.len() >= m - 2,
                "mcx_dirty: need {} dirty ancillae, got {}",
                m - 2, dirty.len()
            );
            let mut ops = Vec::new();
            for _pass in 0..2 {
                // Core Toffoli: fires when c[m-1] AND d[m-3] are both 1.
                ops.extend(toffoli(controls[m - 1], dirty[m - 3], target));

                // Action gadget cascade (reverse order, i from m-4 down to 0):
                for i in (0..(m - 3)).rev() {
                    ops.extend(action_gadget(controls[i + 2], dirty[i], dirty[i + 1]));
                }

                // RCCX at the bottom of the chain:
                ops.extend(rccx(controls[0], controls[1], dirty[0]));

                // Reset gadget cascade (forward order, i from 0 to m-4):
                for i in 0..(m - 3) {
                    ops.extend(reset_gadget(controls[i + 2], dirty[i], dirty[i + 1]));
                }
            }
            ops
        }
    }
}

/// MCX with one clean ancilla qubit (Barenco 1995 Lemma 7.3 + Iten 2016).
///
/// Decomposes MCX with m = controls.len() >= 3 controls using one ancilla
/// initialized to |0⟩. Uses 4 calls to mcx_dirty with the two halves of
/// controls providing dirty ancillae for each other.
///
/// Gate count: roughly 4 × mcx_dirty(m/2) ≈ 4 × 8(m/2) = 16m CX.
fn mcx_with_clean_ancilla(controls: &[QWire], target: QWire, ancilla: QWire) -> Vec<Op> {
    let m = controls.len();
    debug_assert!(m >= 3, "mcx_with_clean_ancilla requires >= 3 controls");

    if m == 3 {
        // Special case from spec section 5 for n=4 MCZ, 3 controls:
        // RCCX(c0,c1,anc) + Toffoli(c2,anc,target) + RCCX(c0,c1,anc)
        // Correct because ancilla starts clean (|0>).
        let mut ops = rccx(controls[0], controls[1], ancilla);
        ops.extend(toffoli(controls[2], ancilla, target));
        ops.extend(rccx(controls[0], controls[1], ancilla));
        return ops;
    }

    let k = (m + 1) / 2; // k = ceil(m/2)
    let first  = &controls[..k];   // first half controls
    let second = &controls[k..];   // second half controls

    let mut ops = Vec::new();

    // Step 1: AND first half into ancilla, using second as dirty.
    // ancilla starts clean (|0>) so mcx_dirty is correct for any initial state of second.
    ops.extend(mcx_dirty(first, ancilla, second));

    // Step 2: MCX conditioned on second ++ [ancilla] → target, using first as dirty.
    // After step 1, ancilla = AND(first) if first is all-1 (w/ dirty second).
    // With Iten's dirty ancilla algorithm, this correctly computes:
    //   target XOR AND(second, ancilla) = target XOR AND(second, AND(first)) = target XOR AND(all controls)
    let second_plus_anc: Vec<QWire> = second.iter().copied()
        .chain(std::iter::once(ancilla))
        .collect();
    ops.extend(mcx_dirty(&second_plus_anc, target, first));

    // Step 3: Uncompute ancilla (identical to step 1).
    ops.extend(mcx_dirty(first, ancilla, second));

    // Step 4: Repeat step 2 (required for exact Barenco 4-step construction).
    ops.extend(mcx_dirty(&second_plus_anc, target, first));

    ops
}

// =============================================================================
// Public decomposers
// =============================================================================

/// Decompose a multi-controlled-Z gate on the given wires.
///
/// MCZ flips the phase of |1...1⟩.
///
/// | n | Method                                         | CX count  |
/// |---|------------------------------------------------|-----------|
/// | 0 | no-op                                          | 0         |
/// | 1 | Rz(π)                                          | 0         |
/// | 2 | CZ(w0, w1)                                     | 0         |
/// | 3 | H(w2) · Toffoli · H(w2)                        | 6         |
/// | 4 | H(w3) · RCCX · Toffoli · RCCX · H(w3)         | 12        |
/// | ≥5 | H(wn-1) · mcx_with_clean_ancilla · H(wn-1)   | ~16(n-1)  |
pub(super) fn decompose_mcz(wires: &[QWire], ancilla: Option<QWire>) -> Vec<Op> {
    let n = wires.len();
    match n {
        0 => vec![],
        1 => {
            // Z gate on single qubit = phase flip on |1⟩
            vec![rz(wires[0], PI)]
        }
        2 => {
            // CZ gate
            vec![cz(wires[0], wires[1])]
        }
        3 => {
            // MCZ = H(target) · Toffoli(c0, c1, target) · H(target)
            let mut ops = vec![h(wires[2])];
            ops.extend(toffoli(wires[0], wires[1], wires[2]));
            ops.push(h(wires[2]));
            ops
        }
        4 => {
            // n=4: H(w3) · RCCX(w0,w1,a) · Toffoli(w2,a,w3) · RCCX(w0,w1,a) · H(w3)
            // CX count: 3 + 6 + 3 = 12.
            // Ancilla starts clean; RCCX sets ancilla = AND(w0,w1). Correct.
            let anc = ancilla.expect("decompose_mcz(n=4): ancilla required");
            let mut ops = vec![h(wires[3])];
            ops.extend(rccx(wires[0], wires[1], anc));
            ops.extend(toffoli(wires[2], anc, wires[3]));
            ops.extend(rccx(wires[0], wires[1], anc));
            ops.push(h(wires[3]));
            ops
        }
        _ => {
            // n >= 5: H(wn-1) · mcx_with_clean_ancilla(controls, wn-1, a) · H(wn-1)
            let anc = ancilla.expect("decompose_mcz(n>=5): ancilla required");
            let controls = &wires[..n - 1];
            let target = wires[n - 1];
            let mut ops = vec![h(target)];
            ops.extend(mcx_with_clean_ancilla(controls, target, anc));
            ops.push(h(target));
            ops
        }
    }
}

/// Decompose a multi-controlled-X gate.
///
/// | controls | Method                                |
/// |----------|---------------------------------------|
/// | 0        | X(target)                             |
/// | 1        | CX(c0, target)                        |
/// | 2        | Toffoli(c0, c1, target)               |
/// | ≥ 3      | mcx_with_clean_ancilla (if ancilla)   |
/// | ≥ 3      | fallback via decompose_mcz (no ancilla)|
pub(super) fn decompose_multi_cx(controls: &[QWire], target: QWire, ancilla: Option<QWire>) -> Vec<Op> {
    let n = controls.len();
    match n {
        0 => vec![x(target)],
        1 => vec![cx(controls[0], target)],
        2 => toffoli(controls[0], controls[1], target),
        _ => {
            if let Some(anc) = ancilla {
                mcx_with_clean_ancilla(controls, target, anc)
            } else {
                // Fallback: no ancilla available. Use the Gray-code diagonal
                // decomposition (exponential but correct for small n).
                // MCX = H(tgt) · MCZ(controls ++ [tgt]) · H(tgt)
                // MCZ expressed as diagonal with phase pi on |1..1⟩.
                let all_wires: Vec<QWire> = controls.iter().copied()
                    .chain(std::iter::once(target))
                    .collect();
                let m = all_wires.len();
                let dim = 1usize << m;
                let mut phases = vec![0.0f64; dim];
                phases[dim - 1] = PI;
                let mut ops = vec![h(target)];
                ops.extend(diagonal_to_gates(&all_wires, &phases));
                ops.push(h(target));
                ops
            }
        }
    }
}

/// Decompose the Diffuse (Grover diffusion) kernel.
///
/// D = H^n · X^n · MCZ · X^n · H^n
pub fn decompose_diffuse(wires: &[QWire], _params: &KernelParams, ancilla: Option<QWire>) -> Result<Vec<Op>, MicroError> {
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
    ops.extend(decompose_mcz(wires, ancilla));

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
pub fn decompose_grover(wires: &[QWire], params: &KernelParams, ancilla: Option<QWire>) -> Result<Vec<Op>, MicroError> {
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

        // MCZ on all wires (ancilla shared across oracle and diffusion)
        ops.extend(decompose_mcz(wires, ancilla));

        // Undo X gates
        for (i, &wire) in wires.iter().enumerate() {
            let bit_pos = n - 1 - i;
            if (target >> bit_pos) & 1 == 0 {
                ops.push(x(wire));
            }
        }
    }

    // Diffusion phase
    ops.extend(decompose_diffuse(wires, params, ancilla)?);

    Ok(ops)
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
    // Helper: build the ideal MCZ matrix (diag(1,...,1,-1)) for n qubits
    // -----------------------------------------------------------------------

    fn mcz_matrix(n: usize) -> Vec<C64> {
        let dim = 1usize << n;
        let mut m = vec![C64::ZERO; dim * dim];
        for i in 0..dim {
            m[i * dim + i] = C64::ONE;
        }
        m[(dim - 1) * dim + (dim - 1)] = C64(-1.0, 0.0);
        m
    }

    // -----------------------------------------------------------------------
    // RCCX: self-inverse test
    // -----------------------------------------------------------------------

    #[test]
    fn test_rccx_self_inverse() {
        // Apply RCCX twice on 3 qubits; result should be identity.
        let c0 = QWire(0);
        let c1 = QWire(1);
        let t  = QWire(2);
        let mut ops = rccx(c0, c1, t);
        ops.extend(rccx(c0, c1, t));
        let u = gate_sequence_unitary(&ops, 3);
        // Identity 8x8
        let dim = 8;
        let mut expected = vec![C64::ZERO; dim * dim];
        for i in 0..dim { expected[i * dim + i] = C64::ONE; }
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-9),
            "RCCX applied twice should equal identity"
        );
    }

    // -----------------------------------------------------------------------
    // RCCX: computational-basis action (maps |110⟩ ↔ |111⟩)
    // -----------------------------------------------------------------------

    #[test]
    fn test_rccx_computational_action() {
        let c0 = QWire(0);
        let c1 = QWire(1);
        let t  = QWire(2);
        let ops = rccx(c0, c1, t);
        let u = gate_sequence_unitary(&ops, 3);
        let dim = 8;
        // |110⟩ = index 6 (big-endian), |111⟩ = index 7
        // Column 6 of U should map |110⟩ → |111⟩ (up to phase)
        let col6_row7 = u[7 * dim + 6];
        assert!(
            col6_row7.norm() > 0.99,
            "RCCX should flip |110⟩ ↔ |111⟩, got amplitude {col6_row7:?}"
        );
        let col7_row6 = u[6 * dim + 7];
        assert!(
            col7_row6.norm() > 0.99,
            "RCCX should flip |110⟩ ↔ |111⟩ (reverse), got amplitude {col7_row6:?}"
        );
    }

    // -----------------------------------------------------------------------
    // MCZ unitary equivalence: n = 2, 3
    // (No ancilla needed for these cases)
    // -----------------------------------------------------------------------

    #[test]
    fn test_mcz_n2() {
        let wires = [QWire(0), QWire(1)];
        let ops = decompose_mcz(&wires, None);
        let u = gate_sequence_unitary(&ops, 2);
        let expected = mcz_matrix(2);
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-9),
            "MCZ(n=2) unitary mismatch"
        );
    }

    #[test]
    fn test_mcz_n3() {
        let wires = [QWire(0), QWire(1), QWire(2)];
        let ops = decompose_mcz(&wires, None);
        let u = gate_sequence_unitary(&ops, 3);
        let expected = mcz_matrix(3);
        assert!(
            unitaries_equal_up_to_phase(&u, &expected, 1e-9),
            "MCZ(n=3) unitary mismatch"
        );
    }

    // -----------------------------------------------------------------------
    // MCZ unitary equivalence: n = 4, 5
    // (Ancilla = QWire(n), simulate on n+1 qubits, project onto ancilla=|0⟩)
    // -----------------------------------------------------------------------

    fn mcz_ancilla_test(n: usize) {
        let wires: Vec<QWire> = (0..n).map(|i| QWire(i as u32)).collect();
        let ancilla = QWire(n as u32);
        let ops = decompose_mcz(&wires, Some(ancilla));
        let total_qubits = n + 1; // ancilla is the last qubit (LSB in big-endian)
        let u = gate_sequence_unitary(&ops, total_qubits as u8);

        // Extract the n-qubit unitary by projecting onto ancilla = |0⟩.
        // Ancilla = QWire(n) is qubit index n (LSB = bit 0 of state index).
        // ancilla=0 → even-indexed basis states (last bit = 0).
        let dim_full = 1usize << total_qubits;
        let dim_n    = 1usize << n;

        let indices: Vec<usize> = (0..dim_full).filter(|i| i % 2 == 0).collect();
        assert_eq!(indices.len(), dim_n);

        let mut proj = vec![C64::ZERO; dim_n * dim_n];
        for (ri, &row) in indices.iter().enumerate() {
            for (ci, &col) in indices.iter().enumerate() {
                proj[ri * dim_n + ci] = u[row * dim_full + col];
            }
        }

        let expected = mcz_matrix(n);
        assert!(
            unitaries_equal_up_to_phase(&proj, &expected, 1e-9),
            "MCZ(n={n}) unitary mismatch (ancilla projection)"
        );
    }

    #[test]
    fn test_mcz_n4() { mcz_ancilla_test(4); }

    #[test]
    fn test_mcz_n5() { mcz_ancilla_test(5); }

    // -----------------------------------------------------------------------
    // MCX equivalence: n = 3, 4 controls with ancilla
    // -----------------------------------------------------------------------

    fn mcx_matrix(n_controls: usize) -> Vec<C64> {
        // MCX on (n_controls + 1) qubits: identity except |1..10⟩ ↔ |1..11⟩
        let total = n_controls + 1;
        let dim = 1usize << total;
        let mut m = vec![C64::ZERO; dim * dim];
        for i in 0..dim { m[i * dim + i] = C64::ONE; }
        // |1..10⟩ = dim-2, |1..11⟩ = dim-1
        m[(dim - 2) * dim + (dim - 2)] = C64::ZERO;
        m[(dim - 1) * dim + (dim - 1)] = C64::ZERO;
        m[(dim - 2) * dim + (dim - 1)] = C64::ONE;
        m[(dim - 1) * dim + (dim - 2)] = C64::ONE;
        m
    }

    fn mcx_ancilla_test(n_controls: usize) {
        let controls: Vec<QWire> = (0..n_controls).map(|i| QWire(i as u32)).collect();
        let target  = QWire(n_controls as u32);
        let ancilla = QWire((n_controls + 1) as u32);
        let ops = decompose_multi_cx(&controls, target, Some(ancilla));
        let total_qubits = n_controls + 2; // +1 target, +1 ancilla
        let u = gate_sequence_unitary(&ops, total_qubits as u8);

        // Project out ancilla (last qubit, ancilla=|0⟩ → even indices)
        let dim_full = 1usize << total_qubits;
        let dim_n    = 1usize << (n_controls + 1);
        let indices: Vec<usize> = (0..dim_full).filter(|i| i % 2 == 0).collect();
        assert_eq!(indices.len(), dim_n);

        let mut proj = vec![C64::ZERO; dim_n * dim_n];
        for (ri, &row) in indices.iter().enumerate() {
            for (ci, &col) in indices.iter().enumerate() {
                proj[ri * dim_n + ci] = u[row * dim_full + col];
            }
        }

        let expected = mcx_matrix(n_controls);
        assert!(
            unitaries_equal_up_to_phase(&proj, &expected, 1e-9),
            "MCX({n_controls} controls) unitary mismatch"
        );
    }

    #[test]
    fn test_mcx_n3_controls() { mcx_ancilla_test(3); }

    #[test]
    fn test_mcx_n4_controls() { mcx_ancilla_test(4); }

    // -----------------------------------------------------------------------
    // Gate count regression: MCZ(16) should produce far fewer CX than old
    // diagonal_to_gates approach (old: 917,508 CX, new: O(n))
    // -----------------------------------------------------------------------

    #[test]
    fn test_mcz_n16_gate_count() {
        let wires: Vec<QWire> = (0..16u32).map(QWire).collect();
        let ancilla = QWire(16);
        let ops = decompose_mcz(&wires, Some(ancilla));
        let cx_count = count_cx(&ops);
        // Old diagonal: 917,508 CX. New V-chain: << 5000.
        assert!(
            cx_count < 5000,
            "MCZ(16) should use < 5000 CX gates, got {cx_count}"
        );
        assert!(
            ops.len() < 20_000,
            "MCZ(16) should use < 20,000 total gates, got {}",
            ops.len()
        );
    }

    // -----------------------------------------------------------------------
    // Grover integration: n=4 and n=5 complete without panic and produce
    // a bounded gate count
    // -----------------------------------------------------------------------

    fn grover_params(target: usize) -> KernelParams {
        KernelParams::Int { param0: target as i64, param1: 0, cmem_data: vec![] }
    }

    #[test]
    fn test_grover_n4_completes() {
        let wires: Vec<QWire> = (0..4u32).map(QWire).collect();
        let ancilla = QWire(4);
        let params = grover_params(0b1111);
        let ops = decompose_grover(&wires, &params, Some(ancilla)).unwrap();
        assert!(
            ops.len() < 50_000,
            "Grover(n=4) produced too many gates: {}",
            ops.len()
        );
    }

    #[test]
    fn test_grover_n5_completes() {
        let wires: Vec<QWire> = (0..5u32).map(QWire).collect();
        let ancilla = QWire(5);
        let params = grover_params(0b11111);
        let ops = decompose_grover(&wires, &params, Some(ancilla)).unwrap();
        assert!(
            ops.len() < 50_000,
            "Grover(n=5) produced too many gates: {}",
            ops.len()
        );
    }
}
