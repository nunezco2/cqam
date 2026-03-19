//! IBM QPU backend for CQAM.
//!
//! Provides circuit construction and IBM-specific transpilation via the Qiskit C API (FFI),
//! and job submission/polling via the IBM Quantum Platform REST API.

pub mod backend;
pub mod calibration;
pub mod convert;
pub mod error;
pub mod ffi;
pub mod qasm;
pub mod rest;
pub mod safe;
pub mod transpile;

pub use backend::IbmQpuBackend;
pub use error::IbmError;
pub use transpile::{
    build_ibm_target,
    build_ibm_target_with_calibration,
    transpile_for_ibm,
    transpile_for_ibm_calibrated,
};
