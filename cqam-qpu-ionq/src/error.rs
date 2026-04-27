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
                // The explicit RestError arm passes the raw detail string through —
                // it must NOT add the "IonQ REST error: " Display prefix.
                assert_eq!(detail, "rate limited",
                    "RestError arm must pass raw detail, not Display-formatted string: got {detail:?}");
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
                // The explicit Timeout arm formats as "timeout waiting for job <id>",
                // NOT as the Display string "Job <id> timed out after <n>s".
                assert_eq!(detail, "timeout waiting for job job-1",
                    "Timeout arm must use its specific format, not Display: got {detail:?}");
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
    fn test_error_source_none_for_non_http_variants() {
        use std::error::Error;
        let non_http: Vec<IonQError> = vec![
            IonQError::ConversionError { detail: "x".into() },
            IonQError::RestError { detail: "x".into() },
            IonQError::Timeout { job_id: "x".into(), elapsed_secs: 0 },
            IonQError::UnexpectedStatus { job_id: "x".into(), status: "x".into() },
            IonQError::CalibrationError { detail: "x".into() },
        ];
        for e in &non_http {
            assert!(e.source().is_none(), "{e:?} must have no error source");
        }
    }

    #[test]
    fn test_from_conversion_error_maps_to_submission_failed() {
        let e = IonQError::ConversionError { detail: "unsupported gate: foo".into() };
        let cqam: CqamError = e.into();
        match cqam {
            CqamError::QpuSubmissionFailed { provider, detail } => {
                assert_eq!(provider, "IonQ");
                assert!(detail.contains("unsupported gate: foo"), "detail: {detail}");
            }
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

    fn make_reqwest_error() -> reqwest::Error {
        // An invalid URL causes a reqwest::Error at build time without any network call.
        reqwest::blocking::Client::new()
            .get("not-a-valid-url")
            .send()
            .expect_err("invalid URL must produce a reqwest::Error without network access")
    }

    #[test]
    fn test_display_http_error() {
        // The HttpError(e) arm of Display (error.rs:29-30) must format as "HTTP error: ...".
        let e = IonQError::HttpError(make_reqwest_error());
        let s = format!("{e}");
        assert!(s.starts_with("HTTP error:"), "HttpError Display must start with 'HTTP error:': got {s}");
    }

    #[test]
    fn test_error_source_http_error_returns_some() {
        // The HttpError(e) arm of source() (error.rs:44) must return Some(&reqwest::Error).
        use std::error::Error;
        let e = IonQError::HttpError(make_reqwest_error());
        assert!(e.source().is_some(), "HttpError must expose its inner reqwest::Error via source()");
    }

    #[test]
    fn test_from_reqwest_error_creates_http_error_variant() {
        // From<reqwest::Error> for IonQError (error.rs:51-53) must produce HttpError.
        let ionq_err: IonQError = make_reqwest_error().into();
        assert!(
            matches!(ionq_err, IonQError::HttpError(_)),
            "From<reqwest::Error> must yield IonQError::HttpError, got {:?}", ionq_err
        );
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
