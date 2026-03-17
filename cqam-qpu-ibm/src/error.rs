//! Error types for the IBM QPU backend.

use std::fmt;

use crate::ffi::QkExitCode;

/// Errors produced by the `cqam-qpu-ibm` crate.
#[derive(Debug)]
pub enum IbmError {
    /// A Qiskit C API call returned a non-zero exit code.
    QiskitFfi {
        function: &'static str,
        code: QkExitCode,
    },
    /// The Qiskit C API returned a null pointer where an allocation was expected.
    NullPointer { context: &'static str },
    /// Circuit conversion failed (unsupported gate or IR structure).
    ConversionError { detail: String },
    /// IBM Quantum REST API error.
    RestError { detail: String },
    /// HTTP transport error (wraps reqwest::Error).
    HttpError(reqwest::Error),
    /// Job did not complete within the polling timeout.
    Timeout { job_id: String, elapsed_secs: u64 },
    /// The backend reported an unexpected job status.
    UnexpectedStatus { job_id: String, status: String },
    /// Calibration fetch or parse error.
    CalibrationError { detail: String },
    /// Transpilation failed (error message from Qiskit).
    TranspileError { detail: String },
}

impl fmt::Display for IbmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IbmError::QiskitFfi { function, code } =>
                write!(f, "Qiskit FFI error in {}: exit code {}", function, code),
            IbmError::NullPointer { context } =>
                write!(f, "Qiskit FFI returned null pointer: {}", context),
            IbmError::ConversionError { detail } =>
                write!(f, "Circuit conversion error: {}", detail),
            IbmError::RestError { detail } =>
                write!(f, "IBM Quantum REST error: {}", detail),
            IbmError::HttpError(e) =>
                write!(f, "HTTP error: {}", e),
            IbmError::Timeout { job_id, elapsed_secs } =>
                write!(f, "Job {} timed out after {}s", job_id, elapsed_secs),
            IbmError::UnexpectedStatus { job_id, status } =>
                write!(f, "Job {} has unexpected status: {}", job_id, status),
            IbmError::CalibrationError { detail } =>
                write!(f, "Calibration error: {}", detail),
            IbmError::TranspileError { detail } =>
                write!(f, "Transpilation failed: {}", detail),
        }
    }
}

impl std::error::Error for IbmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IbmError::HttpError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for IbmError {
    fn from(e: reqwest::Error) -> Self {
        IbmError::HttpError(e)
    }
}

impl From<IbmError> for cqam_core::error::CqamError {
    fn from(e: IbmError) -> Self {
        match &e {
            IbmError::RestError { detail } | IbmError::TranspileError { detail } =>
                cqam_core::error::CqamError::QpuSubmissionFailed {
                    provider: "IBM".to_string(),
                    detail: detail.clone(),
                },
            IbmError::Timeout { job_id, .. } =>
                cqam_core::error::CqamError::QpuSubmissionFailed {
                    provider: "IBM".to_string(),
                    detail: format!("timeout waiting for job {}", job_id),
                },
            _ =>
                cqam_core::error::CqamError::QpuSubmissionFailed {
                    provider: "IBM".to_string(),
                    detail: e.to_string(),
                },
        }
    }
}

/// Convert a `QkExitCode` to `Result<(), IbmError>`.
pub fn check_exit_code(code: QkExitCode, function: &'static str) -> Result<(), IbmError> {
    if code == crate::ffi::QK_EXIT_SUCCESS {
        Ok(())
    } else {
        Err(IbmError::QiskitFfi { function, code })
    }
}
