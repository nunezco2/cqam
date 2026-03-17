//! Raw FFI bindings to the Qiskit C API.
//!
//! These match the actual headers in `/tmp/qiskit/dist/c/include/qiskit/`.
//! All types are transcribed faithfully from `types.h` and `funcs.h`.

use std::os::raw::{c_char, c_double};

// ---------------------------------------------------------------------------
// Opaque types
// ---------------------------------------------------------------------------

/// Opaque circuit handle.
#[repr(C)]
pub struct QkCircuit {
    _opaque: [u8; 0],
}

/// Opaque transpile layout handle.
#[repr(C)]
pub struct QkTranspileLayout {
    _opaque: [u8; 0],
}

/// Opaque target handle.
#[repr(C)]
pub struct QkTarget {
    _opaque: [u8; 0],
}

/// Opaque target entry handle.
#[repr(C)]
pub struct QkTargetEntry {
    _opaque: [u8; 0],
}

// ---------------------------------------------------------------------------
// QkExitCode  (u32)
// ---------------------------------------------------------------------------

pub type QkExitCode = u32;

pub const QK_EXIT_SUCCESS: QkExitCode = 0;
pub const QK_EXIT_C_INPUT_ERROR: QkExitCode = 100;
pub const QK_EXIT_NULL_POINTER_ERROR: QkExitCode = 101;
pub const QK_EXIT_ALIGNMENT_ERROR: QkExitCode = 102;
pub const QK_EXIT_INDEX_ERROR: QkExitCode = 103;
pub const QK_EXIT_DUPLICATE_INDEX_ERROR: QkExitCode = 104;
pub const QK_EXIT_INVALID_OPERATION_KIND: QkExitCode = 105;
pub const QK_EXIT_ARITHMETIC_ERROR: QkExitCode = 200;
pub const QK_EXIT_MISMATCHED_QUBITS: QkExitCode = 201;
pub const QK_EXIT_EXPECTED_UNITARY: QkExitCode = 202;
pub const QK_EXIT_TARGET_ERROR: QkExitCode = 300;
pub const QK_EXIT_TRANSPILER_ERROR: QkExitCode = 400;

// ---------------------------------------------------------------------------
// QkGate  (u8)
// ---------------------------------------------------------------------------

pub type QkGate = u8;

pub const QK_GATE_GLOBAL_PHASE: QkGate = 0;
pub const QK_GATE_H: QkGate = 1;
pub const QK_GATE_I: QkGate = 2;
pub const QK_GATE_X: QkGate = 3;
pub const QK_GATE_Y: QkGate = 4;
pub const QK_GATE_Z: QkGate = 5;
pub const QK_GATE_PHASE: QkGate = 6;
pub const QK_GATE_R: QkGate = 7;
pub const QK_GATE_RX: QkGate = 8;
pub const QK_GATE_RY: QkGate = 9;
pub const QK_GATE_RZ: QkGate = 10;
pub const QK_GATE_S: QkGate = 11;
pub const QK_GATE_SDG: QkGate = 12;
pub const QK_GATE_SX: QkGate = 13;
pub const QK_GATE_SXDG: QkGate = 14;
pub const QK_GATE_T: QkGate = 15;
pub const QK_GATE_TDG: QkGate = 16;
pub const QK_GATE_U: QkGate = 17;
pub const QK_GATE_U1: QkGate = 18;
pub const QK_GATE_U2: QkGate = 19;
pub const QK_GATE_U3: QkGate = 20;
pub const QK_GATE_CX: QkGate = 22;

// ---------------------------------------------------------------------------
// QkTranspileOptions
// ---------------------------------------------------------------------------

/// Options for `qk_transpile`.  Fields match the header exactly.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct QkTranspileOptions {
    /// Optimization level: 0–3.
    pub optimization_level: u8,
    /// Seed for the transpiler RNGs.  Negative → system entropy.
    pub seed: i64,
    /// Approximation degree: 1.0 = no approximation, NAN = use target error rate.
    pub approximation_degree: c_double,
}

impl Default for QkTranspileOptions {
    fn default() -> Self {
        Self {
            optimization_level: 1,
            seed: -1,
            approximation_degree: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// QkTranspileResult
// ---------------------------------------------------------------------------

/// Output container written to by `qk_transpile`.
#[repr(C)]
pub struct QkTranspileResult {
    /// Compiled circuit (caller must free with `qk_circuit_free`).
    pub circuit: *mut QkCircuit,
    /// Layout metadata (caller must free with `qk_transpile_layout_free`).
    pub layout: *mut QkTranspileLayout,
}

impl Default for QkTranspileResult {
    fn default() -> Self {
        Self {
            circuit: std::ptr::null_mut(),
            layout: std::ptr::null_mut(),
        }
    }
}

// ---------------------------------------------------------------------------
// Extern declarations
// ---------------------------------------------------------------------------

extern "C" {
    // -- Circuit lifecycle --
    pub fn qk_circuit_new(num_qubits: u32, num_clbits: u32) -> *mut QkCircuit;
    pub fn qk_circuit_free(circuit: *mut QkCircuit);
    pub fn qk_circuit_copy(circuit: *const QkCircuit) -> *mut QkCircuit;
    pub fn qk_circuit_num_qubits(circuit: *const QkCircuit) -> u32;
    pub fn qk_circuit_num_clbits(circuit: *const QkCircuit) -> u32;

    // -- Circuit instruction builders --
    pub fn qk_circuit_gate(
        circuit: *mut QkCircuit,
        gate: QkGate,
        qubits: *const u32,
        params: *const c_double,
    ) -> QkExitCode;

    pub fn qk_circuit_measure(
        circuit: *mut QkCircuit,
        qubit: u32,
        clbit: u32,
    ) -> QkExitCode;

    pub fn qk_circuit_reset(circuit: *mut QkCircuit, qubit: u32) -> QkExitCode;

    pub fn qk_circuit_barrier(
        circuit: *mut QkCircuit,
        qubits: *const u32,
        num_qubits: u32,
    ) -> QkExitCode;

    // -- Target --
    pub fn qk_target_new(num_qubits: u32) -> *mut QkTarget;
    pub fn qk_target_free(target: *mut QkTarget);
    pub fn qk_target_entry_new(operation: QkGate) -> *mut QkTargetEntry;
    pub fn qk_target_entry_new_measure() -> *mut QkTargetEntry;
    pub fn qk_target_entry_new_reset() -> *mut QkTargetEntry;
    pub fn qk_target_add_instruction(
        target: *mut QkTarget,
        target_entry: *mut QkTargetEntry,
    ) -> QkExitCode;

    // -- Transpiler --
    pub fn qk_transpile(
        qc: *const QkCircuit,
        target: *const QkTarget,
        options: *const QkTranspileOptions,
        result: *mut QkTranspileResult,
        error: *mut *mut c_char,
    ) -> QkExitCode;

    pub fn qk_transpile_layout_free(layout: *mut QkTranspileLayout);

    // -- String free --
    /// Free a string allocated by the Qiskit C API (e.g. transpiler error messages).
    /// Must be used instead of libc `free` for Qiskit-allocated strings.
    pub fn qk_str_free(string: *mut c_char);
}
