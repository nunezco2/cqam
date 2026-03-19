//! RAII wrappers around Qiskit C API opaque types.
//!
//! Each wrapper owns the underlying pointer and frees it on `Drop`.
//! None of the wrappers are `Clone` by default because the C API requires
//! explicit copies; use `SafeQkCircuit::copy` where needed.

use crate::ffi;

// ---------------------------------------------------------------------------
// CircuitInstructionView
// ---------------------------------------------------------------------------

/// A safe, owned view of a single circuit instruction extracted from a `QkCircuit`.
///
/// Not lifetime-tied to the circuit because the C API copies its data into the
/// `QkCircuitInstruction` struct, and we copy it again into owned Rust types
/// before calling `qk_circuit_instruction_clear`.
pub struct CircuitInstructionView {
    /// Operation name (e.g. `"h"`, `"cx"`, `"rz"`, `"measure"`, `"reset"`, `"barrier"`).
    pub name: String,
    /// Qubit indices this instruction acts on.
    pub qubits: Vec<u32>,
    /// Classical bit indices (non-empty for `measure` instructions).
    pub clbits: Vec<u32>,
    /// Parameter values (e.g. rotation angles for `rz`, `rx`, `u`, …).
    pub params: Vec<f64>,
    /// Operation kind discriminator (one of the `QK_OP_KIND_*` constants).
    pub kind: ffi::QkOperationKind,
}

// ---------------------------------------------------------------------------
// SafeQkCircuit
// ---------------------------------------------------------------------------

/// Owned handle to a `QkCircuit`.
pub struct SafeQkCircuit {
    ptr: *mut ffi::QkCircuit,
}

// SAFETY: QkCircuit is an opaque heap allocation; single-owner access is safe
// to send between threads as long as we never alias.
unsafe impl Send for SafeQkCircuit {}

impl SafeQkCircuit {
    /// Allocate a new circuit with the given qubit / clbit counts.
    pub fn new(num_qubits: u32, num_clbits: u32) -> Option<Self> {
        let ptr = unsafe { ffi::qk_circuit_new(num_qubits, num_clbits) };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Return a raw const pointer (for read-only C calls).
    pub fn as_ptr(&self) -> *const ffi::QkCircuit {
        self.ptr
    }

    /// Return a raw mutable pointer (for mutation C calls).
    pub fn as_mut_ptr(&mut self) -> *mut ffi::QkCircuit {
        self.ptr
    }

    /// Deep-copy the circuit via `qk_circuit_copy`.
    pub fn copy(&self) -> Option<Self> {
        let ptr = unsafe { ffi::qk_circuit_copy(self.ptr) };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Number of qubits this circuit was allocated for.
    pub fn num_qubits(&self) -> u32 {
        unsafe { ffi::qk_circuit_num_qubits(self.ptr) }
    }

    /// Number of classical bits this circuit was allocated for.
    pub fn num_clbits(&self) -> u32 {
        unsafe { ffi::qk_circuit_num_clbits(self.ptr) }
    }

    /// Wrap an already-allocated pointer (takes ownership).
    ///
    /// # Safety
    /// `ptr` must be a valid, non-null pointer returned by the Qiskit C API
    /// that has not yet been freed and is not aliased elsewhere.
    pub unsafe fn from_raw(ptr: *mut ffi::QkCircuit) -> Self {
        debug_assert!(!ptr.is_null());
        Self { ptr }
    }

    /// Number of instructions in this circuit.
    pub fn num_instructions(&self) -> usize {
        unsafe { ffi::qk_circuit_num_instructions(self.ptr) }
    }

    /// Extract the instruction at `index` as a safe, owned Rust struct.
    ///
    /// All C-allocated buffers (name, qubits, clbits, params) are copied into
    /// owned Rust types and the C allocations are freed via
    /// `qk_circuit_instruction_clear` before returning.
    ///
    /// # Panics
    /// Panics if `index >= self.num_instructions()`.
    pub fn get_instruction(&self, index: usize) -> CircuitInstructionView {
        assert!(
            index < self.num_instructions(),
            "instruction index {} out of bounds (circuit has {} instructions)",
            index,
            self.num_instructions(),
        );

        let kind = unsafe { ffi::qk_circuit_instruction_kind(self.ptr, index) };

        let mut raw = std::mem::MaybeUninit::<ffi::QkCircuitInstruction>::uninit();
        unsafe {
            ffi::qk_circuit_get_instruction(self.ptr, index, raw.as_mut_ptr());
        }
        let mut raw = unsafe { raw.assume_init() };

        // Copy all data into owned Rust types before freeing C allocations.
        let name = if raw.name.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(raw.name) }
                .to_string_lossy()
                .into_owned()
        };

        let qubits = if raw.qubits.is_null() || raw.num_qubits == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(raw.qubits, raw.num_qubits as usize) }.to_vec()
        };

        let clbits = if raw.clbits.is_null() || raw.num_clbits == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(raw.clbits, raw.num_clbits as usize) }.to_vec()
        };

        let params = if raw.params.is_null() || raw.num_params == 0 {
            Vec::new()
        } else {
            // params is a *mut *mut QkParam — an array of opaque parameter
            // pointers.  Extract each concrete f64 via qk_param_as_real.
            let param_ptrs = unsafe {
                std::slice::from_raw_parts(raw.params, raw.num_params as usize)
            };
            param_ptrs
                .iter()
                .map(|&p| unsafe { ffi::qk_param_as_real(p as *const ffi::QkParam) })
                .collect()
        };

        // Free C-allocated internals now that everything has been copied.
        unsafe { ffi::qk_circuit_instruction_clear(&mut raw) };

        CircuitInstructionView {
            name,
            qubits,
            clbits,
            params,
            kind,
        }
    }

    /// Return all instructions as a `Vec` of safe, owned views.
    ///
    /// Equivalent to calling `get_instruction(i)` for each `i` in
    /// `0..self.num_instructions()`.
    pub fn instructions(&self) -> Vec<CircuitInstructionView> {
        (0..self.num_instructions())
            .map(|i| self.get_instruction(i))
            .collect()
    }
}

impl Drop for SafeQkCircuit {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { ffi::qk_circuit_free(self.ptr) };
            self.ptr = std::ptr::null_mut();
        }
    }
}

// ---------------------------------------------------------------------------
// SafeQkTarget
// ---------------------------------------------------------------------------

/// Owned handle to a `QkTarget`.
pub struct SafeQkTarget {
    ptr: *mut ffi::QkTarget,
}

unsafe impl Send for SafeQkTarget {}

impl SafeQkTarget {
    /// Allocate a new target for a device with `num_qubits` physical qubits.
    pub fn new(num_qubits: u32) -> Option<Self> {
        let ptr = unsafe { ffi::qk_target_new(num_qubits) };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    pub fn as_ptr(&self) -> *const ffi::QkTarget {
        self.ptr
    }

    pub fn as_mut_ptr(&mut self) -> *mut ffi::QkTarget {
        self.ptr
    }
}

impl Drop for SafeQkTarget {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { ffi::qk_target_free(self.ptr) };
            self.ptr = std::ptr::null_mut();
        }
    }
}

// ---------------------------------------------------------------------------
// SafeQkTranspileLayout
// ---------------------------------------------------------------------------

/// Owned handle to a `QkTranspileLayout` produced by `qk_transpile`.
pub struct SafeQkTranspileLayout {
    ptr: *mut ffi::QkTranspileLayout,
}

unsafe impl Send for SafeQkTranspileLayout {}

impl SafeQkTranspileLayout {
    /// Wrap an already-allocated layout pointer (takes ownership).
    ///
    /// # Safety
    /// `ptr` must be a valid, non-null pointer returned by `qk_transpile`.
    pub unsafe fn from_raw(ptr: *mut ffi::QkTranspileLayout) -> Self {
        debug_assert!(!ptr.is_null());
        Self { ptr }
    }
}

impl Drop for SafeQkTranspileLayout {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { ffi::qk_transpile_layout_free(self.ptr) };
            self.ptr = std::ptr::null_mut();
        }
    }
}
