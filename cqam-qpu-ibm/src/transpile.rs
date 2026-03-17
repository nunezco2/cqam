//! IBM-specific transpilation via `qk_transpile`.
//!
//! Builds a minimal `QkTarget` that models the IBM superconducting native gate
//! set (`{SX, X, Rz, CX, Measure, Reset}`), then calls the Qiskit transpiler.
//!
//! The transpiled circuit is returned as a fresh `SafeQkCircuit`; the layout
//! metadata is dropped (callers that need it can extend this module).

use crate::error::{check_exit_code, IbmError};
use crate::ffi::{self, QkTranspileOptions, QkTranspileResult};
use crate::safe::{SafeQkCircuit, SafeQkTarget, SafeQkTranspileLayout};

/// Build a `QkTarget` for the IBM superconducting native gate set.
///
/// The target is global (no per-qubit instruction properties) -- sufficient
/// for basis translation and optimization passes.
pub fn build_ibm_target(num_qubits: u32) -> Result<SafeQkTarget, IbmError> {
    let mut target = SafeQkTarget::new(num_qubits)
        .ok_or(IbmError::NullPointer { context: "qk_target_new" })?;

    // Native 1-qubit gates
    for &gate in &[
        ffi::QK_GATE_SX,
        ffi::QK_GATE_X,
        ffi::QK_GATE_RZ,
        ffi::QK_GATE_I,
    ] {
        let entry = unsafe { ffi::qk_target_entry_new(gate) };
        if entry.is_null() {
            return Err(IbmError::NullPointer { context: "qk_target_entry_new" });
        }
        let code = unsafe { ffi::qk_target_add_instruction(target.as_mut_ptr(), entry) };
        check_exit_code(code, "qk_target_add_instruction")?;
    }

    // Native 2-qubit gate
    {
        let entry = unsafe { ffi::qk_target_entry_new(ffi::QK_GATE_CX) };
        if entry.is_null() {
            return Err(IbmError::NullPointer { context: "qk_target_entry_new(CX)" });
        }
        let code = unsafe { ffi::qk_target_add_instruction(target.as_mut_ptr(), entry) };
        check_exit_code(code, "qk_target_add_instruction(CX)")?;
    }

    // Measure and Reset
    {
        let entry = unsafe { ffi::qk_target_entry_new_measure() };
        if entry.is_null() {
            return Err(IbmError::NullPointer { context: "qk_target_entry_new_measure" });
        }
        let code = unsafe { ffi::qk_target_add_instruction(target.as_mut_ptr(), entry) };
        check_exit_code(code, "qk_target_add_instruction(Measure)")?;
    }
    {
        let entry = unsafe { ffi::qk_target_entry_new_reset() };
        if entry.is_null() {
            return Err(IbmError::NullPointer { context: "qk_target_entry_new_reset" });
        }
        let code = unsafe { ffi::qk_target_add_instruction(target.as_mut_ptr(), entry) };
        check_exit_code(code, "qk_target_add_instruction(Reset)")?;
    }

    Ok(target)
}

/// Result of IBM transpilation: the optimized circuit and (optionally) layout.
pub struct TranspileOutput {
    pub circuit: SafeQkCircuit,
    pub layout: Option<SafeQkTranspileLayout>,
}

/// Transpile `circuit` for the IBM native gate set.
///
/// `optimization_level` is clamped to 0–3 by the Qiskit transpiler.
/// Pass `seed = None` to let the transpiler choose its own seed.
pub fn transpile_for_ibm(
    circuit: &SafeQkCircuit,
    num_qubits: u32,
    optimization_level: u8,
    seed: Option<i64>,
) -> Result<TranspileOutput, IbmError> {
    let target = build_ibm_target(num_qubits)?;

    let options = QkTranspileOptions {
        optimization_level,
        seed: seed.unwrap_or(-1),
        approximation_degree: 1.0,
    };

    let mut result = QkTranspileResult::default();
    let mut error_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

    let code = unsafe {
        ffi::qk_transpile(
            circuit.as_ptr(),
            target.as_ptr(),
            &options as *const QkTranspileOptions,
            &mut result as *mut QkTranspileResult,
            &mut error_ptr as *mut *mut std::os::raw::c_char,
        )
    };

    if code != ffi::QK_EXIT_SUCCESS {
        // Collect the error message if one was set
        let detail = if !error_ptr.is_null() {
            let msg = unsafe { std::ffi::CStr::from_ptr(error_ptr) }
                .to_string_lossy()
                .into_owned();
            // Free the error string using qk_str_free (NOT libc free --
            // the Qiskit C API documents that its strings must be freed
            // with qk_str_free, not the system allocator's free).
            unsafe { ffi::qk_str_free(error_ptr) };
            msg
        } else {
            format!("exit code {}", code)
        };
        return Err(IbmError::TranspileError { detail });
    }

    // Wrap outputs; both must be non-null on success
    if result.circuit.is_null() {
        return Err(IbmError::NullPointer { context: "qk_transpile result.circuit" });
    }
    let transpiled_circuit = unsafe { SafeQkCircuit::from_raw(result.circuit) };

    let layout = if !result.layout.is_null() {
        Some(unsafe { SafeQkTranspileLayout::from_raw(result.layout) })
    } else {
        None
    };

    Ok(TranspileOutput {
        circuit: transpiled_circuit,
        layout,
    })
}

