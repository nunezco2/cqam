//! RAII wrappers around Qiskit C API opaque types.
//!
//! Each wrapper owns the underlying pointer and frees it on `Drop`.
//! None of the wrappers are `Clone` by default because the C API requires
//! explicit copies; use `SafeQkCircuit::copy` where needed.

use crate::ffi;

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
