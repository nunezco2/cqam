//! IonQ Cloud REST client (API v0.4).
//!
//! Handles authentication, job submission, status polling, and result
//! retrieval against `https://api.ionq.co/v0.4`.
//!
//! Auth: API key passed as `Authorization: apiKey <key>` on every request.
//! No token exchange needed.
//!
//! Workflow for a single job:
//! 1. POST `/jobs` → `JobSubmitResponse` (contains `id`).
//! 2. Poll GET `/jobs/{id}` → `JobResponse` until `status == "completed"`.
//! 3. When complete, `JobResponse.results.probabilities.url` is a relative path.
//!    GET that URL → flat `{ "0": 0.49, "3": 0.51 }` (decimal bitstring → prob).
//!
//! Characterization:
//! 1. GET `/backends` → list of `BackendInfo` (includes `characterization_id`).
//! 2. GET `/backends/{backend}/characterizations/{uuid}` → `CharacterizationResponse`.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::IonQError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const IONQ_API_BASE: &str = "https://api.ionq.co/v0.4";

const DEFAULT_POLL_INTERVAL_MS: u64 = 2_000;
const DEFAULT_TIMEOUT_SECS: u64 = 1800;

// ---------------------------------------------------------------------------
// REST data types — Job submission
// ---------------------------------------------------------------------------

/// Job submission payload for the IonQ Cloud API v0.4.
///
/// Serializes to:
/// ```json
/// {
///   "type": "ionq.circuit.v1",
///   "backend": "simulator",
///   "shots": 1024,
///   "input": { "gateset": "qis", "qubits": 2, "circuit": [...] }
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct JobSubmitPayload {
    /// Always `"ionq.circuit.v1"` for v0.4 QIS circuit jobs.
    #[serde(rename = "type")]
    pub type_: String,
    /// Target backend (e.g. `"simulator"`, `"qpu.forte-1"`).
    pub backend: String,
    /// Number of shots to execute.
    pub shots: u32,
    /// Native JSON circuit (`input` object from `circuit_to_ionq_json`).
    pub input: serde_json::Value,
    /// Optional human-readable job name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Response from job creation (POST /jobs).
///
/// Real response shape:
/// `{"id": "...", "status": "submitted", "session_id": null}`
#[derive(Debug, Deserialize)]
pub struct JobSubmitResponse {
    pub id: String,
    #[serde(default)]
    pub status: String,
}

// ---------------------------------------------------------------------------
// REST data types — Job status
// ---------------------------------------------------------------------------

/// Full job status response from GET /jobs/{id}.
///
/// Does NOT contain histogram data — use the URL in `results.probabilities.url`
/// to fetch the probabilities.
///
/// Real completed response includes many additional fields (`type`, `name`,
/// `backend`, `output`, `stats`, etc.) that are not captured here; serde
/// ignores them by default.
#[derive(Debug, Deserialize)]
pub struct JobResponse {
    pub id: String,
    pub status: String,
    /// Echoed shot count. Not always present in the completed-job response.
    #[serde(default)]
    pub shots: Option<u32>,
    /// Set only when the job has completed.
    #[serde(default)]
    pub results: Option<JobResults>,
    /// Set only when the job has failed. Null when not failed.
    #[serde(default)]
    pub failure: Option<JobFailure>,
}

/// Nested `results` block in a completed `JobResponse`.
#[derive(Debug, Deserialize)]
pub struct JobResults {
    /// URL reference for the probabilities histogram.
    #[serde(default)]
    pub probabilities: Option<ResultsUrl>,
}

/// A URL pointing to a results resource.
#[derive(Debug, Deserialize)]
pub struct ResultsUrl {
    pub url: String,
}

/// Failure information for a failed IonQ job.
#[derive(Debug, Deserialize)]
pub struct JobFailure {
    /// Machine-readable failure code (string enum in v0.4).
    #[serde(default)]
    pub code: Option<String>,
    /// Human-readable failure message.
    #[serde(default)]
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// REST data types — Backend list
// ---------------------------------------------------------------------------

/// Summary of an IonQ backend, returned by GET /backends.
///
/// Real response fields vary by backend type. Simulators include
/// `noise_models`; QPUs include `characterization_id`. Unknown fields
/// are silently ignored.
#[derive(Debug, Deserialize, Clone)]
pub struct BackendInfo {
    pub backend: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub degraded: Option<bool>,
    #[serde(default)]
    pub qubits: Option<u32>,
    #[serde(default)]
    pub average_queue_time: Option<u64>,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub noise_models: Option<Vec<String>>,
    /// UUID of the latest characterization record for this backend.
    /// Present for QPUs; absent for simulators.
    #[serde(default)]
    pub characterization_id: Option<String>,
}

// ---------------------------------------------------------------------------
// REST data types — Characterization
// ---------------------------------------------------------------------------

/// Full characterization response from
/// GET /backends/{backend}/characterizations/{uuid}.
///
/// Real response also includes `date`, `connectivity`, and per-qubit detail
/// arrays; only fields used by the calibration model are captured here.
#[derive(Debug, Deserialize, Clone)]
pub struct CharacterizationResponse {
    pub id: String,
    #[serde(default)]
    pub date: Option<String>,
    pub backend: String,
    pub qubits: u32,
    #[serde(default)]
    pub fidelity: Option<CharFidelity>,
    #[serde(default)]
    pub timing: Option<CharTiming>,
}

/// Fidelity section of a characterization response.
///
/// Real response shape:
/// ```json
/// {
///   "spam": {"median": 0.9942, "stderr": null},
///   "1q":   {"median": 0.9998, "stderr": 0},
///   "2q":   {"median": 0.9952, "stderr": 0}
/// }
/// ```
#[derive(Debug, Deserialize, Clone)]
pub struct CharFidelity {
    /// SPAM (state-prep-and-measurement) median fidelity.
    #[serde(default)]
    pub spam: Option<GateFidelity>,
    /// Single-qubit gate median fidelity. API field name is `"1q"`.
    #[serde(rename = "1q", default)]
    pub single_qubit: Option<GateFidelity>,
    /// Two-qubit gate median fidelity. API field name is `"2q"`.
    #[serde(rename = "2q", default)]
    pub two_qubit: Option<GateFidelity>,
}

/// Median fidelity (and optional standard error) for a gate type.
#[derive(Debug, Deserialize, Clone)]
pub struct GateFidelity {
    #[serde(default)]
    pub median: Option<f64>,
}

/// Gate timing section of a characterization response (all values in seconds).
///
/// Real response shape:
/// ```json
/// {"readout": 0, "reset": 0, "1q": 0, "2q": 0, "t1": 100, "t2": 1}
/// ```
/// `readout` and `reset` are not used by the calibration model.
#[derive(Debug, Deserialize, Clone)]
pub struct CharTiming {
    #[serde(default)]
    pub t1: Option<f64>,
    #[serde(default)]
    pub t2: Option<f64>,
    /// Single-qubit gate time (seconds). API field name is `"1q"`.
    #[serde(rename = "1q", default)]
    pub single_qubit: Option<f64>,
    /// Two-qubit gate time (seconds). API field name is `"2q"`.
    #[serde(rename = "2q", default)]
    pub two_qubit: Option<f64>,
}

// ---------------------------------------------------------------------------
// IonQRestClient
// ---------------------------------------------------------------------------

/// REST client for the IonQ Cloud API v0.4.
///
/// Uses API-key authentication: every request includes the header
/// `Authorization: apiKey <key>`. No token exchange is required.
#[derive(Clone)]
pub struct IonQRestClient {
    api_key: String,
    backend: String,
    pub(crate) base_url: String,
    http: reqwest::blocking::Client,
    poll_timeout_secs: u64,
}

impl fmt::Debug for IonQRestClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IonQRestClient")
            .field("api_key", &"***")
            .field("backend", &self.backend)
            .field("base_url", &self.base_url)
            .field("poll_timeout_secs", &self.poll_timeout_secs)
            .finish()
    }
}

impl IonQRestClient {
    /// Construct a client targeting the production IonQ Cloud API v0.4.
    ///
    /// `api_key` is your IonQ Cloud API key.
    /// `backend_name` is the device (e.g. `"simulator"`, `"qpu.forte-1"`).
    pub fn new(
        api_key: impl Into<String>,
        backend_name: impl Into<String>,
    ) -> Self {
        Self::with_base_url(api_key, backend_name, IONQ_API_BASE)
    }

    /// Construct a client with a custom base URL (for testing or staging).
    pub fn with_base_url(
        api_key: impl Into<String>,
        backend_name: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            backend: backend_name.into(),
            base_url: base_url.into(),
            http: reqwest::blocking::Client::new(),
            poll_timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Set the job polling timeout in seconds.
    pub fn with_poll_timeout(mut self, secs: u64) -> Self {
        self.poll_timeout_secs = secs;
        self
    }

    fn auth_header(&self) -> String {
        format!("apiKey {}", self.api_key)
    }

    fn authed_get(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .get(url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
    }

    fn authed_post(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .post(url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
    }

    /// Submit a native JSON circuit and return the assigned job ID.
    ///
    /// `circuit_json` must be the `input` object produced by
    /// `circuit_to_ionq_json`.
    pub fn submit_job(
        &self,
        circuit_json: serde_json::Value,
        shots: u32,
    ) -> Result<String, IonQError> {
        let payload = JobSubmitPayload {
            type_: "ionq.circuit.v1".to_string(),
            backend: self.backend.clone(),
            shots,
            input: circuit_json,
            name: None,
        };

        let url = format!("{}/jobs", self.base_url);
        let resp = self
            .authed_post(&url)
            .json(&payload)
            .send()
            .map_err(IonQError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IonQError::RestError {
                detail: format!("job submission HTTP {}: {}", status, body),
            });
        }

        let job: JobSubmitResponse = resp.json().map_err(IonQError::HttpError)?;
        Ok(job.id)
    }

    /// Poll until a job reaches a terminal status, then return the response.
    ///
    /// Blocks the calling thread with exponential back-off between polls.
    /// Returns `Err(IonQError::Timeout)` if the job does not complete within
    /// the configured timeout.
    ///
    /// Valid v0.4 terminal statuses: `completed`, `canceled`, `failed`.
    pub fn poll_until_done(
        &self,
        job_id: &str,
        initial_interval: Option<Duration>,
        timeout: Option<Duration>,
    ) -> Result<JobResponse, IonQError> {
        const MAX_INTERVAL_SECS: f64 = 30.0;
        const BACKOFF_MULTIPLIER: f64 = 1.5;

        let mut interval = initial_interval
            .unwrap_or(Duration::from_millis(DEFAULT_POLL_INTERVAL_MS));
        let deadline = timeout.unwrap_or(Duration::from_secs(self.poll_timeout_secs));
        let start = Instant::now();

        loop {
            let job = self.get_job(job_id)?;

            match job.status.to_lowercase().as_str() {
                "completed" => return Ok(job),
                "canceled" | "failed" => {
                    let reason = job
                        .failure
                        .as_ref()
                        .and_then(|f| f.message.clone())
                        .or_else(|| {
                            job.failure.as_ref().and_then(|f| f.code.clone())
                        })
                        .unwrap_or_default();
                    return Err(IonQError::UnexpectedStatus {
                        job_id: job_id.to_string(),
                        status: format!("{}: {}", job.status, reason),
                    });
                }
                // submitted, ready, started — keep polling
                _ => {}
            }

            if start.elapsed() > deadline {
                return Err(IonQError::Timeout {
                    job_id: job_id.to_string(),
                    elapsed_secs: start.elapsed().as_secs(),
                });
            }

            std::thread::sleep(interval);
            interval = Duration::from_secs_f64(
                (interval.as_secs_f64() * BACKOFF_MULTIPLIER).min(MAX_INTERVAL_SECS),
            );
        }
    }

    /// Fetch the current state of a job.
    pub fn get_job(&self, job_id: &str) -> Result<JobResponse, IonQError> {
        let url = format!("{}/jobs/{}", self.base_url, job_id);
        let resp = self
            .authed_get(&url)
            .send()
            .map_err(IonQError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IonQError::RestError {
                detail: format!("get job {} HTTP {}: {}", job_id, status, body),
            });
        }

        resp.json().map_err(IonQError::HttpError)
    }

    /// Fetch a probabilities histogram from the URL embedded in a `JobResponse`.
    ///
    /// The `url` may be a relative path (e.g. `/v0.4/jobs/{id}/results/probabilities`);
    /// in that case it is joined onto the scheme+host extracted from `base_url`.
    ///
    /// The response is a flat map: `{ "0": 0.49, "3": 0.51, ... }` where
    /// keys are decimal-encoded bitstrings and values are probabilities.
    ///
    /// Returns `(counts, total_shots)` where counts maps bitstring integers to
    /// approximate shot counts scaled by `total_shots`.
    pub fn get_probabilities_with_shots(
        &self,
        url: &str,
        total_shots: u32,
    ) -> Result<(BTreeMap<u64, u32>, u32), IonQError> {
        let full_url = resolve_url(&self.base_url, url);

        let resp = self
            .authed_get(&full_url)
            .send()
            .map_err(IonQError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IonQError::RestError {
                detail: format!("get probabilities HTTP {}: {}", status, body),
            });
        }

        // Real response: flat map {"0": 0.5, "3": 0.5}
        let probs: HashMap<String, f64> = resp.json().map_err(IonQError::HttpError)?;
        Ok(histogram_to_counts(&probs, total_shots))
    }

    /// List all backends available to this API key.
    pub fn list_backends(&self) -> Result<Vec<BackendInfo>, IonQError> {
        let url = format!("{}/backends", self.base_url);
        let resp = self
            .authed_get(&url)
            .send()
            .map_err(IonQError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IonQError::CalibrationError {
                detail: format!("list backends HTTP {}: {}", status, body),
            });
        }

        resp.json().map_err(|e| IonQError::CalibrationError {
            detail: format!("backend list parse error: {}", e),
        })
    }

    /// Fetch a specific characterization record.
    ///
    /// `backend` is the backend name (e.g. `"qpu.forte-1"`).
    /// `char_id` is the UUID from `BackendInfo.characterization_id`.
    pub fn get_characterization(
        &self,
        backend: &str,
        char_id: &str,
    ) -> Result<CharacterizationResponse, IonQError> {
        let url = format!(
            "{}/backends/{}/characterizations/{}",
            self.base_url, backend, char_id
        );
        let resp = self
            .authed_get(&url)
            .send()
            .map_err(IonQError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(IonQError::CalibrationError {
                detail: format!(
                    "get characterization for '{}' / '{}' HTTP {}: {}",
                    backend, char_id, status, body
                ),
            });
        }

        resp.json().map_err(|e| IonQError::CalibrationError {
            detail: format!(
                "characterization parse error for '{}' / '{}': {}",
                backend, char_id, e
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// URL resolution helper
// ---------------------------------------------------------------------------

/// Resolve a possibly-relative URL against a base URL.
///
/// If `url` starts with `/`, it is joined onto the origin (scheme + host + port)
/// extracted from `base_url`. Otherwise it is returned as-is.
///
/// Examples:
/// - `base_url = "https://api.ionq.co/v0.4"`, `url = "/v0.4/jobs/abc/results/probabilities"`
///   → `"https://api.ionq.co/v0.4/jobs/abc/results/probabilities"`
/// - `base_url = "http://127.0.0.1:9090"`, `url = "/v0.4/jobs/abc/results/probabilities"`
///   → `"http://127.0.0.1:9090/v0.4/jobs/abc/results/probabilities"`
fn resolve_url(base_url: &str, url: &str) -> String {
    if !url.starts_with('/') {
        return url.to_string();
    }
    // Extract origin = scheme + "://" + host[:port] by finding the first path
    // slash that follows the authority (i.e., the part after "://").
    let origin = if let Some(authority_start) = base_url.find("://") {
        let after_scheme = &base_url[authority_start + 3..];
        match after_scheme.find('/') {
            Some(slash_pos) => &base_url[..authority_start + 3 + slash_pos],
            None => base_url.trim_end_matches('/'),
        }
    } else {
        base_url.trim_end_matches('/')
    };
    format!("{}{}", origin, url)
}

// ---------------------------------------------------------------------------
// Result parsing helpers
// ---------------------------------------------------------------------------

/// Convert an IonQ probability map to a counts histogram.
///
/// IonQ returns probabilities (0.0–1.0) per bitstring. We multiply by
/// `total_shots` to recover approximate integer counts. The bitstring keys
/// are decimal-encoded integers (e.g. `"3"` = `0b11`).
///
/// Returns `(counts, total_shots)`.
pub fn histogram_to_counts(
    probs: &HashMap<String, f64>,
    total_shots: u32,
) -> (BTreeMap<u64, u32>, u32) {
    let mut counts = BTreeMap::new();

    for (key, prob) in probs {
        if let Ok(bits) = key.parse::<u64>() {
            let count = (prob * total_shots as f64).round() as u32;
            if count > 0 {
                *counts.entry(bits).or_insert(0) += count;
            }
        }
    }

    (counts, total_shots)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- histogram_to_counts ------------------------------------------------

    #[test]
    fn test_histogram_to_counts_uniform() {
        let mut probs = HashMap::new();
        probs.insert("0".to_string(), 0.5);
        probs.insert("3".to_string(), 0.5);
        let (counts, total) = histogram_to_counts(&probs, 1000);
        assert_eq!(total, 1000);
        assert_eq!(counts[&0], 500);
        assert_eq!(counts[&3], 500);
    }

    #[test]
    fn test_histogram_to_counts_shots_zero_produces_empty() {
        // When shots=0, all counts round to 0 and are omitted.
        let mut probs = HashMap::new();
        probs.insert("0".to_string(), 0.5);
        probs.insert("3".to_string(), 0.5);
        let (counts, total) = histogram_to_counts(&probs, 0);
        assert_eq!(total, 0);
        assert!(counts.is_empty(), "zero shots → no counts");
    }

    #[test]
    fn test_histogram_to_counts_rounding() {
        // 0.499 * 10 = 4.99 → rounds to 5, not 4.
        let mut probs = HashMap::new();
        probs.insert("0".to_string(), 0.499);
        probs.insert("1".to_string(), 0.501);
        let (counts, _) = histogram_to_counts(&probs, 10);
        // 4.99 rounds to 5, 5.01 rounds to 5.
        assert_eq!(counts[&0] + counts[&1], 10);
    }

    #[test]
    fn test_histogram_to_counts_empty() {
        let probs = HashMap::new();
        let (counts, total) = histogram_to_counts(&probs, 100);
        assert_eq!(total, 100);
        assert!(counts.is_empty());
    }

    #[test]
    fn test_histogram_skips_zero_counts() {
        let mut probs = HashMap::new();
        probs.insert("0".to_string(), 0.0);
        probs.insert("1".to_string(), 1.0);
        let (counts, _) = histogram_to_counts(&probs, 10);
        assert!(!counts.contains_key(&0), "zero-count entry should be omitted");
        assert_eq!(counts[&1], 10);
    }

    #[test]
    fn test_histogram_ignores_non_integer_keys() {
        let mut probs = HashMap::new();
        probs.insert("abc".to_string(), 0.5);
        probs.insert("2".to_string(), 0.5);
        let (counts, total) = histogram_to_counts(&probs, 100);
        assert_eq!(total, 100);
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[&2], 50);
    }

    // ---- resolve_url --------------------------------------------------------

    #[test]
    fn test_resolve_url_relative_path() {
        let result = resolve_url(
            "https://api.ionq.co/v0.4",
            "/v0.4/jobs/abc-123/results/probabilities",
        );
        assert_eq!(
            result,
            "https://api.ionq.co/v0.4/jobs/abc-123/results/probabilities"
        );
    }

    #[test]
    fn test_resolve_url_absolute_unchanged() {
        let result = resolve_url(
            "https://api.ionq.co/v0.4",
            "https://other.host.com/path",
        );
        assert_eq!(result, "https://other.host.com/path");
    }

    #[test]
    fn test_resolve_url_trailing_slash_in_base() {
        let result = resolve_url("https://api.ionq.co/v0.4/", "/v0.4/jobs/x");
        assert_eq!(result, "https://api.ionq.co/v0.4/jobs/x");
    }

    #[test]
    fn test_resolve_url_host_port_no_path() {
        // Mockito-style base URL with no path component.
        let result = resolve_url(
            "http://127.0.0.1:9090",
            "/v0.4/jobs/abc/results/probabilities",
        );
        assert_eq!(result, "http://127.0.0.1:9090/v0.4/jobs/abc/results/probabilities");
    }

    // ---- JobSubmitPayload serialization ------------------------------------

    #[test]
    fn test_job_submit_payload_serializes() {
        let input = serde_json::json!({
            "gateset": "qis",
            "qubits": 2,
            "circuit": [{"gate": "v", "target": 0}]
        });
        let payload = JobSubmitPayload {
            type_: "ionq.circuit.v1".to_string(),
            backend: "simulator".to_string(),
            shots: 1024,
            input,
            name: None,
        };
        let json: serde_json::Value = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["type"], "ionq.circuit.v1");
        assert_eq!(json["backend"], "simulator");
        assert_eq!(json["shots"], 1024);
        assert_eq!(json["input"]["gateset"], "qis");
        assert!(
            !json.as_object().unwrap().contains_key("name"),
            "None fields should be omitted"
        );
    }

    #[test]
    fn test_job_submit_payload_with_name() {
        let payload = JobSubmitPayload {
            type_: "ionq.circuit.v1".to_string(),
            backend: "simulator".to_string(),
            shots: 100,
            input: serde_json::json!({}),
            name: Some("my-job".to_string()),
        };
        let json: serde_json::Value = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["name"], "my-job");
    }

    // ---- JobSubmitResponse deserialization (real shape) -------------------

    #[test]
    fn test_job_submit_response_real_shape() {
        // Real response: {"id": "...", "status": "submitted", "session_id": null}
        let raw = r#"{"id": "019db075-c147-7224-aeaa-fbf75ce776cd", "status": "submitted", "session_id": null}"#;
        let resp: JobSubmitResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.id, "019db075-c147-7224-aeaa-fbf75ce776cd");
        assert_eq!(resp.status, "submitted");
    }

    // ---- JobResponse deserialization (real shape) -------------------------

    #[test]
    fn test_job_response_real_completed_shape() {
        // Stripped-down version of the real completed job response.
        let raw = r#"{
            "id": "019db075-c147-7224-aeaa-fbf75ce776cd",
            "type": "ionq.circuit.v1",
            "status": "completed",
            "name": "cqam-bell-test",
            "metadata": null,
            "backend": "simulator",
            "submitter_id": "618cad11-bd3b-45d1-a889-cf315b0ee1f6",
            "project_id": "33382be3-1cde-40be-bd9a-1dccf3ef377e",
            "parent_job_id": null,
            "session_id": null,
            "dry_run": false,
            "failure": null,
            "noise": {"model": "ideal"},
            "submitted_at": "2026-04-21T14:33:20.458Z",
            "started_at": "2026-04-21T14:33:22.425Z",
            "completed_at": "2026-04-21T14:33:22.627Z",
            "execution_duration_ms": 0,
            "output": {"compilation": {"opt": 1}},
            "stats": {"qubits": 2, "circuits": 1},
            "results": {
                "probabilities": {
                    "url": "/v0.4/jobs/019db075-c147-7224-aeaa-fbf75ce776cd/results/probabilities"
                }
            }
        }"#;
        let job: JobResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(job.id, "019db075-c147-7224-aeaa-fbf75ce776cd");
        assert_eq!(job.status, "completed");
        assert!(job.shots.is_none(), "shots not echoed in completed response");
        assert!(job.failure.is_none());
        let probs_url = job.results.unwrap().probabilities.unwrap();
        assert_eq!(
            probs_url.url,
            "/v0.4/jobs/019db075-c147-7224-aeaa-fbf75ce776cd/results/probabilities"
        );
    }

    #[test]
    fn test_job_response_submitted_shape() {
        let raw = r#"{"id": "xyz", "status": "submitted", "session_id": null}"#;
        let job: JobResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(job.status, "submitted");
        assert!(job.results.is_none());
        assert!(job.failure.is_none());
    }

    #[test]
    fn test_job_response_failed_shape() {
        let raw = r#"{
            "id": "xyz-999",
            "status": "failed",
            "failure": {
                "code": "InvalidCircuit",
                "message": "unsupported gate in circuit"
            }
        }"#;
        let job: JobResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(job.status, "failed");
        let failure = job.failure.unwrap();
        assert_eq!(failure.code.as_deref(), Some("InvalidCircuit"));
        assert_eq!(failure.message.as_deref(), Some("unsupported gate in circuit"));
    }

    #[test]
    fn test_job_response_null_failure_field() {
        // Real API sends "failure": null for non-failed jobs.
        let raw = r#"{"id": "c1", "status": "completed", "failure": null,
                       "results": {"probabilities": {"url": "/v0.4/jobs/c1/results/probabilities"}}}"#;
        let job: JobResponse = serde_json::from_str(raw).unwrap();
        assert!(job.failure.is_none());
    }

    // ---- BackendInfo deserialization (real shapes) -------------------------

    #[test]
    fn test_backend_info_real_simulator_shape() {
        // Real simulator entry — no characterization_id, has noise_models.
        let raw = r#"{
            "backend": "simulator",
            "status": "available",
            "degraded": false,
            "qubits": 29,
            "average_queue_time": 0,
            "last_updated": "2026-04-21T14:30:27Z",
            "noise_models": ["aria-1", "forte-1", "forte-enterprise-1", "ideal"]
        }"#;
        let info: BackendInfo = serde_json::from_str(raw).unwrap();
        assert_eq!(info.backend, "simulator");
        assert_eq!(info.status.as_deref(), Some("available"));
        assert_eq!(info.qubits, Some(29));
        assert!(info.characterization_id.is_none(), "simulator has no characterization_id");
        assert_eq!(
            info.noise_models.as_ref().map(|v| v.len()),
            Some(4)
        );
    }

    #[test]
    fn test_backend_info_real_qpu_shape() {
        // Real QPU entry — has characterization_id, no noise_models.
        let raw = r#"{
            "backend": "qpu.forte-1",
            "status": "available",
            "degraded": false,
            "qubits": 36,
            "average_queue_time": 12190512,
            "last_updated": "2026-04-21T14:30:27Z",
            "characterization_id": "ffbc9da9-96cc-4f39-8715-ec6f038327d3"
        }"#;
        let info: BackendInfo = serde_json::from_str(raw).unwrap();
        assert_eq!(info.backend, "qpu.forte-1");
        assert_eq!(info.qubits, Some(36));
        assert_eq!(
            info.characterization_id.as_deref(),
            Some("ffbc9da9-96cc-4f39-8715-ec6f038327d3")
        );
    }

    #[test]
    fn test_backend_info_unknown_fields_ignored() {
        // Real forte-enterprise-1 response has an unknown "kw" field.
        let raw = r#"{
            "backend": "qpu.forte-enterprise-1",
            "status": "unavailable",
            "degraded": false,
            "qubits": 36,
            "average_queue_time": 6870772,
            "last_updated": "2026-04-21T14:30:27Z",
            "characterization_id": "68f9b822-76b3-4d82-b86d-67001d281d11",
            "kw": 4.88141
        }"#;
        let info: BackendInfo = serde_json::from_str(raw).unwrap();
        assert_eq!(info.backend, "qpu.forte-enterprise-1");
        assert_eq!(info.status.as_deref(), Some("unavailable"));
    }

    // ---- CharacterizationResponse deserialization (real shape) ------------

    #[test]
    fn test_characterization_response_real_shape() {
        // Matches the real forte-1 characterization response (condensed).
        let raw = r#"{
            "id": "ffbc9da9-96cc-4f39-8715-ec6f038327d3",
            "date": "2026-04-20T00:00:00Z",
            "backend": "qpu.forte-1",
            "qubits": 36,
            "fidelity": {
                "spam": {"median": 0.9942, "stderr": null},
                "1q":   {"median": 0.9998, "stderr": 0},
                "2q":   {"median": 0.9952, "stderr": 0}
            },
            "timing": {
                "readout": 0,
                "reset":   0,
                "1q":      0,
                "2q":      0,
                "t1":      100,
                "t2":      1
            }
        }"#;
        let char_resp: CharacterizationResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(char_resp.id, "ffbc9da9-96cc-4f39-8715-ec6f038327d3");
        assert_eq!(char_resp.backend, "qpu.forte-1");
        assert_eq!(char_resp.qubits, 36);
        assert_eq!(char_resp.date.as_deref(), Some("2026-04-20T00:00:00Z"));

        let fidelity = char_resp.fidelity.unwrap();
        assert!((fidelity.spam.as_ref().unwrap().median.unwrap() - 0.9942).abs() < 1e-9);
        assert!((fidelity.single_qubit.as_ref().unwrap().median.unwrap() - 0.9998).abs() < 1e-9);
        assert!((fidelity.two_qubit.as_ref().unwrap().median.unwrap() - 0.9952).abs() < 1e-9);

        let timing = char_resp.timing.unwrap();
        assert!((timing.t1.unwrap() - 100.0).abs() < 1e-9);
        assert!((timing.t2.unwrap() - 1.0).abs() < 1e-9);
        // Gate times are 0 in the real forte-1 response; verify we parse them.
        assert_eq!(timing.single_qubit, Some(0.0));
        assert_eq!(timing.two_qubit, Some(0.0));
    }

    #[test]
    fn test_characterization_response_minimal() {
        let raw = r#"{"id": "x", "backend": "simulator", "qubits": 29}"#;
        let char_resp: CharacterizationResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(char_resp.qubits, 29);
        assert!(char_resp.fidelity.is_none());
        assert!(char_resp.timing.is_none());
        assert!(char_resp.date.is_none());
    }

    #[test]
    fn test_char_timing_field_aliases() {
        let raw = r#"{"t1": 50.0, "t2": 0.5, "1q": 1.0e-4, "2q": 2.0e-4, "readout": 0, "reset": 0}"#;
        let timing: CharTiming = serde_json::from_str(raw).unwrap();
        assert!((timing.t1.unwrap() - 50.0).abs() < 1e-9);
        assert!((timing.single_qubit.unwrap() - 1.0e-4).abs() < 1e-15);
        assert!((timing.two_qubit.unwrap() - 2.0e-4).abs() < 1e-15);
    }

    #[test]
    fn test_char_fidelity_field_aliases() {
        let raw = r#"{
            "spam": {"median": 0.999},
            "1q":   {"median": 0.9998},
            "2q":   {"median": 0.995}
        }"#;
        let fid: CharFidelity = serde_json::from_str(raw).unwrap();
        assert!((fid.spam.as_ref().unwrap().median.unwrap() - 0.999).abs() < 1e-9);
        assert!((fid.single_qubit.as_ref().unwrap().median.unwrap() - 0.9998).abs() < 1e-9);
        assert!((fid.two_qubit.as_ref().unwrap().median.unwrap() - 0.995).abs() < 1e-9);
    }

    // ---- IonQRestClient construction ----------------------------------------

    #[test]
    fn test_client_default_url() {
        let client = IonQRestClient::new("test_key", "simulator");
        assert_eq!(client.base_url, "https://api.ionq.co/v0.4");
        assert_eq!(client.backend, "simulator");
    }

    #[test]
    fn test_auth_header_format() {
        let client = IonQRestClient::new("my_api_key", "simulator");
        assert_eq!(client.auth_header(), "apiKey my_api_key");
    }

    #[test]
    fn test_client_with_base_url() {
        let client =
            IonQRestClient::with_base_url("key", "qpu.forte-1", "http://localhost:9090");
        assert_eq!(client.base_url, "http://localhost:9090");
        assert_eq!(client.backend, "qpu.forte-1");
    }

    // ---- Mock HTTP tests (require network socket, use mockito) --------------

    #[cfg(test)]
    mod mock_http {
        use super::*;

        fn simulator_backend_json() -> &'static str {
            r#"[
                {
                    "backend": "simulator",
                    "status": "available",
                    "degraded": false,
                    "qubits": 29,
                    "average_queue_time": 0,
                    "last_updated": "2026-04-21T00:00:00Z",
                    "noise_models": ["ideal", "forte-1"]
                },
                {
                    "backend": "qpu.forte-1",
                    "status": "available",
                    "degraded": false,
                    "qubits": 36,
                    "average_queue_time": 12000000,
                    "last_updated": "2026-04-21T00:00:00Z",
                    "characterization_id": "ffbc9da9-96cc-4f39-8715-ec6f038327d3"
                }
            ]"#
        }

        fn forte1_characterization_json() -> &'static str {
            r#"{
                "id": "ffbc9da9-96cc-4f39-8715-ec6f038327d3",
                "date": "2026-04-20T00:00:00Z",
                "backend": "qpu.forte-1",
                "qubits": 36,
                "fidelity": {
                    "spam": {"median": 0.9942, "stderr": null},
                    "1q":   {"median": 0.9998, "stderr": 0},
                    "2q":   {"median": 0.9952, "stderr": 0}
                },
                "timing": {
                    "readout": 0, "reset": 0,
                    "1q": 0, "2q": 0,
                    "t1": 100, "t2": 1
                }
            }"#
        }

        #[test]
        fn test_mock_list_backends() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/backends")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(simulator_backend_json())
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url());
            let backends = client.list_backends().unwrap();

            assert_eq!(backends.len(), 2);
            assert_eq!(backends[0].backend, "simulator");
            assert_eq!(backends[0].qubits, Some(29));
            assert!(backends[0].characterization_id.is_none());
            assert_eq!(backends[1].backend, "qpu.forte-1");
            assert_eq!(
                backends[1].characterization_id.as_deref(),
                Some("ffbc9da9-96cc-4f39-8715-ec6f038327d3")
            );
        }

        #[test]
        fn test_mock_get_characterization() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock(
                    "GET",
                    "/backends/qpu.forte-1/characterizations/ffbc9da9-96cc-4f39-8715-ec6f038327d3",
                )
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(forte1_characterization_json())
                .create();

            let client = IonQRestClient::with_base_url("key", "qpu.forte-1", server.url());
            let char_resp = client
                .get_characterization("qpu.forte-1", "ffbc9da9-96cc-4f39-8715-ec6f038327d3")
                .unwrap();

            assert_eq!(char_resp.backend, "qpu.forte-1");
            assert_eq!(char_resp.qubits, 36);
            let spam = char_resp
                .fidelity.unwrap().spam.unwrap().median.unwrap();
            assert!((spam - 0.9942).abs() < 1e-9);
            assert!((char_resp.timing.unwrap().t1.unwrap() - 100.0).abs() < 1e-9);
        }

        #[test]
        fn test_mock_submit_job() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("POST", "/jobs")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{"id": "test-job-001", "status": "submitted", "session_id": null}"#)
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url());
            let circuit_json = serde_json::json!({
                "gateset": "qis",
                "qubits": 2,
                "circuit": [{"gate": "v", "target": 0}, {"gate": "cnot", "control": 0, "target": 1}]
            });
            let job_id = client.submit_job(circuit_json, 1024).unwrap();
            assert_eq!(job_id, "test-job-001");
        }

        #[test]
        fn test_mock_get_job_completed() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/jobs/test-job-001")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{
                    "id": "test-job-001",
                    "type": "ionq.circuit.v1",
                    "status": "completed",
                    "name": "cqam-bell-test",
                    "metadata": null,
                    "backend": "simulator",
                    "failure": null,
                    "results": {
                        "probabilities": {
                            "url": "/v0.4/jobs/test-job-001/results/probabilities"
                        }
                    }
                }"#)
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url());
            let job = client.get_job("test-job-001").unwrap();

            assert_eq!(job.status, "completed");
            assert!(job.failure.is_none());
            let probs_url = job.results.unwrap().probabilities.unwrap();
            assert_eq!(probs_url.url, "/v0.4/jobs/test-job-001/results/probabilities");
        }

        #[test]
        fn test_mock_get_probabilities_bell_state() {
            // Probabilities endpoint returns a flat map — {"0": 0.5, "3": 0.5}
            // for a Bell state on 2 qubits.
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/v0.4/jobs/test-job-001/results/probabilities")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{"0": 0.5, "3": 0.5}"#)
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url());
            // The URL from the API is relative: /v0.4/jobs/.../results/probabilities
            let url = "/v0.4/jobs/test-job-001/results/probabilities";
            let (counts, total) = client.get_probabilities_with_shots(url, 1024).unwrap();

            assert_eq!(total, 1024);
            assert_eq!(counts[&0], 512); // |00⟩
            assert_eq!(counts[&3], 512); // |11⟩
            assert_eq!(counts.len(), 2);
        }

        #[test]
        fn test_mock_full_bell_circuit_flow() {
            // End-to-end mock: submit → poll (submitted) → poll (completed) → probabilities.
            let mut server = mockito::Server::new();

            let _submit_mock = server
                .mock("POST", "/jobs")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{"id": "bell-job-42", "status": "submitted", "session_id": null}"#)
                .create();

            // First poll: still running
            let _poll1 = server
                .mock("GET", "/jobs/bell-job-42")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{"id": "bell-job-42", "status": "submitted", "failure": null}"#)
                .expect(1)
                .create();

            // Second poll: completed
            let _poll2 = server
                .mock("GET", "/jobs/bell-job-42")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{
                    "id": "bell-job-42",
                    "status": "completed",
                    "failure": null,
                    "results": {
                        "probabilities": {"url": "/v0.4/jobs/bell-job-42/results/probabilities"}
                    }
                }"#)
                .expect(1)
                .create();

            let _probs_mock = server
                .mock("GET", "/v0.4/jobs/bell-job-42/results/probabilities")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{"0": 0.5, "3": 0.5}"#)
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url())
                .with_poll_timeout(10);

            let circuit_json = serde_json::json!({
                "gateset": "qis", "qubits": 2,
                "circuit": [{"gate": "v", "target": 0}, {"gate": "cnot", "control": 0, "target": 1}]
            });

            let job_id = client.submit_job(circuit_json, 1024).unwrap();
            assert_eq!(job_id, "bell-job-42");

            let result = client
                .poll_until_done(&job_id, Some(Duration::from_millis(1)), None)
                .unwrap();
            assert_eq!(result.status, "completed");

            let probs_url = result.results.unwrap().probabilities.unwrap();
            let (counts, total) = client
                .get_probabilities_with_shots(&probs_url.url, 1024)
                .unwrap();

            assert_eq!(total, 1024);
            assert_eq!(counts[&0], 512);
            assert_eq!(counts[&3], 512);
        }

        #[test]
        fn test_mock_failed_job_propagates_error() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/jobs/bad-job")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{
                    "id": "bad-job",
                    "status": "failed",
                    "failure": {
                        "code": "InvalidCircuit",
                        "message": "unsupported gate: foo"
                    }
                }"#)
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url())
                .with_poll_timeout(5);

            let err = client
                .poll_until_done("bad-job", Some(Duration::from_millis(1)), None)
                .unwrap_err();

            match err {
                IonQError::UnexpectedStatus { status, .. } => {
                    assert!(status.contains("failed"));
                    assert!(status.contains("unsupported gate: foo"));
                }
                other => panic!("expected UnexpectedStatus, got {:?}", other),
            }
        }

        #[test]
        fn test_mock_canceled_job_propagates_error() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/jobs/canceled-job")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{
                    "id": "canceled-job",
                    "status": "canceled",
                    "failure": {"code": "UserCanceled", "message": "canceled by user"}
                }"#)
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url())
                .with_poll_timeout(5);
            let err = client
                .poll_until_done("canceled-job", Some(Duration::from_millis(1)), None)
                .unwrap_err();

            match err {
                IonQError::UnexpectedStatus { status, .. } => {
                    assert!(status.contains("canceled"));
                    assert!(status.contains("canceled by user"));
                }
                other => panic!("expected UnexpectedStatus, got {:?}", other),
            }
        }

        #[test]
        fn test_mock_auth_header_sent() {
            // Verify the Authorization header is actually sent with the correct format.
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/backends")
                .match_header("Authorization", "apiKey my-secret-key")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body("[]")
                .create();

            let client = IonQRestClient::with_base_url("my-secret-key", "simulator", server.url());
            let backends = client.list_backends().unwrap();
            assert!(backends.is_empty());
        }

        #[test]
        fn test_mock_get_job_not_found() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/jobs/missing-job")
                .with_status(404)
                .with_body(r#"{"error": "not found"}"#)
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url());
            let err = client.get_job("missing-job").unwrap_err();
            match err {
                IonQError::RestError { detail } => assert!(detail.contains("404")),
                other => panic!("expected RestError, got {:?}", other),
            }
        }

        #[test]
        fn test_mock_list_backends_error_response() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/backends")
                .with_status(401)
                .with_body(r#"{"error": "Unauthorized"}"#)
                .create();

            let client = IonQRestClient::with_base_url("bad_key", "simulator", server.url());
            assert!(client.list_backends().is_err());
        }

        #[test]
        fn test_mock_submit_job_error_response() {
            let mut server = mockito::Server::new();
            let _mock = server
                .mock("POST", "/jobs")
                .with_status(400)
                .with_body(r#"{"error": "invalid circuit"}"#)
                .create();

            let client = IonQRestClient::with_base_url("key", "simulator", server.url());
            let result = client.submit_job(serde_json::json!({}), 100);
            assert!(result.is_err());
        }
    }
}
