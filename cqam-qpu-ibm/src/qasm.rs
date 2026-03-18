//! Pure-Rust OpenQASM 3 emitter from a transpiled `SafeQkCircuit`.
//!
//! The single public entry point is [`circuit_to_qasm3`], which walks the
//! circuit's instruction list via the Task 6.5 API and produces an OpenQASM 3
//! string suitable for submission to the IBM Quantum Platform REST API.

use std::fmt::Write;

use crate::error::IbmError;
use crate::ffi;
use crate::safe::{CircuitInstructionView, SafeQkCircuit};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Emit an OpenQASM 3 program string from a transpiled `SafeQkCircuit`.
///
/// The circuit must already be transpiled to the target basis gate set.
/// Returns `Err(IbmError::ConversionError)` if an instruction has an
/// unrecognized operation kind that cannot be emitted.
pub fn circuit_to_qasm3(circuit: &SafeQkCircuit) -> Result<String, IbmError> {
    let num_q = circuit.num_qubits();
    let num_c = circuit.num_clbits();
    let num_instr = circuit.num_instructions();

    // Pre-allocate: header ~80 bytes + ~40 bytes per instruction avoids
    // repeated reallocations for typical circuits.
    let mut out = String::with_capacity(128 + num_instr * 40);

    // --- Header ---
    out.push_str("OPENQASM 3;\n");
    out.push_str("include \"stdgates.inc\";\n");
    writeln!(out, "qubit[{num_q}] q;").unwrap();
    if num_c > 0 {
        writeln!(out, "bit[{num_c}] c;").unwrap();
    }
    out.push('\n');

    // --- Instructions ---
    for i in 0..num_instr {
        let inst = circuit.get_instruction(i);
        emit_instruction(&mut out, &inst)?;
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn emit_instruction(out: &mut String, inst: &CircuitInstructionView) -> Result<(), IbmError> {
    match inst.kind {
        ffi::QK_OP_KIND_GATE => {
            emit_gate(out, &inst.name, &inst.qubits, &inst.params);
            Ok(())
        }
        ffi::QK_OP_KIND_MEASURE => {
            emit_measure(out, &inst.qubits, &inst.clbits);
            Ok(())
        }
        ffi::QK_OP_KIND_RESET => {
            for &q in &inst.qubits {
                writeln!(out, "reset q[{q}];").unwrap();
            }
            Ok(())
        }
        ffi::QK_OP_KIND_BARRIER => {
            emit_barrier(out, &inst.qubits);
            Ok(())
        }
        ffi::QK_OP_KIND_DELAY => {
            // Delays are scheduling hints; the IBM REST API does not accept
            // delay instructions, so we silently skip them.
            Ok(())
        }
        other => Err(IbmError::ConversionError {
            detail: format!(
                "unsupported instruction kind {} (name: \"{}\")",
                other, inst.name
            ),
        }),
    }
}

fn emit_gate(out: &mut String, name: &str, qubits: &[u32], params: &[f64]) {
    out.push_str(name);
    if !params.is_empty() {
        out.push('(');
        for (i, &p) in params.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            write_f64(out, p);
        }
        out.push(')');
    }
    out.push(' ');
    for (i, &q) in qubits.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write!(out, "q[{q}]").unwrap();
    }
    out.push_str(";\n");
}

fn emit_measure(out: &mut String, qubits: &[u32], clbits: &[u32]) {
    // OpenQASM 3 syntax: c[clbit] = measure q[qubit];
    // The Qiskit C API emits one qubit and one clbit per measure instruction,
    // but we handle the multi-qubit case defensively using zip.
    for (q, c) in qubits.iter().zip(clbits.iter()) {
        writeln!(out, "c[{c}] = measure q[{q}];").unwrap();
    }
}

fn emit_barrier(out: &mut String, qubits: &[u32]) {
    if qubits.is_empty() {
        out.push_str("barrier;\n");
    } else {
        out.push_str("barrier ");
        for (i, &q) in qubits.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            write!(out, "q[{q}]").unwrap();
        }
        out.push_str(";\n");
    }
}

/// Format an f64 with enough precision to preserve IEEE 754 double
/// semantics, then trim cosmetic trailing zeros for readability.
///
/// Guarantees at least one digit after the decimal point (e.g. `3.0`,
/// not `3.`).
///
/// IMPORTANT: formats into a temporary buffer, then appends to `out`.
/// Never trims `out` directly — doing so would corrupt previously-written
/// content (e.g. trimming `'0'` from `q[0]`).
fn write_f64(out: &mut String, v: f64) {
    // 15 significant digits preserves full f64 round-trip fidelity for
    // values in the range [0, 2*pi].
    let mut buf = format!("{v:.15}");

    // Trim trailing zeros, but keep at least one digit after the dot.
    if let Some(dot_pos) = buf.find('.') {
        let trimmed_len = buf.trim_end_matches('0').len();
        // Ensure we keep at least "X.Y" (one digit after dot).
        let min_len = dot_pos + 2;
        buf.truncate(trimmed_len.max(min_len));
    }

    out.push_str(&buf);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi;

    // -----------------------------------------------------------------------
    // Test 1: Empty circuit header
    // -----------------------------------------------------------------------

    #[test]
    fn empty_circuit_header() {
        let circ = SafeQkCircuit::new(3, 2).unwrap();
        let qasm = circuit_to_qasm3(&circ).unwrap();
        assert!(qasm.starts_with("OPENQASM 3;\n"));
        assert!(qasm.contains("include \"stdgates.inc\";"));
        assert!(qasm.contains("qubit[3] q;"));
        assert!(qasm.contains("bit[2] c;"));
        // Header lines: OPENQASM, include, qubit, bit, blank line = 5 lines
        // The trailing '\n' from push('\n') means lines() sees an empty entry
        // for that blank line, so we count it.
        let lines: Vec<&str> = qasm.lines().collect();
        assert_eq!(lines.len(), 5, "empty circuit should have header only: {qasm:?}");
    }

    // -----------------------------------------------------------------------
    // Test 2: Empty circuit, zero clbits
    // -----------------------------------------------------------------------

    #[test]
    fn empty_circuit_no_clbits() {
        let circ = SafeQkCircuit::new(2, 0).unwrap();
        let qasm = circuit_to_qasm3(&circ).unwrap();
        assert!(qasm.contains("qubit[2] q;"));
        assert!(!qasm.contains("bit["), "no bit declaration when clbits = 0");
    }

    // -----------------------------------------------------------------------
    // Test 3: Gate emission (non-parameterized)
    // -----------------------------------------------------------------------

    #[test]
    fn gate_sx_and_cx() {
        let mut circ = SafeQkCircuit::new(2, 2).unwrap();
        let q0 = [0u32];
        let q01 = [0u32, 1];
        unsafe {
            ffi::qk_circuit_gate(
                circ.as_mut_ptr(),
                ffi::QK_GATE_SX,
                q0.as_ptr(),
                std::ptr::null(),
            );
            ffi::qk_circuit_gate(
                circ.as_mut_ptr(),
                ffi::QK_GATE_CX,
                q01.as_ptr(),
                std::ptr::null(),
            );
        }
        let qasm = circuit_to_qasm3(&circ).unwrap();
        assert!(qasm.contains("sx q[0];\n"), "missing sx: {qasm:?}");
        assert!(qasm.contains("cx q[0], q[1];\n"), "missing cx: {qasm:?}");
    }

    // -----------------------------------------------------------------------
    // Test 4: Gate emission (parameterized)
    // -----------------------------------------------------------------------

    #[test]
    fn gate_rz_parameterized() {
        let mut circ = SafeQkCircuit::new(1, 0).unwrap();
        let q0 = [0u32];
        let angle = std::f64::consts::FRAC_PI_2; // 1.5707963267948966
        let params = [angle];
        unsafe {
            ffi::qk_circuit_gate(
                circ.as_mut_ptr(),
                ffi::QK_GATE_RZ,
                q0.as_ptr(),
                params.as_ptr(),
            );
        }
        let qasm = circuit_to_qasm3(&circ).unwrap();

        // Must contain the gate with parenthesized parameter
        assert!(qasm.contains("rz("), "missing rz: {qasm:?}");
        assert!(qasm.contains(") q[0];"), "missing closing paren: {qasm:?}");

        // Extract the parameter value and verify round-trip precision
        let rz_line = qasm.lines().find(|l| l.starts_with("rz(")).unwrap();
        let param_str = &rz_line[3..rz_line.find(')').unwrap()];
        let recovered: f64 = param_str.parse().unwrap();
        assert!(
            (recovered - angle).abs() < 1e-12,
            "parameter round-trip failed: emitted {param_str}, parsed {recovered}, expected {angle}"
        );
    }

    // -----------------------------------------------------------------------
    // Test 5: Measure emission
    // -----------------------------------------------------------------------

    #[test]
    fn measure_emission() {
        let mut circ = SafeQkCircuit::new(2, 2).unwrap();
        unsafe {
            ffi::qk_circuit_measure(circ.as_mut_ptr(), 0, 0);
            ffi::qk_circuit_measure(circ.as_mut_ptr(), 1, 1);
        }
        let qasm = circuit_to_qasm3(&circ).unwrap();
        assert!(
            qasm.contains("c[0] = measure q[0];\n"),
            "missing measure q[0]: {qasm:?}"
        );
        assert!(
            qasm.contains("c[1] = measure q[1];\n"),
            "missing measure q[1]: {qasm:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: Reset emission
    // -----------------------------------------------------------------------

    #[test]
    fn reset_emission() {
        let mut circ = SafeQkCircuit::new(1, 0).unwrap();
        unsafe {
            ffi::qk_circuit_reset(circ.as_mut_ptr(), 0);
        }
        let qasm = circuit_to_qasm3(&circ).unwrap();
        assert!(qasm.contains("reset q[0];\n"), "missing reset: {qasm:?}");
    }

    // -----------------------------------------------------------------------
    // Test 7: Barrier emission
    // -----------------------------------------------------------------------

    #[test]
    fn barrier_emission() {
        let mut circ = SafeQkCircuit::new(3, 0).unwrap();
        let qs = [0u32, 2];
        unsafe {
            ffi::qk_circuit_barrier(circ.as_mut_ptr(), qs.as_ptr(), 2);
        }
        let qasm = circuit_to_qasm3(&circ).unwrap();
        assert!(
            qasm.contains("barrier q[0], q[2];\n"),
            "missing barrier: {qasm:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Test 8: write_f64 precision and trimming
    // -----------------------------------------------------------------------

    #[test]
    fn write_f64_precision_and_trimming() {
        fn fmt(v: f64) -> String {
            let mut s = String::new();
            write_f64(&mut s, v);
            s
        }

        // Integer-like values keep one digit after dot
        assert_eq!(fmt(3.0), "3.0");
        assert_eq!(fmt(0.0), "0.0");

        // Trailing zeros trimmed
        assert_eq!(fmt(1.5), "1.5");

        // Full precision for pi/2 round-trips correctly
        let s = fmt(std::f64::consts::FRAC_PI_2);
        let recovered: f64 = s.parse().unwrap();
        assert!(
            (recovered - std::f64::consts::FRAC_PI_2).abs() < 1e-14,
            "pi/2 round-trip failed: emitted {s}"
        );

        // Negative value
        assert_eq!(fmt(-0.25), "-0.25");
    }

    // -----------------------------------------------------------------------
    // Test 9: write_f64 does NOT corrupt previously-written content
    //
    // This is the regression test for the critical bug described in the spec:
    // if write_f64 trimmed the entire `out` buffer instead of a temporary
    // string, any trailing '0' in content written before the float (e.g.
    // the index in `q[0]`) would be corrupted.
    // -----------------------------------------------------------------------

    #[test]
    fn write_f64_does_not_corrupt_prior_content() {
        let mut circ = SafeQkCircuit::new(1, 0).unwrap();
        let q0 = [0u32];
        // rz(1.0) has a trailing zero — the old bug would trim 'q[0' → 'q['
        let params = [1.0f64];
        unsafe {
            ffi::qk_circuit_gate(
                circ.as_mut_ptr(),
                ffi::QK_GATE_RZ,
                q0.as_ptr(),
                params.as_ptr(),
            );
        }
        let qasm = circuit_to_qasm3(&circ).unwrap();
        // q[0] must be intact — the old bug would produce q[]
        assert!(
            qasm.contains("q[0]"),
            "q[0] was corrupted by float trimming: {qasm:?}"
        );
        assert!(
            qasm.contains("rz(1.0) q[0];\n"),
            "rz(1.0) line malformed: {qasm:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Test 10: Barrier-only circuit (edge case)
    // -----------------------------------------------------------------------

    #[test]
    fn barrier_only_circuit() {
        let mut circ = SafeQkCircuit::new(2, 0).unwrap();
        let qs = [0u32, 1];
        unsafe {
            ffi::qk_circuit_barrier(circ.as_mut_ptr(), qs.as_ptr(), 2);
        }
        let qasm = circuit_to_qasm3(&circ).unwrap();
        assert!(
            qasm.contains("barrier q[0], q[1];\n"),
            "missing barrier: {qasm:?}"
        );
        assert!(!qasm.contains("bit["), "barrier-only circuit has no clbits");
    }

    // -----------------------------------------------------------------------
    // Test 11: Round-trip transpiled circuit (integration test)
    //
    // Builds a Bell state circuit, transpiles it to the IBM basis, emits QASM,
    // and validates the output structure.
    // -----------------------------------------------------------------------

    #[test]
    fn round_trip_transpiled_bell_state() {
        let mut circ = SafeQkCircuit::new(2, 2).unwrap();
        let q0 = [0u32];
        let q01 = [0u32, 1];
        unsafe {
            ffi::qk_circuit_gate(
                circ.as_mut_ptr(),
                ffi::QK_GATE_H,
                q0.as_ptr(),
                std::ptr::null(),
            );
            ffi::qk_circuit_gate(
                circ.as_mut_ptr(),
                ffi::QK_GATE_CX,
                q01.as_ptr(),
                std::ptr::null(),
            );
            ffi::qk_circuit_measure(circ.as_mut_ptr(), 0, 0);
            ffi::qk_circuit_measure(circ.as_mut_ptr(), 1, 1);
        }

        // Transpile with deterministic seed
        let output = crate::transpile::transpile_for_ibm(&circ, 2, 1, Some(42)).unwrap();
        let qasm = circuit_to_qasm3(&output.circuit).unwrap();

        // Structural validation
        assert!(qasm.starts_with("OPENQASM 3;"), "wrong header: {qasm:?}");
        assert!(
            qasm.contains("include \"stdgates.inc\";"),
            "missing include: {qasm:?}"
        );
        assert!(qasm.contains("qubit["), "missing qubit decl: {qasm:?}");
        assert!(qasm.contains("measure"), "missing measure: {qasm:?}");

        // Every non-empty body line must end with a semicolon
        let body_lines: Vec<&str> = qasm
            .lines()
            .skip_while(|l| !l.is_empty()) // skip header declarations
            .skip(1) // skip blank separator line
            .filter(|l| !l.is_empty())
            .collect();

        for line in &body_lines {
            let trimmed = line.trim();
            assert!(
                trimmed.ends_with(';'),
                "instruction line must end with semicolon: {trimmed:?}"
            );
        }
    }
}
