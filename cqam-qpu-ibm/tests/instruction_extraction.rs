//! Integration tests for `SafeQkCircuit` instruction extraction (Task 6.5).
//!
//! These tests exercise the FFI bindings and safe wrappers added in Task 6.5:
//! `num_instructions()`, `get_instruction()`, and `instructions()`.
//!
//! They require the Qiskit C shared library to be present at build time
//! (controlled by the `QISKIT_C_DIR` environment variable).  The tests are
//! grouped by scenario:
//!
//! 1. Bell circuit  — basic gate/measure extraction
//! 2. Parameterized gate — Rz angle is recovered exactly
//! 3. Post-transpilation — gates are restricted to the IBM native gate set
//! 4. Bulk iteration — `instructions()` returns the full list
//! 5. Out-of-bounds — `get_instruction()` panics on a bad index
//! 6. Barrier and reset — non-gate operation kinds are extracted correctly

use cqam_qpu_ibm::ffi;
use cqam_qpu_ibm::safe::{CircuitInstructionView, SafeQkCircuit};
use cqam_qpu_ibm::transpile::transpile_for_ibm;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a 2-qubit, 2-clbit Bell circuit: H(0), CX(0,1), Measure(0→0), Measure(1→1).
fn build_bell_circuit() -> SafeQkCircuit {
    let mut circ = SafeQkCircuit::new(2, 2).expect("qk_circuit_new failed");

    let q0 = [0u32];
    let rc = unsafe {
        ffi::qk_circuit_gate(
            circ.as_mut_ptr(),
            ffi::QK_GATE_H,
            q0.as_ptr(),
            std::ptr::null(),
        )
    };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS, "H gate failed");

    let q01 = [0u32, 1];
    let rc = unsafe {
        ffi::qk_circuit_gate(
            circ.as_mut_ptr(),
            ffi::QK_GATE_CX,
            q01.as_ptr(),
            std::ptr::null(),
        )
    };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS, "CX gate failed");

    let rc = unsafe { ffi::qk_circuit_measure(circ.as_mut_ptr(), 0, 0) };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS, "Measure(0→0) failed");

    let rc = unsafe { ffi::qk_circuit_measure(circ.as_mut_ptr(), 1, 1) };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS, "Measure(1→1) failed");

    circ
}

// ---------------------------------------------------------------------------
// Test 1: Bell circuit instruction extraction
// ---------------------------------------------------------------------------

#[test]
fn test_instruction_extraction_bell() {
    let circ = build_bell_circuit();

    assert_eq!(circ.num_instructions(), 4);

    let inst0 = circ.get_instruction(0);
    assert_eq!(inst0.name, "h", "first instruction should be H");
    assert_eq!(inst0.qubits, vec![0u32]);
    assert!(inst0.clbits.is_empty());
    assert!(inst0.params.is_empty());
    assert_eq!(inst0.kind, ffi::QK_OP_KIND_GATE);

    let inst1 = circ.get_instruction(1);
    assert_eq!(inst1.name, "cx", "second instruction should be CX");
    assert_eq!(inst1.qubits, vec![0u32, 1]);
    assert!(inst1.clbits.is_empty());
    assert!(inst1.params.is_empty());
    assert_eq!(inst1.kind, ffi::QK_OP_KIND_GATE);

    let inst2 = circ.get_instruction(2);
    assert_eq!(inst2.kind, ffi::QK_OP_KIND_MEASURE, "third instruction should be Measure");
    assert_eq!(inst2.qubits, vec![0u32]);
    assert_eq!(inst2.clbits, vec![0u32]);

    let inst3 = circ.get_instruction(3);
    assert_eq!(inst3.kind, ffi::QK_OP_KIND_MEASURE, "fourth instruction should be Measure");
    assert_eq!(inst3.qubits, vec![1u32]);
    assert_eq!(inst3.clbits, vec![1u32]);
}

// ---------------------------------------------------------------------------
// Test 2: Parameterized gate extraction (Rz)
// ---------------------------------------------------------------------------

#[test]
fn test_instruction_extraction_parameterized() {
    let mut circ = SafeQkCircuit::new(1, 0).expect("qk_circuit_new failed");

    let q0 = [0u32];
    let angle = std::f64::consts::FRAC_PI_4;
    let params = [angle];

    let rc = unsafe {
        ffi::qk_circuit_gate(
            circ.as_mut_ptr(),
            ffi::QK_GATE_RZ,
            q0.as_ptr(),
            params.as_ptr(),
        )
    };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS, "Rz gate failed");

    assert_eq!(circ.num_instructions(), 1);

    let inst = circ.get_instruction(0);
    assert_eq!(inst.name, "rz", "instruction name should be 'rz'");
    assert_eq!(inst.qubits, vec![0u32]);
    assert!(inst.clbits.is_empty());
    assert_eq!(inst.params.len(), 1, "Rz should have exactly 1 parameter");
    assert!(
        (inst.params[0] - angle).abs() < 1e-14,
        "Rz parameter mismatch: expected {}, got {}",
        angle,
        inst.params[0]
    );
    assert_eq!(inst.kind, ffi::QK_OP_KIND_GATE);
}

// ---------------------------------------------------------------------------
// Test 3: Post-transpilation extraction
// ---------------------------------------------------------------------------

#[test]
fn test_instructions_after_transpile() {
    let mut circ = SafeQkCircuit::new(2, 0).expect("qk_circuit_new failed");

    let q0 = [0u32];
    let rc = unsafe {
        ffi::qk_circuit_gate(
            circ.as_mut_ptr(),
            ffi::QK_GATE_H,
            q0.as_ptr(),
            std::ptr::null(),
        )
    };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS);

    let q01 = [0u32, 1];
    let rc = unsafe {
        ffi::qk_circuit_gate(
            circ.as_mut_ptr(),
            ffi::QK_GATE_CX,
            q01.as_ptr(),
            std::ptr::null(),
        )
    };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS);

    let output = transpile_for_ibm(&circ, 2, 1, Some(42))
        .expect("transpile_for_ibm failed");
    let instructions = output.circuit.instructions();

    assert!(
        !instructions.is_empty(),
        "transpiled circuit should have at least one instruction"
    );

    // IBM native gate set: SX, X, Rz, CX, and occasionally I (identity).
    // The transpiler should not emit any gates outside this basis.
    let allowed_gates = ["sx", "x", "rz", "cx", "id", "measure", "reset", "barrier"];
    for inst in &instructions {
        if inst.kind == ffi::QK_OP_KIND_GATE {
            assert!(
                allowed_gates.contains(&inst.name.as_str()),
                "unexpected gate after IBM transpilation: '{}'",
                inst.name
            );
        }
    }

    // H decomposes to at least Rz + SX + Rz; expect more than 2 total instructions.
    assert!(
        instructions.len() >= 2,
        "transpiled Bell prep should have at least 2 instructions, got {}",
        instructions.len()
    );
}

// ---------------------------------------------------------------------------
// Test 4: Bulk iteration via instructions()
// ---------------------------------------------------------------------------

#[test]
fn test_instructions_bulk() {
    let mut circ = SafeQkCircuit::new(2, 0).expect("qk_circuit_new failed");

    let q0 = [0u32];
    unsafe {
        ffi::qk_circuit_gate(
            circ.as_mut_ptr(),
            ffi::QK_GATE_H,
            q0.as_ptr(),
            std::ptr::null(),
        )
    };

    let q01 = [0u32, 1];
    unsafe {
        ffi::qk_circuit_gate(
            circ.as_mut_ptr(),
            ffi::QK_GATE_CX,
            q01.as_ptr(),
            std::ptr::null(),
        )
    };

    let all: Vec<CircuitInstructionView> = circ.instructions();
    assert_eq!(all.len(), 2, "circuit should have 2 instructions");
    assert_eq!(all[0].name, "h");
    assert_eq!(all[1].name, "cx");
}

// ---------------------------------------------------------------------------
// Test 5: Out-of-bounds get_instruction panics
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "out of bounds")]
fn test_get_instruction_out_of_bounds() {
    let circ = SafeQkCircuit::new(1, 0).expect("qk_circuit_new failed");
    assert_eq!(circ.num_instructions(), 0, "fresh circuit should have 0 instructions");
    // This must panic.
    let _ = circ.get_instruction(0);
}

// ---------------------------------------------------------------------------
// Test 6: Barrier and reset extraction
// ---------------------------------------------------------------------------

#[test]
fn test_instruction_extraction_barrier_reset() {
    let mut circ = SafeQkCircuit::new(2, 0).expect("qk_circuit_new failed");

    // Reset qubit 0.
    let rc = unsafe { ffi::qk_circuit_reset(circ.as_mut_ptr(), 0) };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS, "Reset failed");

    // Barrier on both qubits.
    let qubits = [0u32, 1];
    let rc = unsafe { ffi::qk_circuit_barrier(circ.as_mut_ptr(), qubits.as_ptr(), 2) };
    assert_eq!(rc, ffi::QK_EXIT_SUCCESS, "Barrier failed");

    assert_eq!(circ.num_instructions(), 2);

    let inst0 = circ.get_instruction(0);
    assert_eq!(
        inst0.kind,
        ffi::QK_OP_KIND_RESET,
        "first instruction should be Reset"
    );
    assert_eq!(inst0.qubits, vec![0u32]);

    let inst1 = circ.get_instruction(1);
    assert_eq!(
        inst1.kind,
        ffi::QK_OP_KIND_BARRIER,
        "second instruction should be Barrier"
    );
    assert_eq!(inst1.qubits, vec![0u32, 1]);
}
