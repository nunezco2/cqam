//! IBM Quantum Platform REST client.
//!
//! Handles authentication, job submission, status polling, and result
//! retrieval against the IBM Quantum Platform v2 API (api.quantum.ibm.com).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::IbmError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// IBM Quantum Platform API base URL (v2).
const IBM_API_BASE: &str = "https://api.quantum.ibm.com";

/// Job submission / listing path.
///   POST  /api/v1/jobs         -> submit
///   GET   /api/v1/jobs/{id}    -> status
///   GET   /api/v1/jobs/{id}/results -> results
const IBM_JOBS_PATH: &str = "/api/v1/jobs";

/// Backend discovery and configuration path.
///   GET  /api/v1/backends                        -> list
///   GET  /api/v1/backends/{name}/configuration   -> config
const IBM_BACKENDS_PATH: &str = "/api/v1/backends";

const DEFAULT_POLL_INTERVAL_MS: u64 = 2_000;
const DEFAULT_TIMEOUT_SECS: u64 = 600;

// ---------------------------------------------------------------------------
// REST data types
// ---------------------------------------------------------------------------

/// Job submission payload for the IBM Quantum Platform v2 API.
///
/// Serializes to:
/// ```json
/// {
///   "program": { "qasm": "...", "shots": 4096 },
///   "backend": "ibm_brisbane"
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct JobSubmitPayload {
    /// The program to execute.
    pub program: ProgramPayload,
    /// Backend device name (e.g. `"ibm_brisbane"`).
    pub backend: String,
}

/// Program specification within a job submission.
#[derive(Debug, Serialize)]
pub struct ProgramPayload {
    /// OpenQASM 3 source string.
    pub qasm: String,
    /// Number of measurement shots.
    pub shots: u32,
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

/// Summary metadata for an IBM backend device.
///
/// Returned by `GET /api/v1/backends`.  We deserialize only the fields
/// needed for device selection; the response may contain additional
/// provider-specific fields that are silently ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct BackendInfo {
    /// Device name (e.g. `"ibm_brisbane"`, `"ibm_sherbrooke"`).
    pub name: String,
    /// Number of physical qubits.
    pub num_qubits: u32,
    /// Device status string (e.g. `"online"`, `"offline"`, `"maintenance"`).
    pub status: String,
    /// Whether the backend is a simulator rather than real hardware.
    #[serde(default)]
    pub simulator: bool,
}

/// Full backend configuration from the IBM REST API.
///
/// Returned by `GET /api/v1/backends/{name}/configuration`.
#[derive(Debug, Clone, Deserialize)]
pub struct BackendConfig {
    /// Device name.
    pub name: String,
    /// Number of physical qubits.
    pub num_qubits: u32,
    /// Coupling map: list of `[control, target]` qubit pairs.
    ///
    /// IBM returns directed edges.  `ConnectivityGraph::from_edges` normalizes
    /// these to undirected pairs with dedup.
    pub coupling_map: Vec<[u32; 2]>,
    /// Basis gate names (e.g. `["id", "rz", "sx", "x", "cx"]`).
    #[serde(default)]
    pub basis_gates: Vec<String>,
}

/// Raw calibration properties returned by
/// `GET /api/v1/backends/{name}/properties`.
///
/// IBM's response shape:
/// ```json
/// {
///   "qubits": [ [ {"name": "T1", "value": 0.000123, "unit": "s"}, ... ], ... ],
///   "gates":  [ {"gate": "sx", "qubits": [0], "parameters": [...]}, ... ],
///   "last_update_date": "2026-03-19T12:00:00Z"
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct BackendProperties {
    /// Per-qubit calibration properties.  Outer index = qubit index.
    pub qubits: Vec<Vec<QubitProperty>>,
    /// Per-gate calibration properties.
    pub gates: Vec<GateProperty>,
    /// ISO 8601 timestamp of the last calibration run (optional).
    #[serde(default)]
    pub last_update_date: Option<String>,
}

/// A single named property for one qubit (e.g., T1, T2, readout_error).
#[derive(Debug, Clone, Deserialize)]
pub struct QubitProperty {
    /// Property name (e.g., `"T1"`, `"T2"`, `"readout_error"`).
    pub name: String,
    /// Property value.
    pub value: f64,
    /// Physical unit (e.g., `"s"`, `"us"`).  Absent for dimensionless values.
    #[serde(default)]
    pub unit: Option<String>,
}

/// Calibration data for one gate applied to specific qubit(s).
#[derive(Debug, Clone, Deserialize)]
pub struct GateProperty {
    /// Gate name (e.g., `"sx"`, `"cx"`, `"ecr"`).
    pub gate: String,
    /// Qubit indices this gate operates on.
    pub qubits: Vec<u32>,
    /// Calibration parameters for this gate instance.
    pub parameters: Vec<GateParameter>,
}

/// A single named parameter for a gate (e.g., gate_error, gate_length).
#[derive(Debug, Clone, Deserialize)]
pub struct GateParameter {
    /// Parameter name (e.g., `"gate_error"`, `"gate_length"`).
    pub name: String,
    /// Parameter value.
    pub value: f64,
    /// Physical unit (e.g., `"s"`).
    #[serde(default)]
    pub unit: Option<String>,
}

// ---------------------------------------------------------------------------
// RetryPolicy
// ---------------------------------------------------------------------------

/// Configuration for HTTP retry behavior with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts after the initial request.
    /// A value of 0 means no retries -- every request is tried exactly once.
    pub max_retries: u32,
    /// Duration to wait before the first retry.
    pub initial_backoff: Duration,
    /// Upper bound on backoff duration. The backoff will never exceed this
    /// regardless of how many retries have elapsed.
    pub max_backoff: Duration,
    /// Multiplicative factor applied to the backoff after each retry.
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
        }
    }
}

// ---------------------------------------------------------------------------
// IbmRestClient
// ---------------------------------------------------------------------------

/// REST client for interacting with the IBM Quantum Platform.
#[derive(Debug, Clone)]
pub struct IbmRestClient {
    token: String,
    backend_name: String,
    base_url: String,
    http: reqwest::blocking::Client,
    retry_policy: RetryPolicy,
}

impl IbmRestClient {
    /// Construct a client targeting the production IBM Quantum Platform.
    pub fn new(token: impl Into<String>, backend_name: impl Into<String>) -> Self {
        Self::with_base_url(token, backend_name, IBM_API_BASE)
    }

    /// Construct a client with a custom base URL (for testing or staging).
    pub fn with_base_url(
        token: impl Into<String>,
        backend_name: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            token: token.into(),
            backend_name: backend_name.into(),
            base_url: base_url.into(),
            http: reqwest::blocking::Client::new(),
            retry_policy: RetryPolicy::default(),
        }
    }

    /// Override the retry policy for this client.
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Submit a job and return the assigned job ID.
    ///
    /// `qasm_str` is an OpenQASM 3 string produced from the transpiled circuit.
    pub fn submit_job(&self, qasm_str: &str, shots: u32) -> Result<String, IbmError> {
        let payload = JobSubmitPayload {
            program: ProgramPayload {
                qasm: qasm_str.to_string(),
                shots,
            },
            backend: self.backend_name.clone(),
        };

        let url = format!("{}{}", self.base_url, IBM_JOBS_PATH);
        let resp = self.request_with_retry(|| {
            self.http
                .post(&url)
                .bearer_auth(&self.token)
                .json(&payload)
        })?;

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
    /// Blocks the calling thread, sleeping with exponential backoff between
    /// status polls. The initial interval defaults to 2 seconds and grows
    /// by 1.5x per poll, capped at `MAX_POLL_INTERVAL_SECS`.
    ///
    /// Returns `Err(IbmError::Timeout)` if the job does not complete within
    /// `timeout` (default 600 seconds).
    pub fn poll_until_done(
        &self,
        job_id: &str,
        initial_interval: Option<Duration>,
        timeout: Option<Duration>,
    ) -> Result<JobResultResponse, IbmError> {
        const MAX_POLL_INTERVAL_SECS: f64 = 30.0;
        const POLL_BACKOFF_MULTIPLIER: f64 = 1.5;

        let mut interval = initial_interval
            .unwrap_or(Duration::from_millis(DEFAULT_POLL_INTERVAL_MS));
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
                // RUNNING, PENDING, QUEUED, VALIDATING, ...
                _ => {}
            }

            if start.elapsed() > deadline {
                return Err(IbmError::Timeout {
                    job_id: job_id.to_string(),
                    elapsed_secs: start.elapsed().as_secs(),
                });
            }

            std::thread::sleep(interval);

            // Exponential backoff capped at MAX_POLL_INTERVAL_SECS.
            interval = Duration::from_secs_f64(
                (interval.as_secs_f64() * POLL_BACKOFF_MULTIPLIER)
                    .min(MAX_POLL_INTERVAL_SECS),
            );
        }

        self.get_job_results(job_id)
    }

    /// Fetch current status of a job.
    pub fn get_job_status(&self, job_id: &str) -> Result<JobStatusResponse, IbmError> {
        let url = format!("{}{}/{}", self.base_url, IBM_JOBS_PATH, job_id);
        let resp = self.request_with_retry(|| {
            self.http.get(&url).bearer_auth(&self.token)
        })?;

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
        let url = format!("{}{}/{}/results", self.base_url, IBM_JOBS_PATH, job_id);
        let resp = self.request_with_retry(|| {
            self.http.get(&url).bearer_auth(&self.token)
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IbmError::RestError {
                detail: format!("job result HTTP {}: {}", status, body),
            });
        }

        resp.json().map_err(IbmError::HttpError)
    }

    /// List available IBM Quantum backends.
    ///
    /// Returns metadata for all backends accessible with the configured
    /// API token.  Use the `name` field of a returned `BackendInfo` as
    /// the argument to `get_backend_config`.
    pub fn list_backends(&self) -> Result<Vec<BackendInfo>, IbmError> {
        let url = format!("{}{}", self.base_url, IBM_BACKENDS_PATH);
        let resp = self.request_with_retry(|| {
            self.http.get(&url).bearer_auth(&self.token)
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IbmError::RestError {
                detail: format!("list backends HTTP {}: {}", status, body),
            });
        }

        resp.json().map_err(IbmError::HttpError)
    }

    /// Fetch the configuration (coupling map, basis gates) for a named backend.
    ///
    /// The `backend_name` parameter is the device identifier (e.g.
    /// `"ibm_brisbane"`), not necessarily the same as `self.backend_name`.
    /// This allows querying configurations for devices other than the one
    /// the client was constructed for, which is useful for discovery workflows.
    pub fn get_backend_config(
        &self,
        backend_name: &str,
    ) -> Result<BackendConfig, IbmError> {
        let url = format!(
            "{}{}/{}/configuration",
            self.base_url, IBM_BACKENDS_PATH, backend_name
        );
        let resp = self.request_with_retry(|| {
            self.http.get(&url).bearer_auth(&self.token)
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IbmError::RestError {
                detail: format!(
                    "backend config for '{}' HTTP {}: {}",
                    backend_name, status, body
                ),
            });
        }

        resp.json().map_err(IbmError::HttpError)
    }

    /// Fetch device calibration properties.
    ///
    /// Calls `GET /api/v1/backends/{name}/properties` and deserializes the
    /// response into `BackendProperties`.
    ///
    /// # Errors
    ///
    /// Returns `IbmError::CalibrationError` if the request fails or the
    /// response cannot be parsed.
    pub fn get_backend_properties(
        &self,
        backend_name: &str,
    ) -> Result<BackendProperties, IbmError> {
        let url = format!(
            "{}{}/{}/properties",
            self.base_url, IBM_BACKENDS_PATH, backend_name
        );
        let resp = self.request_with_retry(|| {
            self.http.get(&url).bearer_auth(&self.token)
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IbmError::CalibrationError {
                detail: format!(
                    "properties for '{}' HTTP {}: {}",
                    backend_name, status, body
                ),
            });
        }

        resp.json().map_err(|e| IbmError::CalibrationError {
            detail: format!(
                "properties parse error for '{}': {}",
                backend_name, e
            ),
        })
    }

    /// Execute an HTTP request with retry on transient errors.
    ///
    /// `build_request` is a closure that constructs a fresh `RequestBuilder`
    /// on each attempt. It must be `Fn` (not `FnOnce`) because retries call
    /// it multiple times.
    ///
    /// Retries on:
    /// - HTTP 429 (Too Many Requests) -- respects `Retry-After` header
    /// - HTTP 502, 503, 504 (Bad Gateway / Service Unavailable / Gateway Timeout)
    /// - Transport-level connection and timeout errors
    ///
    /// All other status codes and errors are returned immediately.
    fn request_with_retry<F>(
        &self,
        build_request: F,
    ) -> Result<reqwest::blocking::Response, IbmError>
    where
        F: Fn() -> reqwest::blocking::RequestBuilder,
    {
        let policy = &self.retry_policy;
        let mut backoff = policy.initial_backoff;

        for attempt in 0..=policy.max_retries {
            let resp = build_request().send();

            match resp {
                // --- HTTP 429: respect Retry-After if present ---------------
                Ok(r) if r.status().as_u16() == 429 => {
                    if attempt == policy.max_retries {
                        return Ok(r);
                    }
                    let wait = parse_retry_after(&r, backoff);
                    std::thread::sleep(wait);
                }

                // --- HTTP 502/503/504: server-side transient ----------------
                Ok(r) if (502..=504).contains(&r.status().as_u16()) => {
                    if attempt == policy.max_retries {
                        return Ok(r);
                    }
                    std::thread::sleep(backoff);
                }

                // --- Any other successful response: return immediately ------
                Ok(r) => return Ok(r),

                // --- Transport error, transient: retry ----------------------
                Err(e) if is_transient(&e) && attempt < policy.max_retries => {
                    std::thread::sleep(backoff);
                }

                // --- Transport error, non-transient or exhausted: fail ------
                Err(e) => return Err(IbmError::HttpError(e)),
            }

            // Advance backoff for next iteration (capped at max_backoff).
            backoff = Duration::from_secs_f64(
                (backoff.as_secs_f64() * policy.backoff_multiplier)
                    .min(policy.max_backoff.as_secs_f64()),
            );
        }

        // Unreachable: the loop runs max_retries+1 times and every branch
        // either returns or continues. The final iteration always returns.
        unreachable!("retry loop must return on final attempt")
    }
}

// ---------------------------------------------------------------------------
// Retry helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the transport error is transient and worth retrying.
///
/// Currently covers:
/// - Connection errors (DNS, TCP connect, TLS handshake)
/// - Timeout errors (read/write/connect timeout)
fn is_transient(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout()
}

/// Parse the `Retry-After` header from a response.
///
/// Returns the parsed duration if the header contains a valid integer
/// (number of seconds). Falls back to `default` if the header is absent,
/// unparseable, or specifies a value exceeding `MAX_RETRY_AFTER_SECS`.
fn parse_retry_after(resp: &reqwest::blocking::Response, default: Duration) -> Duration {
    const MAX_RETRY_AFTER_SECS: u64 = 300; // 5-minute safety cap

    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&secs| secs <= MAX_RETRY_AFTER_SECS)
        .map(Duration::from_secs)
        .unwrap_or(default)
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

    // --- parse_counts (unchanged) -------------------------------------------

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

    // --- base URL / constructor tests ----------------------------------------

    impl IbmRestClient {
        /// Expose base_url for test assertions.
        fn base_url(&self) -> &str {
            &self.base_url
        }

        /// Expose retry_policy for test assertions.
        fn retry_policy(&self) -> &RetryPolicy {
            &self.retry_policy
        }
    }

    #[test]
    fn test_default_base_url() {
        let client = IbmRestClient::new("tok", "dev");
        assert_eq!(client.base_url(), "https://api.quantum.ibm.com");
    }

    #[test]
    fn test_custom_base_url() {
        let client = IbmRestClient::with_base_url("tok", "dev", "http://localhost:8080");
        assert_eq!(client.base_url(), "http://localhost:8080");
    }

    // --- URL construction tests ----------------------------------------------

    #[test]
    fn test_job_submit_url_format() {
        let base = "http://mock:9999";
        let expected = format!("{}/api/v1/jobs", base);
        let constructed = format!("{}{}", base, IBM_JOBS_PATH);
        assert_eq!(constructed, expected);
    }

    #[test]
    fn test_job_status_url_format() {
        let base = "http://mock:9999";
        let job_id = "abc-123";
        let expected = format!("{}/api/v1/jobs/{}", base, job_id);
        let constructed = format!("{}{}/{}", base, IBM_JOBS_PATH, job_id);
        assert_eq!(constructed, expected);
    }

    #[test]
    fn test_job_results_url_format() {
        let base = "http://mock:9999";
        let job_id = "abc-123";
        let expected = format!("{}/api/v1/jobs/{}/results", base, job_id);
        let constructed = format!("{}{}/{}/results", base, IBM_JOBS_PATH, job_id);
        assert_eq!(constructed, expected);
    }

    // --- payload serialization test ------------------------------------------

    #[test]
    fn test_submit_payload_serialization() {
        let payload = JobSubmitPayload {
            program: ProgramPayload {
                qasm: "OPENQASM 3.0;\nqubit q;\nh q;\nmeasure q;".to_string(),
                shots: 4096,
            },
            backend: "ibm_brisbane".to_string(),
        };

        let json: serde_json::Value = serde_json::to_value(&payload).unwrap();

        // Top-level keys
        assert!(json.get("program").is_some(), "must have 'program' key");
        assert!(json.get("backend").is_some(), "must have 'backend' key");
        assert!(
            json.get("backend_name").is_none(),
            "must NOT have old 'backend_name' key"
        );
        assert!(
            json.get("memory").is_none(),
            "must NOT have old 'memory' key"
        );

        // Nested program
        let program = &json["program"];
        assert_eq!(program["shots"], 4096);
        assert!(program["qasm"].as_str().unwrap().contains("OPENQASM 3.0"));

        // Backend
        assert_eq!(json["backend"], "ibm_brisbane");
    }

    // --- RetryPolicy tests ---------------------------------------------------

    #[test]
    fn test_retry_policy_default() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_retries, 5);
        assert_eq!(p.initial_backoff, Duration::from_secs(1));
        assert_eq!(p.max_backoff, Duration::from_secs(60));
        assert!((p.backoff_multiplier - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_policy_clone() {
        let p = RetryPolicy::default();
        let p2 = p.clone();
        assert_eq!(p2.max_retries, p.max_retries);
    }

    #[test]
    fn test_backoff_progression() {
        let policy = RetryPolicy {
            max_retries: 4,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
        };

        let mut backoff = policy.initial_backoff;
        let mut intervals = vec![backoff];
        for _ in 0..policy.max_retries {
            backoff = Duration::from_secs_f64(
                (backoff.as_secs_f64() * policy.backoff_multiplier)
                    .min(policy.max_backoff.as_secs_f64()),
            );
            intervals.push(backoff);
        }

        // Expected: 1, 2, 4, 8, 10 (capped)
        assert_eq!(intervals.len(), 5);
        assert_eq!(intervals[0], Duration::from_secs(1));
        assert_eq!(intervals[1], Duration::from_secs(2));
        assert_eq!(intervals[2], Duration::from_secs(4));
        assert_eq!(intervals[3], Duration::from_secs(8));
        assert_eq!(intervals[4], Duration::from_secs(10)); // capped at max_backoff
    }

    #[test]
    fn test_zero_retries_single_attempt() {
        let policy = RetryPolicy {
            max_retries: 0,
            ..RetryPolicy::default()
        };
        // 0..=0 yields exactly one iteration
        let attempts: Vec<u32> = (0..=policy.max_retries).collect();
        assert_eq!(attempts, vec![0]);
    }

    #[test]
    fn test_with_retry_policy_builder() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 3.0,
        };
        let client = IbmRestClient::new("tok", "dev")
            .with_retry_policy(policy.clone());
        assert_eq!(client.retry_policy().max_retries, 2);
        assert_eq!(client.retry_policy().initial_backoff, Duration::from_millis(500));
    }

    #[test]
    fn test_poll_backoff_progression() {
        // Verify the 1.5x multiplier with 30s cap used in poll_until_done.
        let mut interval = Duration::from_secs(2);
        let cap = 30.0_f64;
        let mult = 1.5_f64;

        let mut intervals = vec![interval.as_secs_f64()];
        for _ in 0..10 {
            interval = Duration::from_secs_f64(
                (interval.as_secs_f64() * mult).min(cap),
            );
            intervals.push(interval.as_secs_f64());
        }

        // Should reach cap within ~8 iterations
        assert!(intervals[7] <= 30.0);
        assert!((intervals.last().unwrap() - 30.0).abs() < 0.01);
        // First interval is 2s
        assert!((intervals[0] - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_is_transient_connection_error() {
        // Connect to a port that is almost certainly not listening.
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();
        let err = client.get("http://192.0.2.1:1").send().unwrap_err();
        // 192.0.2.1 is TEST-NET-1 (RFC 5737), connection will time out.
        assert!(is_transient(&err), "connection/timeout error should be transient");
    }

    // --- BackendInfo deserialization tests -----------------------------------

    #[test]
    fn test_backend_info_deserialization() {
        let json = r#"{
            "name": "ibm_brisbane",
            "num_qubits": 127,
            "status": "online",
            "simulator": false
        }"#;
        let info: BackendInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "ibm_brisbane");
        assert_eq!(info.num_qubits, 127);
        assert_eq!(info.status, "online");
        assert!(!info.simulator);
    }

    #[test]
    fn test_backend_info_simulator_default() {
        let json = r#"{"name": "test_dev", "num_qubits": 5, "status": "online"}"#;
        let info: BackendInfo = serde_json::from_str(json).unwrap();
        assert!(!info.simulator, "simulator should default to false");
    }

    // --- BackendConfig deserialization tests ---------------------------------

    #[test]
    fn test_backend_config_deserialization() {
        let json = r#"{
            "name": "ibm_brisbane",
            "num_qubits": 7,
            "coupling_map": [[0,1],[1,0],[1,2],[2,1],[2,3],[3,2],[3,4],[4,3],[4,5],[5,4],[5,6],[6,5]],
            "basis_gates": ["id", "rz", "sx", "x", "cx"]
        }"#;
        let config: BackendConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.name, "ibm_brisbane");
        assert_eq!(config.num_qubits, 7);
        assert_eq!(config.coupling_map.len(), 12); // 6 edges x 2 directions
        assert_eq!(config.basis_gates.len(), 5);
    }

    #[test]
    fn test_backend_config_empty_coupling_map() {
        let json = r#"{
            "name": "ibm_qasm_simulator",
            "num_qubits": 32,
            "coupling_map": []
        }"#;
        let config: BackendConfig = serde_json::from_str(json).unwrap();
        assert!(config.coupling_map.is_empty());
        assert!(config.basis_gates.is_empty()); // default
    }

    // --- Backends list deserialization ---------------------------------------

    #[test]
    fn test_backends_list_deserialization() {
        let json = r#"[
            {"name": "ibm_brisbane", "num_qubits": 127, "status": "online", "simulator": false},
            {"name": "ibm_qasm_simulator", "num_qubits": 32, "status": "online", "simulator": true}
        ]"#;
        let backends: Vec<BackendInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(backends.len(), 2);
        assert!(!backends[0].simulator);
        assert!(backends[1].simulator);
    }

    // --- URL construction tests for backend paths ----------------------------

    #[test]
    fn test_backends_list_url_format() {
        let base = "http://mock:9999";
        let expected = "http://mock:9999/api/v1/backends";
        let constructed = format!("{}{}", base, IBM_BACKENDS_PATH);
        assert_eq!(constructed, expected);
    }

    #[test]
    fn test_backend_config_url_format() {
        let base = "http://mock:9999";
        let name = "ibm_brisbane";
        let expected = "http://mock:9999/api/v1/backends/ibm_brisbane/configuration";
        let constructed = format!("{}{}/{}/configuration", base, IBM_BACKENDS_PATH, name);
        assert_eq!(constructed, expected);
    }

    // --- BackendProperties deserialization tests -----------------------------

    const IBM_PROPERTIES_JSON: &str = r#"{
        "qubits": [
            [
                {"name": "T1", "value": 0.000123, "unit": "s"},
                {"name": "T2", "value": 0.000098, "unit": "s"},
                {"name": "readout_error", "value": 0.012},
                {"name": "frequency", "value": 5.1e9, "unit": "GHz"}
            ],
            [
                {"name": "T1", "value": 0.000110, "unit": "s"},
                {"name": "T2", "value": 0.000085, "unit": "s"},
                {"name": "readout_error", "value": 0.015}
            ],
            [
                {"name": "T1", "value": 0.000095, "unit": "s"},
                {"name": "T2", "value": 0.000075, "unit": "s"},
                {"name": "readout_error", "value": 0.018}
            ]
        ],
        "gates": [
            {
                "gate": "sx",
                "qubits": [0],
                "parameters": [
                    {"name": "gate_error", "value": 0.00035},
                    {"name": "gate_length", "value": 3.5556e-8, "unit": "s"}
                ]
            },
            {
                "gate": "sx",
                "qubits": [1],
                "parameters": [
                    {"name": "gate_error", "value": 0.00042}
                ]
            },
            {
                "gate": "sx",
                "qubits": [2],
                "parameters": [
                    {"name": "gate_error", "value": 0.00028}
                ]
            },
            {
                "gate": "cx",
                "qubits": [0, 1],
                "parameters": [
                    {"name": "gate_error", "value": 0.0078},
                    {"name": "gate_length", "value": 6.6e-7, "unit": "s"}
                ]
            },
            {
                "gate": "cx",
                "qubits": [1, 2],
                "parameters": [
                    {"name": "gate_error", "value": 0.0092},
                    {"name": "gate_length", "value": 7.1e-7, "unit": "s"}
                ]
            }
        ],
        "last_update_date": "2026-03-19T12:00:00Z"
    }"#;

    #[test]
    fn test_backend_properties_deserialization() {
        let props: BackendProperties =
            serde_json::from_str(IBM_PROPERTIES_JSON).unwrap();
        assert_eq!(props.qubits.len(), 3);
        assert_eq!(props.gates.len(), 5);
        assert_eq!(
            props.last_update_date.as_deref(),
            Some("2026-03-19T12:00:00Z")
        );

        // Spot-check qubit 0 T1
        let t1 = props.qubits[0]
            .iter()
            .find(|p| p.name == "T1")
            .unwrap();
        assert!((t1.value - 0.000123).abs() < 1e-12);
        assert_eq!(t1.unit.as_deref(), Some("s"));

        // Spot-check CX gate on [0,1]
        let cx01 = props.gates.iter()
            .find(|g| g.gate == "cx" && g.qubits == vec![0, 1])
            .unwrap();
        let err = cx01.parameters.iter()
            .find(|p| p.name == "gate_error")
            .unwrap();
        assert!((err.value - 0.0078).abs() < 1e-12);
    }

    #[test]
    fn test_backend_properties_url_format() {
        let base = "http://mock:9999";
        let name = "ibm_brisbane";
        let expected = "http://mock:9999/api/v1/backends/ibm_brisbane/properties";
        let constructed = format!(
            "{}{}/{}/properties",
            base, IBM_BACKENDS_PATH, name
        );
        assert_eq!(constructed, expected);
    }
}
