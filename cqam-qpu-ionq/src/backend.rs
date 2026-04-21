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
#[derive(Clone)]
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
        circuit_to_ionq_json(circuit).map(|_| ()).map_err(CqamError::from)
    }

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

            let batch_shots = result.shots.unwrap_or(batch_size);
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
        let c = Circuit::new(5); // exactly at limit
        assert!(b.compile(&c).is_ok());
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
        assert!(msg.contains("failed") || msg.contains("bad gate"), "got: {msg}");
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
            calibration: IonQCalibrationData::synthetic(36), // T1 = 100.0
        };

        // Sanity-check pre-refresh value.
        assert!((backend.calibration().unwrap().t1(0) - 100.0).abs() < 1e-9);

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
}
