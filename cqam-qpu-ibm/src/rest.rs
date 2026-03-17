//! IBM Quantum Platform REST client.
//!
//! Handles authentication, job submission, status polling, and result
//! retrieval.  The actual IBM API base URL and endpoint paths are
//! placeholder values -- full REST integration is Phase 6.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::IbmError;

// ---------------------------------------------------------------------------
// Constants (placeholder -- Phase 6 will wire real endpoints)
// ---------------------------------------------------------------------------

const IBM_API_BASE: &str = "https://quantum-computing.ibm.com/api";
const IBM_JOBS_ENDPOINT: &str = "/Network/jobs";
const DEFAULT_POLL_INTERVAL_MS: u64 = 2_000;
const DEFAULT_TIMEOUT_SECS: u64 = 600;

// ---------------------------------------------------------------------------
// REST data types
// ---------------------------------------------------------------------------

/// Serialized job submission payload (QASM / OpenQASM 3).
#[derive(Debug, Serialize)]
pub struct JobSubmitRequest {
    pub backend_name: String,
    pub qasm: String,
    pub shots: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<bool>,
}

/// Minimal shape of the IBM job creation response.
#[derive(Debug, Deserialize)]
pub struct JobSubmitResponse {
    pub id: String,
    pub status: String,
}

/// Minimal shape of the IBM job status response.
#[derive(Debug, Deserialize)]
pub struct JobStatusResponse {
    pub id: String,
    pub status: String,
}

/// Minimal shape of the IBM job result response.
#[derive(Debug, Deserialize)]
pub struct JobResultResponse {
    pub id: String,
    pub results: Vec<ExperimentResult>,
}

/// Result for a single experiment (circuit) in the job.
#[derive(Debug, Deserialize)]
pub struct ExperimentResult {
    pub success: bool,
    pub shots: u32,
    pub data: ExperimentData,
}

/// Measurement data (counts histogram).
#[derive(Debug, Deserialize)]
pub struct ExperimentData {
    pub counts: BTreeMap<String, u32>,
}

// ---------------------------------------------------------------------------
// IbmRestClient
// ---------------------------------------------------------------------------

/// REST client for interacting with the IBM Quantum Platform.
#[derive(Debug, Clone)]
pub struct IbmRestClient {
    token: String,
    backend_name: String,
    http: reqwest::blocking::Client,
}

impl IbmRestClient {
    /// Construct a new client with the given IBM Quantum API token.
    pub fn new(token: impl Into<String>, backend_name: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            backend_name: backend_name.into(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Submit a job and return the assigned job ID.
    ///
    /// `qasm_str` is an OpenQASM 3 string produced from the transpiled circuit.
    pub fn submit_job(&self, qasm_str: &str, shots: u32) -> Result<String, IbmError> {
        let payload = JobSubmitRequest {
            backend_name: self.backend_name.clone(),
            qasm: qasm_str.to_string(),
            shots,
            memory: Some(false),
        };

        let url = format!("{}{}", IBM_API_BASE, IBM_JOBS_ENDPOINT);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .map_err(IbmError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IbmError::RestError {
                detail: format!("job submission HTTP {}: {}", status, body),
            });
        }

        let job: JobSubmitResponse = resp.json().map_err(IbmError::HttpError)?;
        Ok(job.id)
    }

    /// Poll for the job status, returning the final result when complete.
    ///
    /// Blocks the calling thread, sleeping `poll_interval` between requests.
    /// Returns `Err(IbmError::Timeout)` if the job does not complete within
    /// `timeout`.
    pub fn poll_until_done(
        &self,
        job_id: &str,
        poll_interval: Option<Duration>,
        timeout: Option<Duration>,
    ) -> Result<JobResultResponse, IbmError> {
        let interval = poll_interval.unwrap_or(Duration::from_millis(DEFAULT_POLL_INTERVAL_MS));
        let deadline = timeout.unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        let start = Instant::now();

        loop {
            let status = self.get_job_status(job_id)?;
            match status.status.to_uppercase().as_str() {
                "COMPLETED" | "DONE" => break,
                "FAILED" | "CANCELLED" | "ERROR" => {
                    return Err(IbmError::UnexpectedStatus {
                        job_id: job_id.to_string(),
                        status: status.status,
                    });
                }
                // RUNNING, PENDING, QUEUED, VALIDATING, …
                _ => {}
            }

            if start.elapsed() > deadline {
                return Err(IbmError::Timeout {
                    job_id: job_id.to_string(),
                    elapsed_secs: start.elapsed().as_secs(),
                });
            }

            std::thread::sleep(interval);
        }

        self.get_job_results(job_id)
    }

    /// Fetch current status of a job.
    pub fn get_job_status(&self, job_id: &str) -> Result<JobStatusResponse, IbmError> {
        let url = format!("{}{}/{}", IBM_API_BASE, IBM_JOBS_ENDPOINT, job_id);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .map_err(IbmError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IbmError::RestError {
                detail: format!("job status HTTP {}: {}", status, body),
            });
        }

        resp.json().map_err(IbmError::HttpError)
    }

    /// Fetch the final result of a completed job.
    pub fn get_job_results(&self, job_id: &str) -> Result<JobResultResponse, IbmError> {
        let url = format!("{}{}/{}/results", IBM_API_BASE, IBM_JOBS_ENDPOINT, job_id);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .map_err(IbmError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IbmError::RestError {
                detail: format!("job result HTTP {}: {}", status, body),
            });
        }

        resp.json().map_err(IbmError::HttpError)
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse a hex bitstring count map (IBM format) into a `BTreeMap<u64, u32>`.
///
/// IBM encodes bitstrings as hex strings prefixed with `0x`, e.g. `"0x3"`.
pub fn parse_counts(raw: &BTreeMap<String, u32>) -> BTreeMap<u64, u32> {
    let mut out = BTreeMap::new();
    for (k, &v) in raw {
        let stripped = k.strip_prefix("0x").unwrap_or(k.as_str());
        if let Ok(bits) = u64::from_str_radix(stripped, 16) {
            *out.entry(bits).or_insert(0) += v;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_counts_hex() {
        let mut raw = BTreeMap::new();
        raw.insert("0x0".to_string(), 512);
        raw.insert("0x3".to_string(), 512);
        let counts = parse_counts(&raw);
        assert_eq!(counts.get(&0u64), Some(&512));
        assert_eq!(counts.get(&3u64), Some(&512));
    }

    #[test]
    fn test_parse_counts_no_prefix() {
        let mut raw = BTreeMap::new();
        raw.insert("ff".to_string(), 100);
        let counts = parse_counts(&raw);
        assert_eq!(counts.get(&255u64), Some(&100));
    }

    #[test]
    fn test_parse_counts_empty() {
        let raw = BTreeMap::new();
        assert!(parse_counts(&raw).is_empty());
    }
}
