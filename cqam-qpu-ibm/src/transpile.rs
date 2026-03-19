//! IBM-specific transpilation via `qk_transpile`.
//!
//! Builds a `QkTarget` that models the IBM superconducting native gate set
//! (`{SX, X, Rz, CX, Measure, Reset}`), then calls the Qiskit transpiler.
//!
//! The target can be built with or without per-qubit calibration properties:
//!
//! - `build_ibm_target` — global target (Phase 5 behavior, backward-compatible).
//! - `build_ibm_target_with_calibration` — adds per-qubit error rates and
//!   gate durations so that the transpiler can make calibration-aware routing
//!   and optimization decisions at optimization level >= 1.
//!
//! The transpiled circuit is returned as a fresh `SafeQkCircuit`; layout
//! metadata is preserved in `TranspileOutput`.

use crate::calibration::IbmCalibrationData;
use crate::error::{check_exit_code, IbmError};
use crate::ffi::{self, QkTranspileOptions, QkTranspileResult};
use crate::safe::{SafeQkCircuit, SafeQkTarget, SafeQkTranspileLayout};
use cqam_qpu::traits::CalibrationData;

// ---------------------------------------------------------------------------
// Target builders
// ---------------------------------------------------------------------------

/// Build a global `QkTarget` for the IBM superconducting native gate set.
///
/// The target has no per-qubit instruction properties — sufficient for basis
/// translation and optimization passes that do not require calibration data.
///
/// This is the Phase 5 behavior, preserved for backward compatibility.
pub fn build_ibm_target(num_qubits: u32) -> Result<SafeQkTarget, IbmError> {
    build_ibm_target_with_calibration(num_qubits, &[], None)
}

/// Build a `QkTarget` with optional per-qubit calibration properties.
///
/// When `calibration` is `None`, the target has global instruction properties
/// only — identical to the Phase 5 `build_ibm_target` behavior.
///
/// When `calibration` is `Some`, the target is enriched with:
///
/// - Each 1-qubit gate (SX, X, Rz, Id) gets per-qubit properties:
///   `duration = cal.single_gate_time()`, `error = cal.single_gate_error(q)`.
/// - The 2-qubit gate (CX) gets per-edge properties for every edge in
///   `edges`: `duration = cal.two_gate_time()`,
///   `error = cal.two_gate_error(a, b)`.  If the calibration returns NaN
///   for an edge, a fallback error of `1e-2` is used.
/// - Measure and Reset get per-qubit properties:
///   `duration = f64::NAN` (not tracked), `error = cal.readout_error(q)`.
///
/// The Qiskit transpiler at optimization level >= 1 will use per-qubit error
/// rates to score SWAP candidates during routing; at level >= 2 it can use
/// gate durations for commutation and gate-cancellation decisions.
///
/// # Parameters
///
/// - `num_qubits`: number of physical qubits on the device.
/// - `edges`: connectivity edges as directed `(control, target)` pairs for CX.
///   Only consulted when `calibration` is `Some`; ignored otherwise.
///   Each pair represents a directional CX link; pass both `(a, b)` and
///   `(b, a)` if the device supports bidirectional CX.
/// - `calibration`: optional reference to per-qubit calibration data.
pub fn build_ibm_target_with_calibration(
    num_qubits: u32,
    edges: &[(u32, u32)],
    calibration: Option<&IbmCalibrationData>,
) -> Result<SafeQkTarget, IbmError> {
    let mut target = SafeQkTarget::new(num_qubits)
        .ok_or(IbmError::NullPointer { context: "qk_target_new" })?;

    // ----- 1-qubit gates: SX, X, Rz, Id -----
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

        if let Some(cal) = calibration {
            for q in 0..num_qubits {
                let mut qargs = [q];
                let duration = cal.single_gate_time();
                let error = cal.single_gate_error(q);
                let code = unsafe {
                    ffi::qk_target_entry_add_property(
                        entry,
                        qargs.as_mut_ptr(),
                        1,
                        duration,
                        error,
                    )
                };
                check_exit_code(code, "qk_target_entry_add_property(1q)")?;
            }
        }

        let code =
            unsafe { ffi::qk_target_add_instruction(target.as_mut_ptr(), entry) };
        check_exit_code(code, "qk_target_add_instruction")?;
    }

    // ----- 2-qubit gate: CX -----
    {
        let entry = unsafe { ffi::qk_target_entry_new(ffi::QK_GATE_CX) };
        if entry.is_null() {
            return Err(IbmError::NullPointer { context: "qk_target_entry_new(CX)" });
        }

        if let Some(cal) = calibration {
            for &(a, b) in edges {
                let mut qargs = [a, b];
                let duration = cal.two_gate_time();
                let raw_error = cal.two_gate_error(a, b);
                // NaN two-gate errors fall back to a typical CX error rather
                // than propagating NaN into the transpiler's cost model.
                let effective_error = if raw_error.is_nan() { 1e-2 } else { raw_error };
                let code = unsafe {
                    ffi::qk_target_entry_add_property(
                        entry,
                        qargs.as_mut_ptr(),
                        2,
                        duration,
                        effective_error,
                    )
                };
                check_exit_code(code, "qk_target_entry_add_property(CX)")?;
            }
        }

        let code =
            unsafe { ffi::qk_target_add_instruction(target.as_mut_ptr(), entry) };
        check_exit_code(code, "qk_target_add_instruction(CX)")?;
    }

    // ----- Measure and Reset: per-qubit readout errors -----
    for new_fn in &[
        ffi::qk_target_entry_new_measure
            as unsafe extern "C" fn() -> *mut ffi::QkTargetEntry,
        ffi::qk_target_entry_new_reset,
    ] {
        let entry = unsafe { new_fn() };
        if entry.is_null() {
            return Err(IbmError::NullPointer {
                context: "qk_target_entry_new(meas/reset)",
            });
        }

        if let Some(cal) = calibration {
            for q in 0..num_qubits {
                let mut qargs = [q];
                let error = cal.readout_error(q);
                let code = unsafe {
                    ffi::qk_target_entry_add_property(
                        entry,
                        qargs.as_mut_ptr(),
                        1,
                        f64::NAN, // measurement duration not tracked
                        error,
                    )
                };
                check_exit_code(code, "qk_target_entry_add_property(meas/reset)")?;
            }
        }

        let code =
            unsafe { ffi::qk_target_add_instruction(target.as_mut_ptr(), entry) };
        check_exit_code(code, "qk_target_add_instruction(meas/reset)")?;
    }

    Ok(target)
}

// ---------------------------------------------------------------------------
// TranspileOutput
// ---------------------------------------------------------------------------

/// Result of IBM transpilation: the optimized circuit and (optionally) layout.
pub struct TranspileOutput {
    pub circuit: SafeQkCircuit,
    pub layout: Option<SafeQkTranspileLayout>,
}

// ---------------------------------------------------------------------------
// Transpile entry points
// ---------------------------------------------------------------------------

/// Transpile `circuit` for the IBM native gate set (global target, no calibration).
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

    // Wrap outputs; circuit must be non-null on success
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

/// Transpile `circuit` using a calibration-aware target.
///
/// The Qiskit transpiler at optimization level >= 2 will use the per-qubit
/// error rates and gate durations to make routing and optimization decisions
/// that minimize expected circuit infidelity.
///
/// # Parameters
///
/// - `circuit`: the circuit to transpile (will not be mutated).
/// - `num_qubits`: number of physical qubits on the device.
/// - `edges`: connectivity edges as directed `(control, target)` pairs for
///   CX gates.  Pass both `(a, b)` and `(b, a)` for bidirectional links.
/// - `calibration`: per-qubit calibration data from the device.
/// - `optimization_level`: 0–3 (clamped by Qiskit).
/// - `seed`: optional RNG seed for reproducibility.
pub fn transpile_for_ibm_calibrated(
    circuit: &SafeQkCircuit,
    num_qubits: u32,
    edges: &[(u32, u32)],
    calibration: &IbmCalibrationData,
    optimization_level: u8,
    seed: Option<i64>,
) -> Result<TranspileOutput, IbmError> {
    let target =
        build_ibm_target_with_calibration(num_qubits, edges, Some(calibration))?;

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
        let detail = if !error_ptr.is_null() {
            let msg = unsafe { std::ffi::CStr::from_ptr(error_ptr) }
                .to_string_lossy()
                .into_owned();
            unsafe { ffi::qk_str_free(error_ptr) };
            msg
        } else {
            format!("exit code {}", code)
        };
        return Err(IbmError::TranspileError { detail });
    }

    if result.circuit.is_null() {
        return Err(IbmError::NullPointer {
            context: "qk_transpile result.circuit",
        });
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

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::IbmCalibrationData;
    use std::collections::HashMap;

    /// Verify that `build_ibm_target_with_calibration(n, &[], None)` produces
    /// the same structural outcome as `build_ibm_target(n)` — both should
    /// succeed without error for a reasonable qubit count.
    #[test]
    fn test_build_target_without_calibration() {
        let t1 = build_ibm_target(5).expect("build_ibm_target failed");
        let t2 = build_ibm_target_with_calibration(5, &[], None)
            .expect("build_ibm_target_with_calibration(None) failed");
        // Targets are opaque; success without error is the assertion.
        drop(t1);
        drop(t2);
    }

    /// Verify that building a target with synthetic calibration data succeeds.
    #[test]
    fn test_build_target_with_synthetic_calibration() {
        let cal = IbmCalibrationData::synthetic(5);
        let edges: Vec<(u32, u32)> = vec![
            (0, 1), (1, 0),
            (1, 2), (2, 1),
            (2, 3), (3, 2),
            (3, 4), (4, 3),
        ];
        let target = build_ibm_target_with_calibration(5, &edges, Some(&cal));
        assert!(
            target.is_ok(),
            "calibrated target build failed: {:?}",
            target.err()
        );
    }

    /// Verify that per-qubit properties for 1q gates do not crash with a
    /// larger (27-qubit) device qubit count.
    #[test]
    fn test_build_target_27q_calibrated() {
        let cal = IbmCalibrationData::synthetic(27);
        // Subset of heavy-hex 27-qubit edges (bidirectional)
        let edges: Vec<(u32, u32)> = vec![
            (0, 1), (1, 0), (1, 2), (2, 1), (1, 4), (4, 1),
            (2, 3), (3, 2), (3, 5), (5, 3),
            (4, 7), (7, 4), (5, 8), (8, 5),
            (6, 7), (7, 6), (7, 10), (10, 7),
        ];
        let target = build_ibm_target_with_calibration(27, &edges, Some(&cal));
        assert!(target.is_ok());
    }

    /// Verify that a NaN two-gate error is replaced by the `1e-2` fallback
    /// and does not cause an error or propagate into the target.
    #[test]
    fn test_nan_two_gate_error_fallback() {
        // Construct calibration with an empty two-gate map so every edge
        // lookup returns NaN.
        let cal = IbmCalibrationData::new(
            vec![100e-6; 3],
            vec![80e-6; 3],
            vec![1e-3; 3],
            HashMap::new(), // no two-gate errors → NaN for all edges
            vec![1e-2; 3],
            35e-9,
            660e-9,
        );
        let edges = vec![(0, 1), (1, 0), (1, 2), (2, 1)];
        // Should succeed; NaN is replaced with 1e-2 internally.
        let target = build_ibm_target_with_calibration(3, &edges, Some(&cal));
        assert!(target.is_ok());
    }
}
