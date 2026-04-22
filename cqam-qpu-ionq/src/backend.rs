//! `IonQQpuBackend` — implements `cqam_qpu::traits::QpuBackend` for IonQ hardware.
//!
//! Workflow:
//! 1. `compile`: validate the `native_ir::Circuit` qubit count against the device
//!    and do a dry-run circuit serialization to catch unsupported ops early.
//! 2. `submit`: convert circuit → IonQ v0.4 native JSON → POST to IonQ Cloud REST
//!    API, then poll for completion using adaptive Bayesian shot convergence.
//!    Results are fetched from the separate probabilities URL in the job response.
//! 3. `poll_results`: GET job status/results endpoint, fetch probabilities if done.
//! 4. `calibration`: return a snapshot of `IonQCalibrationData`.
//!
//! IonQ trapped-ion hardware has all-to-all qubit connectivity, so the
//! `cqam-micro` routing stage inserts zero SWAPs for IonQ targets.
//! We declare `NativeGateSet::Superconducting` so that `cqam-micro` emits
//! the {SX, X, Rz, CX} gate set, which maps cleanly to the IonQ QIS gateset.

use std::collections::BTreeMap;

use cqam_core::error::CqamError;
use cqam_core::native_ir::{self, NativeGateSet};
use cqam_qpu::estimator::BayesianEstimator;
use cqam_qpu::traits::{
    CalibrationData, ConnectivityGraph, ConvergenceCriterion, QpuBackend, QpuMetrics, RawResults,
};

use crate::calibration::IonQCalibrationData;
use crate::circuit::circuit_to_ionq_json;
use crate::error::IonQError;
use crate::rest::IonQRestClient;

// ---------------------------------------------------------------------------
// IonQ device presets
// ---------------------------------------------------------------------------

/// Return the qubit count for a known IonQ device name, or `None` if unknown.
fn qubit_count_for_target(target: &str) -> Option<u32> {
    match target {
        "simulator" | "simulator.statevector" | "simulator.density-matrix" => Some(29),
        "qpu.forte-1" | "qpu.forte-enterprise-1" => Some(36),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// IonQQpuBackend
// ---------------------------------------------------------------------------

/// IonQ QPU backend.
///
/// Submits native JSON circuits to the IonQ Cloud API v0.4 and polls for
/// results. Authentication uses a simple API key — no token exchange needed.
#[derive(Debug, Clone)]
pub struct IonQQpuBackend {
    /// IonQ target device (e.g. `"qpu.forte-1"`, `"simulator"`).
    target: String,
    /// Physical qubit count of the target device.
    num_qubits: u32,
    /// All-to-all connectivity graph (no routing constraints).
    connectivity: ConnectivityGraph,
    /// REST client for job submission and polling.
    rest: IonQRestClient,
    /// Calibration snapshot.
    calibration: IonQCalibrationData,
}

impl IonQQpuBackend {
    /// Construct a backend for a known IonQ device.
    ///
    /// `api_key` is your IonQ Cloud API key.
    /// `target` is the device name (e.g. `"qpu.forte-1"`, `"simulator"`).
    ///
    /// # Errors
    ///
    /// Returns `IonQError::RestError` if `target` is unrecognized and the
    /// qubit count cannot be determined without a network call. Use
    /// `from_device` to auto-discover qubit counts from the API.
    pub fn new(
        api_key: impl Into<String>,
        target: impl Into<String>,
    ) -> Result<Self, IonQError> {
        let api_key = api_key.into();
        let target = target.into();

        let num_qubits = qubit_count_for_target(&target).ok_or_else(|| {
            IonQError::RestError {
                detail: format!(
                    "unknown target '{}'. Use from_device() for auto-discovery, \
                     or provide num_qubits explicitly via new_with_qubits().",
                    target
                ),
            }
        })?;

        let connectivity = ConnectivityGraph::all_to_all(num_qubits);
        let rest = IonQRestClient::new(api_key, &target);
        let calibration = IonQCalibrationData::synthetic(num_qubits);

        Ok(Self { target, num_qubits, connectivity, rest, calibration })
    }

    /// Construct a backend with an explicit qubit count (bypasses preset lookup).
    pub fn new_with_qubits(
        api_key: impl Into<String>,
        target: impl Into<String>,
        num_qubits: u32,
    ) -> Self {
        let api_key = api_key.into();
        let target = target.into();
        let connectivity = ConnectivityGraph::all_to_all(num_qubits);
        let rest = IonQRestClient::new(api_key, &target);
        let calibration = IonQCalibrationData::synthetic(num_qubits);
        Self { target, num_qubits, connectivity, rest, calibration }
    }

    /// Construct a backend by querying the IonQ API for device characterization.
    ///
    /// Lists available backends, selects the matching one, then fetches full
    /// calibration data (T1/T2, gate timing, SPAM fidelity) from the
    /// `/backends/{target}/characterizations/{id}` endpoint.
    pub fn from_device(
        api_key: impl Into<String>,
        target: impl Into<String>,
    ) -> Result<Self, IonQError> {
        let api_key = api_key.into();
        let target = target.into();
        let rest = IonQRestClient::new(&api_key, &target);
        Self::from_device_with_rest(target, rest)
    }

    /// Like `from_device` but uses a caller-supplied REST client.
    /// Allows tests to inject a mock base URL without network access.
    fn from_device_with_rest(target: String, rest: IonQRestClient) -> Result<Self, IonQError> {
        let backends = rest.list_backends()?;
        let info = backends
            .iter()
            .find(|b| b.backend == target)
            .ok_or_else(|| IonQError::RestError {
                detail: format!(
                    "backend '{}' not found in IonQ backend list",
                    target
                ),
            })?;

        let num_qubits = info
            .qubits
            .or_else(|| qubit_count_for_target(&target))
            .unwrap_or_else(|| {
                tracing::warn!(
                    target = %target,
                    "qubit count not available from API or presets; defaulting to 36"
                );
                36
            });

        let connectivity = ConnectivityGraph::all_to_all(num_qubits);

        let calibration = if let Some(char_id) = &info.characterization_id {
            match rest.get_characterization(&target, char_id) {
                Ok(char_resp) => {
                    IonQCalibrationData::from_characterization_response(&char_resp)
                }
                Err(e) => {
                    tracing::warn!(
                        target = %target,
                        char_id = %char_id,
                        error = %e,
                        "characterization fetch failed; using synthetic calibration"
                    );
                    IonQCalibrationData::synthetic(num_qubits)
                }
            }
        } else {
            IonQCalibrationData::synthetic(num_qubits)
        };

        Ok(Self { target, num_qubits, connectivity, rest, calibration })
    }

    /// Set the job polling timeout in seconds.
    pub fn with_poll_timeout(mut self, secs: u64) -> Self {
        self.rest = self.rest.with_poll_timeout(secs);
        self
    }

    /// Replace the calibration snapshot with externally-fetched data.
    pub fn with_calibration(mut self, cal: IonQCalibrationData) -> Self {
        self.calibration = cal;
        self
    }

    /// Fetch fresh calibration data from the IonQ API and update the snapshot.
    pub fn refresh_calibration(&mut self) -> Result<(), IonQError> {
        let backends = self.rest.list_backends()?;
        let info = backends
            .iter()
            .find(|b| b.backend == self.target)
            .ok_or_else(|| IonQError::RestError {
                detail: format!(
                    "backend '{}' not found in IonQ backend list",
                    self.target
                ),
            })?;

        if let Some(char_id) = &info.characterization_id {
            let char_resp = self.rest.get_characterization(&self.target, char_id)?;
            self.calibration = IonQCalibrationData::from_characterization_response(&char_resp);
        }

        Ok(())
    }
}

impl QpuBackend for IonQQpuBackend {
    fn gate_set(&self) -> &NativeGateSet {
        // Declare Superconducting so cqam-micro emits {SX, X, Rz, CX}, which
        // maps directly to IonQ's QIS gateset: {v, x, rz, cnot}.
        &NativeGateSet::Superconducting
    }

    fn connectivity(&self) -> &ConnectivityGraph {
        &self.connectivity
    }

    fn max_qubits(&self) -> u32 {
        self.num_qubits
    }

    fn compile(&self, circuit: &native_ir::Circuit) -> Result<(), CqamError> {
        if circuit.num_physical_qubits > self.num_qubits {
            return Err(CqamError::QpuQubitAllocationFailed {
                required: circuit.num_physical_qubits,
                available: self.num_qubits,
            });
        }
        // Dry-run JSON serialization to catch unsupported ops early.
        let circuit_json = circuit_to_ionq_json(circuit).map_err(CqamError::from)?;
        // IonQ API rejects circuits with an empty gate list. Catch this here
        // rather than letting the API return an opaque error at submission time.
        // Note: Identity, Measure, Reset, and Barrier ops are omitted from the
        // serialized form, so a circuit composed entirely of those will also fail.
        if circuit_json["circuit"]
            .as_array()
            .map_or(true, |a| a.is_empty())
        {
            return Err(CqamError::QpuSubmissionFailed {
                provider: "IonQ".to_string(),
                detail: "circuit contains no gate operations after serialization; \
                         IonQ requires at least one gate (Measure/Reset/Barrier are \
                         omitted from the IonQ QIS format)"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Submit `circuit` to IonQ and collect results using adaptive Bayesian shot convergence.
    ///
    /// Runs up to `shot_budget / convergence.min_batch_size` batches, stopping early
    /// if the estimator converges. Returns accumulated counts across all batches.
    ///
    /// **Fail-fast / partial-batch discard**: if any batch fails (network error, API
    /// rejection, or polling timeout), the entire call returns an error. Results from
    /// any previously completed batches in the same call are discarded. Callers that
    /// need partial results should use `poll_results` directly.
    ///
    /// **Circuit validation**: `submit` does not re-run the full `compile` checks.
    /// Call `compile` first to catch qubit-count overflows and empty gate lists before
    /// the first network round-trip.
    fn submit(
        &mut self,
        circuit: &native_ir::Circuit,
        convergence: &ConvergenceCriterion,
        shot_budget: u32,
    ) -> Result<RawResults, CqamError> {
        if shot_budget == 0 {
            return Err(CqamError::QpuSubmissionFailed {
                provider: "IonQ".to_string(),
                detail: "shot_budget must be > 0".to_string(),
            });
        }
        if circuit.num_physical_qubits > self.num_qubits {
            return Err(CqamError::QpuQubitAllocationFailed {
                required: circuit.num_physical_qubits,
                available: self.num_qubits,
            });
        }

        let circuit_json = circuit_to_ionq_json(circuit).map_err(CqamError::from)?;

        let mut estimator = BayesianEstimator::new(convergence.clone());
        let mut total_shots: u32 = 0;
        let mut remaining_budget = shot_budget;
        let mut batches = 0u32;

        loop {
            let batch_size = convergence.min_batch_size.min(remaining_budget);
            if batch_size == 0 {
                break;
            }

            let job_id = self
                .rest
                .submit_job(circuit_json.clone(), batch_size)
                .map_err(CqamError::from)?;
            let result = self
                .rest
                .poll_until_done(&job_id, None, None)
                .map_err(CqamError::from)?;

            // Guard against the API returning shots=0 on a completed job.
            // Without this, remaining_budget never decreases and the loop spins forever.
            let batch_shots = result.shots.filter(|&s| s > 0).unwrap_or(batch_size);
            let (batch_counts, _) = match result.results.and_then(|r| r.probabilities) {
                Some(probs_url) => self
                    .rest
                    .get_probabilities_with_shots(&probs_url.url, batch_shots)
                    .map_err(CqamError::from)?,
                None => (BTreeMap::new(), 0),
            };

            total_shots += batch_shots;
            remaining_budget = remaining_budget.saturating_sub(batch_shots);
            estimator.update(&batch_counts);
            batches += 1;

            if estimator.is_converged() || remaining_budget == 0 {
                break;
            }
        }

        tracing::debug!(batches, total_shots, "IonQ adaptive shot loop complete");

        Ok(RawResults {
            counts: estimator.finalize(),
            total_shots,
            metrics: QpuMetrics {
                shots_used: total_shots,
                circuit_depth: circuit.depth,
                swap_count: circuit.swap_count,
                physical_qubits_used: circuit.num_physical_qubits,
                estimated_fidelity: self.calibration.estimate_circuit_fidelity(circuit),
                ..QpuMetrics::default()
            },
        })
    }

    fn poll_results(&self, job_id: &str) -> Result<Option<RawResults>, CqamError> {
        let job = self
            .rest
            .get_job(job_id)
            .map_err(CqamError::from)?;

        match job.status.to_lowercase().as_str() {
            "completed" => {}
            "failed" | "canceled" => {
                let reason = job
                    .failure
                    .as_ref()
                    .and_then(|f| f.message.clone())
                    .or_else(|| job.failure.as_ref().and_then(|f| f.code.clone()))
                    .unwrap_or_default();
                return Err(IonQError::UnexpectedStatus {
                    job_id: job_id.to_string(),
                    status: format!("{}: {}", job.status, reason),
                }
                .into());
            }
            // submitted, ready, started → still in progress
            _ => return Ok(None),
        }

        let job_shots = job.shots.unwrap_or_else(|| {
            tracing::warn!(job_id, "completed job response missing shots field; defaulting to 1024");
            1024
        });
        let (counts, total_shots) = match job.results.and_then(|r| r.probabilities) {
            Some(probs_url) => self
                .rest
                .get_probabilities_with_shots(&probs_url.url, job_shots)
                .map_err(CqamError::from)?,
            None => (BTreeMap::new(), 0),
        };

        Ok(Some(RawResults {
            counts,
            total_shots,
            metrics: QpuMetrics {
                shots_used: total_shots,
                ..QpuMetrics::default()
            },
        }))
    }

    fn calibration(&self) -> Result<Box<dyn CalibrationData>, CqamError> {
        Ok(Box::new(self.calibration.clone()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::native_ir::{ApplyGate1q, Circuit, NativeGate1, Op, PhysicalQubit};

    fn make_backend() -> IonQQpuBackend {
        IonQQpuBackend::new_with_qubits("fake_key", "simulator", 29)
    }

    #[test]
    fn test_gate_set_is_superconducting() {
        let b = make_backend();
        assert_eq!(b.gate_set(), &NativeGateSet::Superconducting);
    }

    #[test]
    fn test_max_qubits() {
        let b = make_backend();
        assert_eq!(b.max_qubits(), 29);
    }

    #[test]
    fn test_connectivity_is_all_to_all() {
        let b = make_backend();
        assert!(b.connectivity().are_connected(0, 28));
        assert!(b.connectivity().are_connected(7, 13));
    }

    #[test]
    fn test_compile_too_many_qubits() {
        let b = make_backend();
        let big = Circuit::new(100);
        assert!(b.compile(&big).is_err());
    }

    #[test]
    fn test_compile_valid_circuit() {
        let b = make_backend();
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Sx,
        }));
        assert!(b.compile(&c).is_ok());
    }

    #[test]
    fn test_calibration_returns_correct_type() {
        let b = make_backend();
        let cal = b.calibration().unwrap();
        assert!(cal.t1(0) > 1.0, "trapped-ion T1 should be order of seconds");
    }

    #[test]
    fn test_new_with_known_targets() {
        for (target, expected_qubits) in &[
            ("simulator", 29u32),
            ("simulator.statevector", 29),
            ("simulator.density-matrix", 29),
            ("qpu.forte-1", 36),
            ("qpu.forte-enterprise-1", 36),
        ] {
            let b = IonQQpuBackend::new("key", *target).unwrap();
            assert_eq!(
                b.max_qubits(),
                *expected_qubits,
                "wrong qubit count for {target}"
            );
        }
    }

    #[test]
    fn test_new_unknown_target_returns_error() {
        assert!(IonQQpuBackend::new("key", "qpu.unknown-device").is_err());
    }

    #[test]
    fn test_new_with_qubits_bypasses_preset() {
        // Should succeed even for a target name not in the preset table.
        let b = IonQQpuBackend::new_with_qubits("key", "qpu.custom-device", 50);
        assert_eq!(b.max_qubits(), 50);
    }

    #[test]
    fn test_with_calibration_builder() {
        let cal = IonQCalibrationData::synthetic(36);
        let b = make_backend().with_calibration(cal);
        assert!(b.calibration().unwrap().t1(0) > 0.0);
    }

    #[test]
    fn test_compile_exactly_at_qubit_limit_succeeds() {
        let b = IonQQpuBackend::new_with_qubits("key", "simulator", 5);
        let mut c = Circuit::new(5); // exactly at limit
        c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::Sx }));
        assert!(b.compile(&c).is_ok());
    }

    #[test]
    fn test_compile_rejects_empty_circuit() {
        // A circuit with no gate ops produces "circuit": [] in the IonQ JSON format.
        // The IonQ API rejects this; compile() must surface a clear error before submission.
        let b = make_backend();
        let c = Circuit::new(1); // no ops
        let err = b.compile(&c).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("no gate operations"), "error must explain what's missing: {msg}");
    }

    #[test]
    fn test_compile_rejects_measure_only_circuit() {
        // Measure/Reset/Barrier ops are omitted from the IonQ QIS format, so a
        // circuit with only those ops produces an empty gate list and must also fail.
        let b = make_backend();
        let mut c = Circuit::new(1);
        c.ops.push(Op::Measure(cqam_core::native_ir::Observe { qubit: PhysicalQubit(0), clbit: 0 }));
        assert!(b.compile(&c).is_err(), "measure-only circuit must be rejected (empty gate list)");
    }

    // ---- mock HTTP tests for poll_results and submit -------------------------

    fn make_backend_at(server_url: &str) -> IonQQpuBackend {
        IonQQpuBackend {
            target: "simulator".to_string(),
            num_qubits: 29,
            connectivity: ConnectivityGraph::all_to_all(29),
            rest: crate::rest::IonQRestClient::with_base_url("key", "simulator", server_url),
            calibration: IonQCalibrationData::synthetic(29),
        }
    }

    #[test]
    fn test_poll_results_returns_none_for_in_progress() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/running-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "running-job", "status": "started", "failure": null}"#)
            .create();

        let backend = make_backend_at(&server.url());
        let result = backend.poll_results("running-job").unwrap();
        assert!(result.is_none(), "in-progress job must return None");
    }

    #[test]
    fn test_poll_results_returns_some_for_completed() {
        let mut server = mockito::Server::new();
        let _job_mock = server
            .mock("GET", "/jobs/done-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "done-job",
                "status": "completed",
                "shots": 1024,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/done-job/results/probabilities"}}
            }"#)
            .create();

        let _probs_mock = server
            .mock("GET", "/v0.4/jobs/done-job/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 0.5, "3": 0.5}"#)
            .create();

        let backend = make_backend_at(&server.url());
        let raw = backend.poll_results("done-job").unwrap().expect("completed job must return Some");
        assert_eq!(raw.total_shots, 1024);
        assert_eq!(raw.counts[&0], 512);
        assert_eq!(raw.counts[&3], 512);
    }

    #[test]
    fn test_poll_results_returns_error_for_failed_job() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/failed-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "failed-job",
                "status": "failed",
                "failure": {"code": "InvalidCircuit", "message": "bad gate"}
            }"#)
            .create();

        let backend = make_backend_at(&server.url());
        let err = backend.poll_results("failed-job").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("failed"), "error must mention 'failed': {msg}");
        assert!(msg.contains("bad gate"), "error must include failure message: {msg}");
    }

    #[test]
    fn test_poll_results_returns_none_for_submitted_status() {
        // "submitted" is an in-progress status — must return None, not an error.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/queued-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "queued-job", "status": "submitted", "failure": null}"#)
            .create();

        let backend = make_backend_at(&server.url());
        assert!(backend.poll_results("queued-job").unwrap().is_none());
    }

    #[test]
    fn test_poll_results_no_probabilities_url_returns_empty_counts() {
        // Completed job with no results block → return Some with empty counts.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/no-probs-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "no-probs-job", "status": "completed", "shots": 100}"#)
            .create();

        let backend = make_backend_at(&server.url());
        let raw = backend.poll_results("no-probs-job").unwrap().expect("should return Some");
        assert_eq!(raw.total_shots, 0);
        assert!(raw.counts.is_empty());
    }

    #[test]
    fn test_compile_one_over_qubit_limit_fails() {
        let b = IonQQpuBackend::new_with_qubits("key", "simulator", 5);
        let c = Circuit::new(6); // one over limit
        assert!(b.compile(&c).is_err());
    }

    #[test]
    fn test_refresh_calibration_updates_snapshot() {
        // Verifies that refresh_calibration() replaces the in-memory calibration
        // snapshot with values fetched from the API (T1 changes from 100 → 50).
        let mut server = mockito::Server::new();

        let _backends_mock = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "qpu.forte-1",
                "status": "available",
                "qubits": 36,
                "characterization_id": "char-refresh-id"
            }]"#)
            .create();

        let _char_mock = server
            .mock(
                "GET",
                "/backends/qpu.forte-1/characterizations/char-refresh-id",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "char-refresh-id",
                "backend": "qpu.forte-1",
                "qubits": 36,
                "fidelity": {
                    "spam": {"median": 0.999},
                    "1q":   {"median": 0.9995},
                    "2q":   {"median": 0.993}
                },
                "timing": {"t1": 50.0, "t2": 0.8, "1q": 0, "2q": 0}
            }"#)
            .create();

        let mut backend = IonQQpuBackend {
            target: "qpu.forte-1".to_string(),
            num_qubits: 36,
            connectivity: ConnectivityGraph::all_to_all(36),
            rest: crate::rest::IonQRestClient::with_base_url(
                "key",
                "qpu.forte-1",
                &server.url(),
            ),
            calibration: IonQCalibrationData::synthetic(36), // T1 = 1000.0
        };

        // Sanity-check pre-refresh value.
        assert!((backend.calibration().unwrap().t1(0) - 1000.0).abs() < 1e-9);

        backend.refresh_calibration().unwrap();

        // After refresh T1 must come from the mock characterization (50.0).
        assert!(
            (backend.calibration().unwrap().t1(0) - 50.0).abs() < 1e-9,
            "T1 should update to 50.0 after refresh"
        );
    }

    #[test]
    fn test_submit_zero_shot_budget_returns_error() {
        let mut b = make_backend();
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default();
        let err = b.submit(&c, &convergence, 0).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("shot_budget"),
            "error should mention shot_budget, got: {msg}"
        );
    }

    #[test]
    fn test_submit_circuit_qubit_overflow_returns_error() {
        let mut b = IonQQpuBackend::new_with_qubits("key", "simulator", 5);
        // Circuit needs 6 qubits but backend only has 5.
        let c = Circuit::new(6);
        let convergence = ConvergenceCriterion::default();
        assert!(b.submit(&c, &convergence, 100).is_err());
    }

    #[test]
    fn test_with_poll_timeout_propagates_to_rest() {
        let b = make_backend().with_poll_timeout(42);
        assert_eq!(b.rest.poll_timeout_secs, 42);
    }

    #[test]
    fn test_submit_single_batch_returns_results() {
        // Full submit() path: POST /jobs → poll → GET probabilities.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "submit-job-1", "status": "submitted", "session_id": null}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/submit-job-1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "submit-job-1",
                "status": "completed",
                "shots": 256,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/submit-job-1/results/probabilities"}}
            }"#)
            .create();

        let _probs = server
            .mock("GET", "/v0.4/jobs/submit-job-1/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 0.5, "3": 0.5}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::Sx }));

        // ConvergenceCriterion::default() has min_batch_size=256; submit 256 shots.
        let convergence = ConvergenceCriterion::default();
        let raw = backend.submit(&c, &convergence, 256).unwrap();

        assert_eq!(raw.total_shots, 256);
        assert_eq!(raw.counts[&0], 128);
        assert_eq!(raw.counts[&3], 128);
        assert_eq!(raw.metrics.shots_used, 256);
    }

    #[test]
    fn test_submit_failed_job_propagates_error() {
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "fail-job-1", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/fail-job-1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "fail-job-1",
                "status": "failed",
                "failure": {"code": "InvalidCircuit", "message": "bad gate"}
            }"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default();
        assert!(backend.submit(&c, &convergence, 256).is_err());
    }

    #[test]
    fn test_poll_results_canceled_returns_error() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/canceled-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "canceled-job",
                "status": "canceled",
                "failure": {"code": "UserCanceled", "message": "user canceled"}
            }"#)
            .create();

        let backend = make_backend_at(&server.url());
        let err = backend.poll_results("canceled-job").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("canceled"), "got: {msg}");
    }

    #[test]
    fn test_poll_results_missing_shots_falls_back_to_1024() {
        // Completed job with no `shots` field in the response — should default to 1024.
        let mut server = mockito::Server::new();

        let _job = server
            .mock("GET", "/jobs/no-shots-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "no-shots-job",
                "status": "completed",
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/no-shots-job/results/probabilities"}}
            }"#)
            .create();

        let _probs = server
            .mock("GET", "/v0.4/jobs/no-shots-job/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 0.5, "3": 0.5}"#)
            .create();

        let backend = make_backend_at(&server.url());
        let raw = backend.poll_results("no-shots-job").unwrap().unwrap();
        // Default of 1024 shots → each 0.5 prob outcome gets 512 counts.
        assert_eq!(raw.total_shots, 1024);
        assert_eq!(raw.counts[&0], 512);
        assert_eq!(raw.counts[&3], 512);
    }

    #[test]
    fn test_from_device_happy_path() {
        // from_device() with a QPU backend that has a characterization_id.
        let mut server = mockito::Server::new();

        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "qpu.forte-1",
                "status": "available",
                "qubits": 36,
                "characterization_id": "char-abc-123"
            }]"#)
            .create();

        let _char = server
            .mock("GET", "/backends/qpu.forte-1/characterizations/char-abc-123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "char-abc-123",
                "backend": "qpu.forte-1",
                "qubits": 36,
                "fidelity": {
                    "spam": {"median": 0.999},
                    "1q":   {"median": 0.9995},
                    "2q":   {"median": 0.993}
                },
                "timing": {"t1": 80.0, "t2": 0.9, "1q": 0, "2q": 0}
            }"#)
            .create();

        let backend = IonQQpuBackend {
            target: "qpu.forte-1".to_string(),
            num_qubits: 36,
            connectivity: ConnectivityGraph::all_to_all(36),
            rest: crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url()),
            calibration: IonQCalibrationData::synthetic(36),
        };

        // Simulate what from_device() does: list backends then get characterization.
        let backends = backend.rest.list_backends().unwrap();
        let info = backends.iter().find(|b| b.backend == "qpu.forte-1").unwrap();
        let char_id = info.characterization_id.as_deref().unwrap();
        let char_resp = backend.rest.get_characterization("qpu.forte-1", char_id).unwrap();
        let cal = IonQCalibrationData::from_characterization_response(&char_resp);

        assert_eq!(info.qubits, Some(36));
        assert!((cal.t1(0) - 80.0).abs() < 1e-9, "T1 should be 80 from API");
        assert!((cal.t2(0) - 0.9).abs() < 1e-9, "T2 should be 0.9 from API");
    }

    #[test]
    fn test_from_device_no_characterization_id_uses_synthetic() {
        // from_device() for the simulator — no characterization_id, so synthetic cal.
        let mut server = mockito::Server::new();

        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "simulator",
                "status": "available",
                "qubits": 29
            }]"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "simulator", &server.url());
        let backends = rest.list_backends().unwrap();
        let info = backends.iter().find(|b| b.backend == "simulator").unwrap();

        assert!(info.characterization_id.is_none());
        assert_eq!(info.qubits, Some(29));
        // No characterization_id → synthetic cal would be used.
        let cal = IonQCalibrationData::synthetic(29);
        assert!((cal.t1(0) - 1000.0).abs() < 1e-9, "synthetic T1 is 1000s");
    }

    #[test]
    fn test_poll_results_ready_status_returns_none() {
        // "ready" is a documented in-progress status — must return None, not an error.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/ready-poll-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "ready-poll-job", "status": "ready", "failure": null}"#)
            .create();

        let backend = make_backend_at(&server.url());
        assert!(
            backend.poll_results("ready-poll-job").unwrap().is_none(),
            "'ready' must return None (still in progress)"
        );
    }

    #[test]
    fn test_poll_results_started_status_returns_none() {
        // "started" is a documented in-progress status — must return None, not an error.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/started-poll-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "started-poll-job", "status": "started", "failure": null}"#)
            .create();

        let backend = make_backend_at(&server.url());
        assert!(
            backend.poll_results("started-poll-job").unwrap().is_none(),
            "'started' must return None (still in progress)"
        );
    }

    #[test]
    fn test_poll_results_unknown_status_returns_none() {
        // Any unrecognized status (not completed/failed/canceled) must return None.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/weird-status-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "weird-status-job", "status": "pending_review", "failure": null}"#)
            .create();

        let backend = make_backend_at(&server.url());
        assert!(
            backend.poll_results("weird-status-job").unwrap().is_none(),
            "unknown status must return None (treated as in-progress)"
        );
    }

    #[test]
    fn test_from_device_list_backends_http_error_propagates() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/backends")
            .with_status(503)
            .with_body(r#"{"error": "Service Unavailable"}"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "simulator", &server.url());
        let err = IonQQpuBackend::from_device_with_rest("simulator".to_string(), rest).unwrap_err();
        // list_backends HTTP error → CalibrationError or RestError
        let msg = format!("{err}");
        assert!(!msg.is_empty(), "error must have a message");
    }

    #[test]
    fn test_from_device_empty_backends_list_returns_not_found() {
        // When the API returns an empty backend list, from_device_with_rest must fail
        // with "not found" — the target doesn't appear in an empty list.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "simulator", &server.url());
        let err = IonQQpuBackend::from_device_with_rest("simulator".to_string(), rest).unwrap_err();
        match err {
            IonQError::RestError { detail } => {
                assert!(detail.contains("simulator"), "error must name the missing target: {detail}");
            }
            other => panic!("expected RestError, got {:?}", other),
        }
    }

    #[test]
    fn test_from_device_unavailable_backend_still_creates() {
        // Backend status="unavailable" is informational — from_device must still
        // create the backend object; status does not gate construction.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "qpu.forte-1",
                "status": "unavailable",
                "degraded": false,
                "qubits": 36
            }]"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url());
        let backend = IonQQpuBackend::from_device_with_rest("qpu.forte-1".to_string(), rest)
            .expect("unavailable backend must still be constructible");
        assert_eq!(backend.max_qubits(), 36);
    }

    #[test]
    fn test_refresh_calibration_list_backends_http_error_propagates() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/backends")
            .with_status(500)
            .with_body(r#"{"error": "Internal Server Error"}"#)
            .create();

        let mut backend = IonQQpuBackend {
            target: "qpu.forte-1".to_string(),
            num_qubits: 36,
            connectivity: ConnectivityGraph::all_to_all(36),
            rest: crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url()),
            calibration: IonQCalibrationData::synthetic(36),
        };

        assert!(
            backend.refresh_calibration().is_err(),
            "list_backends HTTP error must propagate from refresh_calibration"
        );
    }

    #[test]
    fn test_submit_probabilities_http_error_propagates() {
        // If get_probabilities_with_shots() fails mid-loop, submit() must propagate the error.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "probs-fail-job", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/probs-fail-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "probs-fail-job",
                "status": "completed",
                "shots": 100,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/probs-fail-job/results/probabilities"}}
            }"#)
            .create();

        let _probs = server
            .mock("GET", "/v0.4/jobs/probs-fail-job/results/probabilities")
            .with_status(500)
            .with_body(r#"{"error": "server error"}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default();
        assert!(
            backend.submit(&c, &convergence, 100).is_err(),
            "probabilities HTTP error must propagate from submit()"
        );
    }

    #[test]
    fn test_compile_zero_qubit_circuit_rejected() {
        // A 0-qubit circuit produces "circuit": [] — no gate operations — and is
        // rejected by compile() with the same empty-gate-list error.
        let b = IonQQpuBackend::new_with_qubits("key", "simulator", 29);
        let c = Circuit::new(0);
        assert!(b.compile(&c).is_err(), "zero-qubit circuit has no gates and must be rejected");
    }

    #[test]
    fn test_new_with_qubits_single_qubit_device() {
        // 1-qubit device: minimum meaningful configuration.
        let b = IonQQpuBackend::new_with_qubits("key", "qpu.tiny", 1);
        assert_eq!(b.max_qubits(), 1);
        let mut c = Circuit::new(1);
        c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::X }));
        assert!(b.compile(&c).is_ok());
        assert!(b.compile(&Circuit::new(2)).is_err());
    }

    #[test]
    fn test_qubit_count_for_target_unknown_and_case_sensitive() {
        // Unknown target returns None; matching is case-sensitive.
        assert!(qubit_count_for_target("qpu.future-device").is_none());
        assert!(qubit_count_for_target("SIMULATOR").is_none(), "lookup must be case-sensitive");
        assert!(qubit_count_for_target("Simulator").is_none());
        assert!(qubit_count_for_target("QPU.FORTE-1").is_none());
        // Known target still works.
        assert_eq!(qubit_count_for_target("simulator"), Some(29));
        assert_eq!(qubit_count_for_target("qpu.forte-enterprise-1"), Some(36));
    }

    #[test]
    fn test_poll_results_uppercase_completed_status() {
        // IonQ API returns uppercase or mixed-case "COMPLETED" — code must lowercase before matching.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/upper-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "upper-job", "status": "COMPLETED", "shots": 100}"#)
            .create();

        let backend = make_backend_at(&server.url());
        let raw = backend
            .poll_results("upper-job")
            .unwrap()
            .expect("COMPLETED status must return Some regardless of case");
        // No results block → empty counts, but total_shots=0
        assert_eq!(raw.total_shots, 0);
        assert!(raw.counts.is_empty());
    }

    #[test]
    fn test_poll_results_failed_with_null_failure_field() {
        // "failure": null — both code and message are None → reason falls back to "".
        // Must not panic; error message should still identify the job as failed.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/failed-null")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "failed-null", "status": "failed", "failure": null}"#)
            .create();

        let backend = make_backend_at(&server.url());
        let err = backend.poll_results("failed-null").unwrap_err();
        assert!(
            matches!(err, CqamError::QpuSubmissionFailed { .. }),
            "null failure field must produce QpuSubmissionFailed, not a panic: {err:?}"
        );
        assert!(format!("{err}").contains("failed"), "error must mention 'failed': {err}");
    }

    #[test]
    fn test_refresh_calibration_target_not_in_list_returns_error() {
        let mut server = mockito::Server::new();
        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"backend": "other-device", "qubits": 10}]"#)
            .create();

        let mut backend = IonQQpuBackend {
            target: "qpu.forte-1".to_string(),
            num_qubits: 36,
            connectivity: ConnectivityGraph::all_to_all(36),
            rest: crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url()),
            calibration: IonQCalibrationData::synthetic(36),
        };

        let err = backend.refresh_calibration().unwrap_err();
        match err {
            IonQError::RestError { detail } => {
                assert!(detail.contains("qpu.forte-1"), "error must name the missing target: {detail}");
            }
            other => panic!("expected RestError, got {:?}", other),
        }
    }

    #[test]
    fn test_refresh_calibration_characterization_error_propagates() {
        // Unlike from_device_with_rest (which falls back to synthetic), refresh_calibration
        // has no fallback — it must propagate the characterization fetch error.
        let mut server = mockito::Server::new();
        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "qpu.forte-1",
                "qubits": 36,
                "characterization_id": "bad-char-id"
            }]"#)
            .create();
        let _char_fail = server
            .mock("GET", "/backends/qpu.forte-1/characterizations/bad-char-id")
            .with_status(403)
            .with_body(r#"{"error": "Forbidden"}"#)
            .create();

        let mut backend = IonQQpuBackend {
            target: "qpu.forte-1".to_string(),
            num_qubits: 36,
            connectivity: ConnectivityGraph::all_to_all(36),
            rest: crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url()),
            calibration: IonQCalibrationData::synthetic(36),
        };

        assert!(
            backend.refresh_calibration().is_err(),
            "characterization fetch failure must propagate as an error"
        );
    }

    #[test]
    fn test_from_device_characterization_minimal_response_uses_defaults() {
        // Characterization endpoint returns only required fields (no fidelity, no timing).
        // from_device_with_rest must succeed and use calibration defaults.
        let mut server = mockito::Server::new();

        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "qpu.forte-1",
                "status": "available",
                "qubits": 36,
                "characterization_id": "minimal-char"
            }]"#)
            .create();

        let _char = server
            .mock("GET", "/backends/qpu.forte-1/characterizations/minimal-char")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "minimal-char", "backend": "qpu.forte-1", "qubits": 36}"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url());
        let backend = IonQQpuBackend::from_device_with_rest("qpu.forte-1".to_string(), rest)
            .expect("from_device_with_rest must succeed with minimal characterization data");

        assert_eq!(backend.max_qubits(), 36);
        let cal = backend.calibration().unwrap();
        // No timing → defaults: T1=1000, T2=1.
        assert!((cal.t1(0) - 1000.0).abs() < 1e-9, "T1 must fall back to 1000: {}", cal.t1(0));
        assert!((cal.t2(0) - 1.0).abs() < 1e-9, "T2 must fall back to 1: {}", cal.t2(0));
    }

    #[test]
    fn test_from_device_no_qubits_field_falls_back_to_preset() {
        // Backend info missing the qubits field → qubit_count_for_target() preset is used.
        let mut server = mockito::Server::new();
        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"backend": "simulator", "status": "available"}]"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "simulator", &server.url());
        let backend = IonQQpuBackend::from_device_with_rest("simulator".to_string(), rest).unwrap();
        assert_eq!(backend.max_qubits(), 29, "preset must supply qubit count when API omits it");
    }

    #[test]
    fn test_from_device_no_qubits_no_preset_defaults_to_36() {
        // Neither API nor preset provides a qubit count → hard-coded fallback of 36.
        let mut server = mockito::Server::new();
        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"backend": "qpu.future-device", "status": "available"}]"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "qpu.future-device", &server.url());
        let backend = IonQQpuBackend::from_device_with_rest("qpu.future-device".to_string(), rest).unwrap();
        assert_eq!(backend.max_qubits(), 36, "ultimate fallback must be 36");
    }

    #[test]
    fn test_submit_job_missing_shots_field_falls_back_to_batch_size() {
        // When the completed job response omits "shots", the loop uses batch_size instead.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "no-shots-loop", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/no-shots-loop")
            .with_status(200)
            .with_header("content-type", "application/json")
            // "shots" field deliberately absent
            .with_body(r#"{"id": "no-shots-loop", "status": "completed", "failure": null}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default(); // min_batch_size=100
        let raw = backend.submit(&c, &convergence, 100).unwrap();

        // batch_size = min(100, 100) = 100; result.shots = None → batch_shots = 100
        assert_eq!(raw.total_shots, 100, "batch_size must be used when shots is absent");
        assert!(raw.counts.is_empty(), "no probabilities URL → empty counts");
    }

    #[test]
    fn test_submit_multi_batch_accumulates_shots() {
        // budget=200, min_batch_size=100 (default) → exactly 2 batches.
        // Bell-state distribution (50/50) does not converge in 100 shots at
        // default confidence=0.95 / max_relative_error=0.05 settings, so the
        // loop must run a second batch before remaining_budget hits zero.
        let mut server = mockito::Server::new();

        // LIFO: create fallback mock first, high-priority mock second.
        let _post_b = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "batch-job-b", "status": "submitted"}"#)
            .expect(1)  // second batch — must be called exactly once
            .create();

        let _post_a = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "batch-job-a", "status": "submitted"}"#)
            .expect(1)
            .create();

        let _poll_a = server
            .mock("GET", "/jobs/batch-job-a")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "batch-job-a",
                "status": "completed",
                "shots": 100,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/batch-job-a/results/probabilities"}}
            }"#)
            .create();

        let _probs_a = server
            .mock("GET", "/v0.4/jobs/batch-job-a/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 0.5, "3": 0.5}"#)
            .create();

        let _poll_b = server
            .mock("GET", "/jobs/batch-job-b")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "batch-job-b",
                "status": "completed",
                "shots": 100,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/batch-job-b/results/probabilities"}}
            }"#)
            .create();

        let _probs_b = server
            .mock("GET", "/v0.4/jobs/batch-job-b/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 0.5, "3": 0.5}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::Sx }));

        let convergence = ConvergenceCriterion::default(); // min_batch_size=100
        let raw = backend.submit(&c, &convergence, 200).unwrap();

        assert_eq!(raw.total_shots, 200, "both batches must be counted");
        assert_eq!(raw.counts[&0], 100, "|00⟩ counts must accumulate across batches");
        assert_eq!(raw.counts[&3], 100, "|11⟩ counts must accumulate across batches");
        assert_eq!(raw.metrics.shots_used, 200);
    }

    #[test]
    fn test_submit_metrics_fields_populated() {
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "metrics-job", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/metrics-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "metrics-job",
                "status": "completed",
                "shots": 100,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/metrics-job/results/probabilities"}}
            }"#)
            .create();

        let _probs = server
            .mock("GET", "/v0.4/jobs/metrics-job/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 0.5, "3": 0.5}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::Sx }));

        let convergence = ConvergenceCriterion::default();
        let raw = backend.submit(&c, &convergence, 100).unwrap();

        assert_eq!(raw.metrics.physical_qubits_used, 2);
        assert!(
            raw.metrics.estimated_fidelity > 0.0 && raw.metrics.estimated_fidelity <= 1.0,
            "fidelity must be in (0, 1]: got {}",
            raw.metrics.estimated_fidelity
        );
        assert_eq!(raw.metrics.shots_used, raw.total_shots);
    }

    #[test]
    fn test_submit_no_probabilities_url_in_loop() {
        // Completed job with no results block → empty counts but shots still counted.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "no-probs-loop-job", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/no-probs-loop-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "no-probs-loop-job", "status": "completed", "shots": 100}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default();
        let raw = backend.submit(&c, &convergence, 100).unwrap();

        assert_eq!(raw.total_shots, 100, "shots from response header must be counted");
        assert!(raw.counts.is_empty(), "no probabilities URL → empty counts");
    }

    #[test]
    fn test_from_device_via_mock() {
        // Exercise from_device_with_rest() end-to-end: list backends → get characterization.
        let mut server = mockito::Server::new();

        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "qpu.forte-1",
                "status": "available",
                "qubits": 36,
                "characterization_id": "char-fd-123"
            }]"#)
            .create();

        let _char = server
            .mock("GET", "/backends/qpu.forte-1/characterizations/char-fd-123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "char-fd-123",
                "backend": "qpu.forte-1",
                "qubits": 36,
                "fidelity": {
                    "spam": {"median": 0.999},
                    "1q":   {"median": 0.9995},
                    "2q":   {"median": 0.993}
                },
                "timing": {"t1": 80.0, "t2": 0.9, "1q": 0, "2q": 0}
            }"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url());
        let backend = IonQQpuBackend::from_device_with_rest("qpu.forte-1".to_string(), rest)
            .expect("from_device_with_rest should succeed");

        assert_eq!(backend.max_qubits(), 36);
        let cal = backend.calibration().unwrap();
        assert!((cal.t1(0) - 80.0).abs() < 1e-9, "T1 must come from characterization: {}", cal.t1(0));
        assert!((cal.t2(0) - 0.9).abs() < 1e-9);
    }

    #[test]
    fn test_from_device_target_not_found_returns_error() {
        let mut server = mockito::Server::new();

        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"backend": "simulator", "qubits": 29}]"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "qpu.missing", &server.url());
        let err = IonQQpuBackend::from_device_with_rest("qpu.missing".to_string(), rest)
            .unwrap_err();

        match err {
            IonQError::RestError { detail } => {
                assert!(detail.contains("qpu.missing"), "error must name the missing target: {detail}");
            }
            other => panic!("expected RestError, got {:?}", other),
        }
    }

    #[test]
    fn test_from_device_characterization_fails_falls_back_to_synthetic() {
        // Characterization endpoint returns 500 → backend still created with synthetic cal.
        let mut server = mockito::Server::new();

        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "qpu.forte-1",
                "status": "available",
                "qubits": 36,
                "characterization_id": "failing-char-id"
            }]"#)
            .create();

        let _char_fail = server
            .mock("GET", "/backends/qpu.forte-1/characterizations/failing-char-id")
            .with_status(500)
            .with_body(r#"{"error": "internal server error"}"#)
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url());
        let backend = IonQQpuBackend::from_device_with_rest("qpu.forte-1".to_string(), rest)
            .expect("from_device_with_rest must succeed even when characterization fails");

        assert_eq!(backend.max_qubits(), 36);
        let cal = backend.calibration().unwrap();
        // Synthetic defaults: T1=1000s, T2=1s.
        assert!((cal.t1(0) - 1000.0).abs() < 1e-9, "should use synthetic T1=1000: got {}", cal.t1(0));
        assert!((cal.t2(0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_refresh_calibration_no_characterization_id_is_noop() {
        // If the backend has no characterization_id, refresh_calibration is a no-op.
        let mut server = mockito::Server::new();

        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"backend": "simulator", "status": "available", "qubits": 29}]"#)
            .create();

        let mut backend = IonQQpuBackend {
            target: "simulator".to_string(),
            num_qubits: 29,
            connectivity: ConnectivityGraph::all_to_all(29),
            rest: crate::rest::IonQRestClient::with_base_url("key", "simulator", &server.url()),
            calibration: IonQCalibrationData::synthetic(29),
        };

        let t1_before = backend.calibration().unwrap().t1(0);
        backend.refresh_calibration().unwrap();
        let t1_after = backend.calibration().unwrap().t1(0);

        assert_eq!(t1_before, t1_after, "no characterization_id → calibration unchanged");
    }

    #[test]
    fn test_poll_results_probabilities_http_error_propagates() {
        // The probabilities fetch in poll_results() must propagate HTTP errors.
        // This is a distinct code path from the identical branch in submit() —
        // both must be independently tested.
        let mut server = mockito::Server::new();

        let _job = server
            .mock("GET", "/jobs/pr-probs-err")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "pr-probs-err",
                "status": "completed",
                "shots": 100,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/pr-probs-err/results/probabilities"}}
            }"#)
            .create();

        let _probs = server
            .mock("GET", "/v0.4/jobs/pr-probs-err/results/probabilities")
            .with_status(500)
            .with_body(r#"{"error": "internal server error"}"#)
            .create();

        let backend = make_backend_at(&server.url());
        assert!(
            backend.poll_results("pr-probs-err").is_err(),
            "probabilities HTTP 500 must propagate as an error from poll_results()"
        );
    }

    #[test]
    fn test_submit_second_batch_post_fails_propagates() {
        // First batch completes successfully; second POST /jobs returns 500.
        // The error must propagate — partial counts from batch 1 are discarded.
        // budget=200, min_batch_size=100 → 2 batches; Bell 50/50 in 100 shots
        // does not converge (relative_error ≈ 0.19 >> 0.05), so the loop
        // always attempts a second batch.
        let mut server = mockito::Server::new();

        // LIFO: fallback (second POST, 500) registered first.
        let _post_b = server
            .mock("POST", "/jobs")
            .with_status(500)
            .with_body(r#"{"error": "quota exceeded"}"#)
            .expect(1)  // second POST must actually be attempted
            .create();

        let _post_a = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "sb-job-a", "status": "submitted"}"#)
            .expect(1)
            .create();

        let _poll_a = server
            .mock("GET", "/jobs/sb-job-a")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "sb-job-a",
                "status": "completed",
                "shots": 100,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/sb-job-a/results/probabilities"}}
            }"#)
            .create();

        let _probs_a = server
            .mock("GET", "/v0.4/jobs/sb-job-a/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 0.5, "3": 0.5}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::Sx }));

        let convergence = ConvergenceCriterion::default(); // min_batch_size=100
        assert!(
            backend.submit(&c, &convergence, 200).is_err(),
            "second-batch POST 500 must propagate; partial results must not be returned"
        );
    }

    #[test]
    fn test_from_device_characterization_parse_error_uses_synthetic() {
        // Characterization endpoint returns HTTP 200 but with a non-JSON body.
        // The parse failure is treated the same as an HTTP error — the fallback
        // arm in from_device_with_rest uses synthetic calibration.
        let mut server = mockito::Server::new();

        let _backends = server
            .mock("GET", "/backends")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{
                "backend": "qpu.forte-1",
                "status": "available",
                "qubits": 36,
                "characterization_id": "parse-fail-char"
            }]"#)
            .create();

        let _char = server
            .mock("GET", "/backends/qpu.forte-1/characterizations/parse-fail-char")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("this is not json at all")
            .create();

        let rest = crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url());
        let backend = IonQQpuBackend::from_device_with_rest("qpu.forte-1".to_string(), rest)
            .expect("JSON parse failure in characterization must fall back to synthetic");

        assert_eq!(backend.max_qubits(), 36);
        let cal = backend.calibration().unwrap();
        assert!(
            (cal.t1(0) - 1000.0).abs() < 1e-9,
            "parse-error fallback must yield synthetic T1=1000; got {}",
            cal.t1(0)
        );
    }

    #[test]
    fn test_backend_clone_is_independent() {
        // IonQQpuBackend derives Clone. Mutations on the clone must not affect
        // the original — specifically, with_poll_timeout on the clone must not
        // change the original's poll_timeout_secs.
        let original = make_backend();
        let clone = original.clone().with_poll_timeout(999);

        assert_eq!(clone.rest.poll_timeout_secs, 999, "clone poll timeout must be 999");
        assert_eq!(
            original.rest.poll_timeout_secs,
            14400,
            "original poll timeout must remain 14400 (default)"
        );
    }

    #[test]
    fn test_submit_request_body_contains_correct_backend() {
        // The POST /jobs body must contain the correct "backend" value.
        // A bug that passes the wrong device name to submit_job would cause the
        // circuit to run on the wrong hardware — this guards against that.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .match_body(mockito::Matcher::Regex(r#""backend"\s*:\s*"qpu\.forte-1""#.to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "body-check-job", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/body-check-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "body-check-job", "status": "completed", "shots": 100}"#)
            .create();

        let mut backend = IonQQpuBackend {
            target: "qpu.forte-1".to_string(),
            num_qubits: 36,
            connectivity: ConnectivityGraph::all_to_all(36),
            rest: crate::rest::IonQRestClient::with_base_url("key", "qpu.forte-1", &server.url()),
            calibration: IonQCalibrationData::synthetic(36),
        };

        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default();
        backend.submit(&c, &convergence, 100).unwrap();
        // If the mock didn't match, mockito returns 501 and submit() would error.
    }

    #[test]
    fn test_submit_failed_job_code_only_uses_code_as_reason() {
        // Exercises rest.rs:381-382: when poll_until_done() encounters a failed job
        // where failure.message is null but failure.code is present, it uses code
        // as the reason. This path is only reachable via submit() → poll_until_done().
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "code-reason-job", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/code-reason-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "code-reason-job",
                "status": "failed",
                "failure": {"code": "QubitOverflow", "message": null}
            }"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default();
        let err = backend.submit(&c, &convergence, 100).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("QubitOverflow"),
            "failure.code must surface as reason when message is null: {msg}"
        );
    }

    #[test]
    fn test_submit_api_returns_zero_shots_falls_back_to_batch_size() {
        // If the API returns "shots": 0 on a completed job, the loop must treat it
        // the same as a missing shots field (fall back to batch_size). Without the
        // filter(|&s| s > 0) guard, remaining_budget never decreases and submit()
        // loops forever submitting new jobs.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "zero-shots-job", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/zero-shots-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "zero-shots-job", "status": "completed", "shots": 0}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default();
        let raw = backend.submit(&c, &convergence, 100).unwrap();

        // shots=0 falls back to batch_size=100 → remaining_budget goes to 0 → loop exits.
        assert_eq!(raw.total_shots, 100, "shots=0 must fall back to batch_size; got {}", raw.total_shots);
    }

    #[test]
    fn test_compile_rz_nan_angle_returns_error() {
        // compile() runs a dry-run serialization and must catch non-finite Rz angles.
        let b = IonQQpuBackend::new_with_qubits("key", "simulator", 29);
        let mut c = Circuit::new(1);
        c.ops.push(Op::Gate1q(ApplyGate1q { qubit: PhysicalQubit(0), gate: NativeGate1::Rz(f64::NAN) }));
        assert!(b.compile(&c).is_err(), "compile() must reject Rz(NaN)");
    }

    #[test]
    fn test_compile_rz_infinity_angle_returns_error() {
        let b = IonQQpuBackend::new_with_qubits("key", "simulator", 29);
        let mut c = Circuit::new(1);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Rz(f64::INFINITY),
        }));
        assert!(b.compile(&c).is_err(), "compile() must reject Rz(+∞)");
    }

    #[test]
    fn test_submit_zero_min_batch_size_exits_immediately() {
        // ConvergenceCriterion::min_batch_size = 0 → batch_size = min(0, budget) = 0
        // → the `if batch_size == 0 { break }` guard fires on the first loop iteration
        // before any HTTP call, yielding empty results with zero shots.
        let mut backend = make_backend();
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion { min_batch_size: 0, ..ConvergenceCriterion::default() };
        let raw = backend.submit(&c, &convergence, 100).unwrap();
        assert_eq!(raw.total_shots, 0, "min_batch_size=0 must exit immediately with zero shots");
        assert!(raw.counts.is_empty());
    }

    #[test]
    fn test_poll_results_failure_code_used_when_message_absent() {
        // Exercises rest.rs:381-382: when failure.message is None but failure.code is Some,
        // the or_else fallback uses code as the reason string.
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/jobs/code-only-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "code-only-job",
                "status": "failed",
                "failure": {"code": "CircuitTooDeep", "message": null}
            }"#)
            .create();

        let backend = make_backend_at(&server.url());
        let err = backend.poll_results("code-only-job").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("CircuitTooDeep"),
            "failure.code must be used as reason when message is absent: {msg}"
        );
    }

    #[test]
    fn test_submit_at_exact_qubit_limit_succeeds() {
        // A circuit using exactly num_qubits qubits must pass the qubit check
        // (circuit.num_physical_qubits > self.num_qubits is false when equal).
        // Catches the `>` → `==` and `>` → `>=` mutations on line 251.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "exact-limit-job", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/exact-limit-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "exact-limit-job", "status": "completed", "shots": 100}"#)
            .create();

        let mut backend = IonQQpuBackend {
            target: "simulator".to_string(),
            num_qubits: 2,
            connectivity: ConnectivityGraph::all_to_all(2),
            rest: crate::rest::IonQRestClient::with_base_url("key", "simulator", &server.url()),
            calibration: IonQCalibrationData::synthetic(2),
        };

        // Circuit uses exactly 2 qubits on a 2-qubit backend — must not error.
        let c = Circuit::new(2);
        let convergence = ConvergenceCriterion::default();
        assert!(
            backend.submit(&c, &convergence, 100).is_ok(),
            "circuit at exact qubit limit must be accepted (> check, not >=)"
        );
    }

    #[test]
    fn test_submit_converges_early_stops_at_one_batch() {
        // A deterministic distribution (single outcome, probability 1.0) converges
        // after exactly min_batch_size shots via the single-outcome fast path in
        // BayesianEstimator::is_converged(). With budget=300 (3 possible batches),
        // the loop must break after the first batch due to the `||` in:
        //   if estimator.is_converged() || remaining_budget == 0
        // Catches the `||` → `&&` mutation on line 294.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "conv-job", "status": "submitted"}"#)
            .expect(1)  // must be called exactly once — no second batch
            .create();

        let _poll = server
            .mock("GET", "/jobs/conv-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "conv-job",
                "status": "completed",
                "shots": 100,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/conv-job/results/probabilities"}}
            }"#)
            .create();

        let _probs = server
            .mock("GET", "/v0.4/jobs/conv-job/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 1.0}"#)  // deterministic: all shots on outcome 0
            .expect(1)  // probabilities fetched exactly once — convergence stops the loop
            .create();

        let mut backend = make_backend_at(&server.url());
        let c = Circuit::new(1);
        let convergence = ConvergenceCriterion::default(); // min_batch_size=100

        // budget=300 allows up to 3 batches, but convergence after batch 1 must stop the loop.
        let raw = backend.submit(&c, &convergence, 300).unwrap();
        assert_eq!(raw.total_shots, 100, "converged after 1 batch; must not submit 2 or 3");
        assert_eq!(raw.counts[&0], 100);
    }

    #[test]
    fn test_submit_metrics_circuit_depth_and_swap_count_populated() {
        // Verifies that circuit.depth and circuit.swap_count are copied into
        // metrics.circuit_depth and metrics.swap_count in the submit() return value.
        // Catches the `delete field circuit_depth` and `delete field swap_count` mutations.
        let mut server = mockito::Server::new();

        let _submit = server
            .mock("POST", "/jobs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "depth-job", "status": "submitted"}"#)
            .create();

        let _poll = server
            .mock("GET", "/jobs/depth-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "depth-job", "status": "completed", "shots": 100}"#)
            .create();

        let mut backend = make_backend_at(&server.url());
        let mut c = Circuit::new(1);
        c.depth = 7;
        c.swap_count = 3;
        let convergence = ConvergenceCriterion::default();
        let raw = backend.submit(&c, &convergence, 100).unwrap();

        assert_eq!(raw.metrics.circuit_depth, 7, "metrics.circuit_depth must reflect circuit.depth");
        assert_eq!(raw.metrics.swap_count, 3, "metrics.swap_count must reflect circuit.swap_count");
    }

    #[test]
    fn test_poll_results_metrics_shots_used_populated() {
        // Verifies that metrics.shots_used is set in poll_results().
        // Catches the `delete field shots_used` mutation on line 356.
        let mut server = mockito::Server::new();

        let _job = server
            .mock("GET", "/jobs/pr-shots-used-job")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "pr-shots-used-job",
                "status": "completed",
                "shots": 512,
                "failure": null,
                "results": {"probabilities": {"url": "/v0.4/jobs/pr-shots-used-job/results/probabilities"}}
            }"#)
            .create();

        let _probs = server
            .mock("GET", "/v0.4/jobs/pr-shots-used-job/results/probabilities")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"0": 0.5, "3": 0.5}"#)
            .create();

        let backend = make_backend_at(&server.url());
        let raw = backend.poll_results("pr-shots-used-job").unwrap().unwrap();
        assert_eq!(
            raw.metrics.shots_used, 512,
            "poll_results metrics.shots_used must equal total_shots"
        );
        assert_eq!(raw.total_shots, 512);
    }

    #[test]
    fn test_calibration_box_dyn_trait_exact_synthetic_values() {
        // calibration() returns Box<dyn CalibrationData>. Verify all 7 exact
        // synthetic values through the trait object to catch any regression in
        // the dynamic dispatch path or in the synthetic() constructor.
        let b = IonQQpuBackend::new_with_qubits("key", "qpu.forte-1", 36);
        let cal: Box<dyn cqam_qpu::traits::CalibrationData> = b.calibration().unwrap();

        assert!((cal.t1(0) - 1000.0).abs() < 1e-12, "T1: got {}", cal.t1(0));
        assert!((cal.t2(0) - 1.0).abs() < 1e-12, "T2: got {}", cal.t2(0));
        assert!((cal.single_gate_error(0) - 6e-4).abs() < 1e-15, "1Q error: got {}", cal.single_gate_error(0));
        assert!((cal.two_gate_error(0, 1) - 6e-3).abs() < 1e-15, "2Q error: got {}", cal.two_gate_error(0, 1));
        assert!((cal.readout_error(0) - 3e-3).abs() < 1e-15, "readout error: got {}", cal.readout_error(0));
        assert!((cal.single_gate_time() - 1.35e-4).abs() < 1e-18, "1Q time: got {}", cal.single_gate_time());
        assert!((cal.two_gate_time() - 2.1e-4).abs() < 1e-18, "2Q time: got {}", cal.two_gate_time());
    }
}
