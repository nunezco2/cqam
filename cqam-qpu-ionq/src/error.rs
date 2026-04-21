//! Error types for the IonQ QPU backend.

use std::fmt;

/// Errors produced by the `cqam-qpu-ionq` crate.
#[derive(Debug)]
pub enum IonQError {
    /// Circuit conversion failed (unsupported gate or IR structure).
    ConversionError { detail: String },
    /// IonQ REST API returned an error response.
    RestError { detail: String },
    /// HTTP transport error (wraps reqwest::Error).
    HttpError(reqwest::Error),
    /// Job did not complete within the polling timeout.
    Timeout { job_id: String, elapsed_secs: u64 },
    /// The backend reported an unexpected or terminal failure status.
    UnexpectedStatus { job_id: String, status: String },
    /// Calibration fetch or parse error.
    CalibrationError { detail: String },
}

impl fmt::Display for IonQError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IonQError::ConversionError { detail } =>
                write!(f, "Circuit conversion error: {}", detail),
            IonQError::RestError { detail } =>
                write!(f, "IonQ REST error: {}", detail),
            IonQError::HttpError(e) =>
                write!(f, "HTTP error: {}", e),
            IonQError::Timeout { job_id, elapsed_secs } =>
                write!(f, "Job {} timed out after {}s", job_id, elapsed_secs),
            IonQError::UnexpectedStatus { job_id, status } =>
                write!(f, "Job {} has unexpected status: {}", job_id, status),
            IonQError::CalibrationError { detail } =>
                write!(f, "Calibration error: {}", detail),
        }
    }
}

impl std::error::Error for IonQError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IonQError::HttpError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for IonQError {
    fn from(e: reqwest::Error) -> Self {
        IonQError::HttpError(e)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::error::CqamError;

    #[test]
    fn test_display_conversion_error() {
        let e = IonQError::ConversionError { detail: "unsupported gate: foo".into() };
        assert!(format!("{e}").contains("Circuit conversion error"));
        assert!(format!("{e}").contains("unsupported gate: foo"));
    }

    #[test]
    fn test_display_rest_error() {
        let e = IonQError::RestError { detail: "HTTP 400: bad request".into() };
        assert!(format!("{e}").contains("IonQ REST error"));
        assert!(format!("{e}").contains("HTTP 400"));
    }

    #[test]
    fn test_display_timeout() {
        let e = IonQError::Timeout { job_id: "job-abc".into(), elapsed_secs: 300 };
        let s = format!("{e}");
        assert!(s.contains("job-abc"));
        assert!(s.contains("300"));
    }

    #[test]
    fn test_display_unexpected_status() {
        let e = IonQError::UnexpectedStatus {
            job_id: "job-xyz".into(),
            status: "failed: InvalidCircuit".into(),
        };
        let s = format!("{e}");
        assert!(s.contains("job-xyz"));
        assert!(s.contains("failed: InvalidCircuit"));
    }

    #[test]
    fn test_display_calibration_error() {
        let e = IonQError::CalibrationError { detail: "parse failed".into() };
        assert!(format!("{e}").contains("Calibration error"));
        assert!(format!("{e}").contains("parse failed"));
    }

    #[test]
    fn test_from_rest_error_maps_to_submission_failed() {
        let e = IonQError::RestError { detail: "rate limited".into() };
        let cqam: CqamError = e.into();
        let s = format!("{cqam}");
        assert!(s.contains("IonQ") || matches!(cqam, CqamError::QpuSubmissionFailed { .. }));
        match cqam {
            CqamError::QpuSubmissionFailed { provider, detail } => {
                assert_eq!(provider, "IonQ");
                assert!(detail.contains("rate limited"));
            }
            other => panic!("expected QpuSubmissionFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_from_timeout_maps_to_submission_failed() {
        let e = IonQError::Timeout { job_id: "job-1".into(), elapsed_secs: 60 };
        let cqam: CqamError = e.into();
        match cqam {
            CqamError::QpuSubmissionFailed { provider, detail } => {
                assert_eq!(provider, "IonQ");
                assert!(detail.contains("job-1"));
            }
            other => panic!("expected QpuSubmissionFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_from_unexpected_status_maps_to_submission_failed() {
        let e = IonQError::UnexpectedStatus {
            job_id: "j2".into(),
            status: "canceled".into(),
        };
        let cqam: CqamError = e.into();
        match cqam {
            CqamError::QpuSubmissionFailed { provider, .. } => assert_eq!(provider, "IonQ"),
            other => panic!("expected QpuSubmissionFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_from_calibration_error_maps_to_submission_failed() {
        let e = IonQError::CalibrationError { detail: "404".into() };
        let cqam: CqamError = e.into();
        match cqam {
            CqamError::QpuSubmissionFailed { provider, .. } => assert_eq!(provider, "IonQ"),
            other => panic!("expected QpuSubmissionFailed, got {:?}", other),
        }
    }
}

impl From<IonQError> for cqam_core::error::CqamError {
    fn from(e: IonQError) -> Self {
        match &e {
            IonQError::RestError { detail } =>
                cqam_core::error::CqamError::QpuSubmissionFailed {
                    provider: "IonQ".to_string(),
                    detail: detail.clone(),
                },
            IonQError::Timeout { job_id, .. } =>
                cqam_core::error::CqamError::QpuSubmissionFailed {
                    provider: "IonQ".to_string(),
                    detail: format!("timeout waiting for job {}", job_id),
                },
            _ =>
                cqam_core::error::CqamError::QpuSubmissionFailed {
                    provider: "IonQ".to_string(),
                    detail: e.to_string(),
                },
        }
    }
}
