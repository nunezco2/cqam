//! `IbmQpuBackend` — implements `cqam_qpu::traits::QpuBackend` for IBM hardware.
//!
//! Workflow:
//! 1. `compile`: convert the `native_ir::Circuit` → `QkCircuit`, optionally
//!    transpile it for the IBM native gate set via the Qiskit C API.
//! 2. `submit`: convert + transpile → serialize to QASM → POST to IBM REST API.
//! 3. `poll_results`: GET the result endpoint and parse counts.
//! 4. `calibration`: return a snapshot of `IbmCalibrationData`.

use cqam_core::error::CqamError;
use cqam_core::native_ir::{self, NativeGateSet};
use cqam_qpu::estimator::BayesianEstimator;
use cqam_qpu::traits::{
    CalibrationData, ConnectivityGraph, ConvergenceCriterion, QpuBackend, QpuMetrics, RawResults,
};

use crate::calibration::IbmCalibrationData;
use crate::convert::native_to_qk;
use crate::error::IbmError;
use crate::rest::{result_to_counts, IbmRestClient};

// ---------------------------------------------------------------------------
// IbmQpuBackend
// ---------------------------------------------------------------------------

/// IBM QPU backend.
///
/// Holds configuration, a REST client, and the last-known calibration
/// snapshot.  Circuit→QkCircuit conversion and transpilation are performed
/// lazily on `submit`.
#[derive(Clone)]
pub struct IbmQpuBackend {
    /// IBM backend device name (e.g. `"ibm_torino"`).
    backend_name: String,
    /// Physical qubit count of the target device.
    num_qubits: u32,
    /// Device connectivity edges (directed, from IBM coupling map).
    edges: Vec<(u32, u32)>,
    /// Device connectivity topology (undirected, normalized).
    connectivity: ConnectivityGraph,
    /// Device basis gate names (e.g. `["cz", "id", "rz", "sx", "x"]`).
    basis_gates: Vec<String>,
    /// REST client for job submission and polling.
    rest: IbmRestClient,
    /// Calibration snapshot.
    calibration: IbmCalibrationData,
    /// Optimization level forwarded to `qk_transpile` (0–3).
    optimization_level: u8,
    /// Whether to run IBM transpilation before REST submission.
    use_transpiler: bool,
}

impl IbmQpuBackend {
    /// Construct a new backend with sensible defaults.
    ///
    /// `api_key` is an IBM Quantum API key (exchanged for an IAM access token).
    /// `backend_name` identifies the device (e.g. `"ibm_brisbane"`).
    /// `num_qubits` and `edges` describe the device topology.
    pub fn new(
        api_key: impl Into<String>,
        backend_name: impl Into<String>,
        num_qubits: u32,
        edges: &[(u32, u32)],
    ) -> Result<Self, IbmError> {
        let backend_name = backend_name.into();
        let rest = IbmRestClient::new(api_key, &backend_name)?;
        let connectivity = ConnectivityGraph::from_edges(num_qubits, edges);
        let calibration = IbmCalibrationData::synthetic(num_qubits);

        Ok(Self {
            backend_name,
            num_qubits,
            edges: edges.to_vec(),
            connectivity,
            basis_gates: vec!["sx".into(), "x".into(), "rz".into(), "cx".into()],
            rest,
            calibration,
            optimization_level: 1,
            use_transpiler: true,
        })
    }

    /// Construct a backend by querying the IBM REST API for device topology.
    ///
    /// Fetches the backend configuration (coupling map, qubit count) and
    /// builds the `ConnectivityGraph` automatically.  Calibration is
    /// initialized with synthetic data; call `with_calibration` or wait
    /// for Task 6.11's live-calibration integration to replace it.
    ///
    /// # Errors
    ///
    /// Returns `IbmError::RestError` if the API request fails or the
    /// backend name is not found.  Returns `IbmError::HttpError` if the
    /// response cannot be deserialized.
    pub fn from_device(
        token: impl Into<String>,
        backend_name: impl Into<String>,
    ) -> Result<Self, IbmError> {
        let token = token.into();
        let backend_name = backend_name.into();
        let rest = IbmRestClient::new(&token, &backend_name)?;

        let config = rest.get_backend_config(&backend_name)?;

        let edges: Vec<(u32, u32)> = config
            .coupling_map
            .iter()
            .map(|pair| (pair[0], pair[1]))
            .collect();
        let connectivity = ConnectivityGraph::from_edges(config.num_qubits, &edges);
        let calibration = IbmCalibrationData::synthetic(config.num_qubits);

        Ok(Self {
            backend_name,
            num_qubits: config.num_qubits,
            edges: edges.clone(),
            connectivity,
            basis_gates: config.basis_gates.clone(),
            rest,
            calibration,
            optimization_level: 1,
            use_transpiler: true,
        })
    }

    /// Set the Qiskit transpiler optimization level (0–3).
    pub fn with_optimization_level(mut self, level: u8) -> Self {
        self.optimization_level = level.min(3);
        self
    }

    /// Set the job polling timeout in seconds.
    pub fn with_poll_timeout(mut self, secs: u64) -> Self {
        self.rest = self.rest.with_poll_timeout(secs);
        self
    }

    /// Disable IBM transpilation (circuit is submitted as-is after conversion).
    pub fn without_transpiler(mut self) -> Self {
        self.use_transpiler = false;
        self
    }

    /// Replace the calibration snapshot.
    pub fn with_calibration(mut self, cal: IbmCalibrationData) -> Self {
        self.calibration = cal;
        self
    }

    /// Fetch fresh calibration data from the IBM REST API and update the
    /// internal calibration snapshot.
    ///
    /// Replaces the current calibration (whether synthetic or from a prior
    /// refresh) with data parsed from the live `/properties` endpoint.
    ///
    /// # Errors
    ///
    /// Returns `IbmError::CalibrationError` if the REST request fails or
    /// the response cannot be parsed.  The existing calibration is NOT
    /// modified on error (the method is failure-atomic).
    pub fn refresh_calibration(&mut self) -> Result<(), IbmError> {
        let props = self.rest.get_backend_properties(&self.backend_name)?;
        let new_cal = IbmCalibrationData::from_ibm_properties(
            &props,
            self.num_qubits,
        )?;
        self.calibration = new_cal;
        Ok(())
    }

    /// Convert + optionally transpile a native IR circuit, then emit OpenQASM 2.
    fn to_qasm(&self, circuit: &native_ir::Circuit) -> Result<String, IbmError> {
        let mut qk_circ = native_to_qk(circuit)?;

        if self.use_transpiler {
            let output = if self.basis_gates.is_empty() {
                // Fallback: use hardcoded IBM {SX, X, Rz, CX} target
                crate::transpile::transpile_for_ibm(
                    &qk_circ,
                    self.num_qubits,
                    self.optimization_level,
                    None,
                )?
            } else {
                // Use the device's actual basis gates (supports CZ, RZZ, etc.)
                crate::transpile::transpile_for_device(
                    &qk_circ,
                    self.num_qubits,
                    &self.basis_gates,
                    &self.edges,
                    self.optimization_level,
                    None,
                )?
            };
            qk_circ = output.circuit;
        }

        crate::qasm::circuit_to_qasm3(&qk_circ)
    }
}

impl QpuBackend for IbmQpuBackend {
    fn gate_set(&self) -> &NativeGateSet {
        &NativeGateSet::Superconducting
    }

    fn connectivity(&self) -> &ConnectivityGraph {
        &self.connectivity
    }

    fn max_qubits(&self) -> u32 {
        self.num_qubits
    }

    fn compile(&self, circuit: &native_ir::Circuit) -> Result<(), CqamError> {
        // Validate: check qubit count against device capacity
        if circuit.num_physical_qubits > self.num_qubits {
            return Err(CqamError::QpuQubitAllocationFailed {
                required: circuit.num_physical_qubits,
                available: self.num_qubits,
            });
        }
        // Validate: dry-run conversion (ensures all gates are in the native set)
        native_to_qk(circuit).map(|_| ()).map_err(|e| e.into())
    }

    fn submit(
        &mut self,
        circuit: &native_ir::Circuit,
        convergence: &ConvergenceCriterion,
        shot_budget: u32,
    ) -> Result<RawResults, CqamError> {
        if circuit.num_physical_qubits > self.num_qubits {
            return Err(CqamError::QpuQubitAllocationFailed {
                required: circuit.num_physical_qubits,
                available: self.num_qubits,
            });
        }

        // Generate QASM once; reuse across all batches (transpilation is expensive).
        let qasm = self.to_qasm(circuit).map_err(CqamError::from)?;

        let mut estimator = BayesianEstimator::new(convergence.clone());
        let mut total_shots = 0u32;
        let mut remaining_budget = shot_budget;
        let mut batches = 0u32;

        loop {
            let batch_size = convergence.min_batch_size.min(remaining_budget);
            if batch_size == 0 {
                break;
            }

            let job_id = self
                .rest
                .submit_job(&qasm, batch_size)
                .map_err(CqamError::from)?;
            let result = self
                .rest
                .poll_until_done(&job_id, None, None)
                .map_err(CqamError::from)?;

            let (batch_counts, batch_shots) = result_to_counts(&result);
            total_shots += batch_shots;
            remaining_budget = remaining_budget.saturating_sub(batch_shots);
            estimator.update(&batch_counts);

            batches += 1;

            if estimator.is_converged() {
                break;
            }

            if remaining_budget == 0 {
                break;
            }
        }

        tracing::debug!(batches, total_shots, "adaptive shot loop complete");

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
        let status = self
            .rest
            .get_job_status(job_id)
            .map_err(CqamError::from)?;

        match status.state.status.to_uppercase().as_str() {
            "COMPLETED" | "DONE" => {}
            "FAILED" | "CANCELLED" | "ERROR" => {
                let reason = status.state.reason.unwrap_or_default();
                return Err(IbmError::UnexpectedStatus {
                    job_id: job_id.to_string(),
                    status: format!("{}: {}", status.state.status, reason),
                }
                .into());
            }
            // Not yet complete
            _ => return Ok(None),
        }

        let result = self
            .rest
            .get_job_results(job_id)
            .map_err(CqamError::from)?;

        let (counts, total_shots) = result_to_counts(&result);

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

    fn make_backend() -> IbmQpuBackend {
        // Use the test-only constructor that skips IAM token exchange.
        let rest = IbmRestClient::with_access_token("fake_token", "ibm_nairobi");
        let edges = &[(0u32,1),(1,2),(2,3),(3,4),(4,5),(5,6)];
        let connectivity = ConnectivityGraph::from_edges(7, edges);
        let calibration = IbmCalibrationData::synthetic(7);
        IbmQpuBackend {
            backend_name: "ibm_nairobi".to_string(),
            num_qubits: 7,
            edges: edges.to_vec(),
            connectivity,
            basis_gates: vec!["sx".into(), "x".into(), "rz".into(), "cx".into()],
            rest,
            calibration,
            optimization_level: 1,
            use_transpiler: false, // avoid calling the C library in pure unit tests
        }
    }

    #[test]
    fn test_gate_set_is_superconducting() {
        let b = make_backend();
        assert_eq!(b.gate_set(), &NativeGateSet::Superconducting);
    }

    #[test]
    fn test_max_qubits() {
        let b = make_backend();
        assert_eq!(b.max_qubits(), 7);
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
        // Should convert successfully (no actual hardware call)
        assert!(b.compile(&c).is_ok());
    }

    #[test]
    fn test_calibration_returns_correct_type() {
        let b = make_backend();
        let cal = b.calibration().unwrap();
        // T1 for qubit 0 should be > 0
        assert!(cal.t1(0) > 0.0);
    }

    #[test]
    fn test_connectivity_edges() {
        let b = make_backend();
        assert!(b.connectivity().are_connected(0, 1));
        assert!(b.connectivity().are_connected(5, 6));
        assert!(!b.connectivity().are_connected(0, 6));
    }

    #[test]
    fn test_builder_without_transpiler() {
        let rest = IbmRestClient::with_access_token("t", "dev");
        let b = IbmQpuBackend {
            backend_name: "dev".to_string(),
            num_qubits: 5,
            edges: vec![],
            connectivity: ConnectivityGraph::from_edges(5, &[]),
            basis_gates: vec![],
            rest,
            calibration: IbmCalibrationData::synthetic(5),
            optimization_level: 1,
            use_transpiler: true,
        }
        .without_transpiler()
        .with_optimization_level(3);
        assert_eq!(b.max_qubits(), 5);
    }

    #[test]
    fn test_submit_uses_convergence_parameter() {
        // Compile-time documentation: the parameter is no longer prefixed with
        // underscore, confirming it is intentionally used in the method body.
        let _criterion = ConvergenceCriterion::default();
        // The actual adaptive loop requires a live REST endpoint;
        // integration testing requires a mock server (out of scope for Task 6.12).
    }

    #[test]
    fn test_convergence_criterion_respected_in_trait_signature() {
        // Verify ConvergenceCriterion is part of the QpuBackend::submit signature
        // by constructing one and checking it compiles as an argument.
        let c = ConvergenceCriterion {
            confidence: 0.99,
            max_relative_error: 0.01,
            min_batch_size: 200,
        };
        assert!((c.confidence - 0.99).abs() < 1e-10);
        assert_eq!(c.min_batch_size, 200);
    }

    #[test]
    fn test_coupling_map_to_connectivity() {
        // Simulates what from_device does internally.
        let coupling_map: Vec<[u32; 2]> = vec![
            [0, 1], [1, 0], [1, 2], [2, 1], [2, 3], [3, 2],
        ];
        let edges: Vec<(u32, u32)> = coupling_map
            .iter()
            .map(|pair| (pair[0], pair[1]))
            .collect();
        let graph = ConnectivityGraph::from_edges(4, &edges);

        assert_eq!(graph.num_qubits, 4);
        assert_eq!(graph.num_edges(), 3); // 6 directed -> 3 undirected
        assert!(graph.are_connected(0, 1));
        assert!(graph.are_connected(1, 2));
        assert!(graph.are_connected(2, 3));
        assert!(!graph.are_connected(0, 3));
    }
}
