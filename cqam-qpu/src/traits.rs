//! Core QPU traits, structs, and error types.

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::fmt;

use cqam_core::error::CqamError;
use cqam_core::native_ir;
use cqam_core::quantum_backend::QuantumBackend;

// =============================================================================
// QpuError
// =============================================================================

/// Errors specific to QPU backend operations.
#[derive(Debug)]
pub enum QpuError {
    /// Job submission failed.
    SubmissionFailed { provider: String, detail: String },
    /// Device is offline or unavailable.
    DeviceOffline { provider: String },
    /// Not enough physical qubits on the device.
    QubitAllocation { required: u32, available: u32 },
    /// Operation not supported by this backend.
    UnsupportedOperation { operation: String, detail: String },
    /// Shot budget exceeded.
    ShotBudgetExhausted { budget: u32, used: u32 },
    /// Calibration data error.
    CalibrationError { detail: String },
    /// Wraps a CqamError from lower layers.
    Core(CqamError),
}

impl fmt::Display for QpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QpuError::SubmissionFailed { provider, detail } =>
                write!(f, "QPU submission to {} failed: {}", provider, detail),
            QpuError::DeviceOffline { provider } =>
                write!(f, "QPU device offline: {}", provider),
            QpuError::QubitAllocation { required, available } =>
                write!(f, "QPU qubit allocation failed: need {} qubits, only {} available", required, available),
            QpuError::UnsupportedOperation { operation, detail } =>
                write!(f, "QPU unsupported operation {}: {}", operation, detail),
            QpuError::ShotBudgetExhausted { budget, used } =>
                write!(f, "QPU shot budget exhausted: used {}/{}", used, budget),
            QpuError::CalibrationError { detail } =>
                write!(f, "QPU calibration error: {}", detail),
            QpuError::Core(e) =>
                write!(f, "{}", e),
        }
    }
}

impl std::error::Error for QpuError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            QpuError::Core(e) => Some(e),
            _ => None,
        }
    }
}

impl From<CqamError> for QpuError {
    fn from(err: CqamError) -> Self {
        QpuError::Core(err)
    }
}

impl From<QpuError> for CqamError {
    fn from(err: QpuError) -> Self {
        match err {
            QpuError::SubmissionFailed { provider, detail } =>
                CqamError::QpuSubmissionFailed { provider, detail },
            QpuError::DeviceOffline { provider } =>
                CqamError::QpuDeviceOffline { provider },
            QpuError::QubitAllocation { required, available } =>
                CqamError::QpuQubitAllocationFailed { required, available },
            QpuError::UnsupportedOperation { operation, detail } =>
                CqamError::QpuUnsupportedOperation { operation, detail },
            QpuError::ShotBudgetExhausted { budget, used } =>
                CqamError::QpuShotBudgetExhausted { budget, used },
            QpuError::CalibrationError { detail } =>
                CqamError::QpuCalibrationError { detail },
            QpuError::Core(e) => e,
        }
    }
}

// =============================================================================
// QpuMetrics
// =============================================================================

/// Hardware execution metrics, separate from abstract ResourceTracker.
#[derive(Debug, Clone, Default)]
pub struct QpuMetrics {
    /// Circuit depth after decomposition and routing.
    pub circuit_depth: u32,
    /// Number of routing SWAPs inserted.
    pub swap_count: u32,
    /// Total shots consumed.
    pub shots_used: u32,
    /// Wall-clock time for job execution (seconds).
    pub wall_time_secs: f64,
    /// Estimated monetary cost (provider-specific).
    pub estimated_cost: Option<f64>,
    /// Number of physical qubits used.
    pub physical_qubits_used: u32,
    /// Estimated circuit fidelity.
    pub estimated_fidelity: f64,
    /// Number of circuit cache hits.
    pub cache_hits: u64,
    /// Number of circuit compilations.
    pub compilations: u64,
}

// =============================================================================
// ConnectivityGraph
// =============================================================================

/// Device qubit connectivity topology.
#[derive(Debug, Clone)]
pub struct ConnectivityGraph {
    /// Number of physical qubits.
    pub num_qubits: u32,
    /// Edges: pairs of physically connected qubits (normalized: a < b).
    edges: Vec<(u32, u32)>,
}

impl ConnectivityGraph {
    /// Create from a list of edges.
    pub fn from_edges(num_qubits: u32, edges: &[(u32, u32)]) -> Self {
        let mut normalized: Vec<(u32, u32)> = edges.iter()
            .map(|&(a, b)| if a <= b { (a, b) } else { (b, a) })
            .collect();
        normalized.sort();
        normalized.dedup();
        Self { num_qubits, edges: normalized }
    }

    /// All-to-all connectivity (trapped-ion, simulation).
    pub fn all_to_all(n: u32) -> Self {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                edges.push((i, j));
            }
        }
        Self { num_qubits: n, edges }
    }

    /// Linear chain connectivity.
    pub fn linear(n: u32) -> Self {
        let mut edges = Vec::new();
        for i in 0..n.saturating_sub(1) {
            edges.push((i, i + 1));
        }
        Self { num_qubits: n, edges }
    }

    /// Heavy-hex topology (IBM Falcon 27-qubit).
    /// Only n=27 is supported in Phase 1.
    pub fn heavy_hex(n: u32) -> Self {
        assert_eq!(n, 27, "heavy_hex only supports n=27 in Phase 1");
        // IBM Falcon (ibm_cairo / ibm_hanoi) 27-qubit coupling map
        let edges: Vec<(u32, u32)> = vec![
            (0,1),(1,2),(1,4),(2,3),(3,5),
            (4,7),(5,8),
            (6,7),(7,10),(8,9),(8,11),
            (10,12),(11,14),
            (12,13),(12,15),(13,14),(14,16),
            (15,18),(16,19),
            (17,18),(18,21),(19,20),(19,22),
            (21,23),(22,25),
            (23,24),(24,25),(25,26),
        ];
        Self::from_edges(n, &edges)
    }

    /// Check if two physical qubits are directly connected.
    pub fn are_connected(&self, a: u32, b: u32) -> bool {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.edges.binary_search(&(lo, hi)).is_ok()
    }

    /// All edges as an iterator.
    pub fn edges(&self) -> &[(u32, u32)] {
        &self.edges
    }

    /// Number of edges.
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Shortest path between two qubits (BFS).
    /// Returns the path including both endpoints, or empty vec if no path.
    pub fn shortest_path(&self, from: u32, to: u32) -> Vec<u32> {
        if from == to {
            return vec![from];
        }
        if from >= self.num_qubits || to >= self.num_qubits {
            return Vec::new();
        }

        // Build adjacency list
        let mut adj: Vec<Vec<u32>> = vec![Vec::new(); self.num_qubits as usize];
        for &(a, b) in &self.edges {
            adj[a as usize].push(b);
            adj[b as usize].push(a);
        }

        // BFS
        let mut visited = vec![false; self.num_qubits as usize];
        let mut parent = vec![u32::MAX; self.num_qubits as usize];
        let mut queue = VecDeque::new();

        visited[from as usize] = true;
        queue.push_back(from);

        while let Some(current) = queue.pop_front() {
            for &neighbor in &adj[current as usize] {
                if !visited[neighbor as usize] {
                    visited[neighbor as usize] = true;
                    parent[neighbor as usize] = current;
                    if neighbor == to {
                        let mut path = Vec::new();
                        let mut node = to;
                        while node != u32::MAX {
                            path.push(node);
                            if node == from { break; }
                            node = parent[node as usize];
                        }
                        path.reverse();
                        return path;
                    }
                    queue.push_back(neighbor);
                }
            }
        }

        Vec::new()
    }

    /// Neighbors of a qubit.
    pub fn neighbors(&self, qubit: u32) -> Vec<u32> {
        let mut result = Vec::new();
        for &(a, b) in &self.edges {
            if a == qubit { result.push(b); }
            else if b == qubit { result.push(a); }
        }
        result
    }

    /// Degree of a qubit (number of connections).
    pub fn degree(&self, qubit: u32) -> usize {
        self.neighbors(qubit).len()
    }
}

// =============================================================================
// ConvergenceCriterion
// =============================================================================

/// Bayesian convergence criterion for shot management.
#[derive(Debug, Clone)]
pub struct ConvergenceCriterion {
    /// Minimum confidence level (0.0 to 1.0).
    pub confidence: f64,
    /// Maximum relative error for probability estimates.
    pub max_relative_error: f64,
    /// Minimum batch size per submission.
    pub min_batch_size: u32,
}

impl Default for ConvergenceCriterion {
    fn default() -> Self {
        Self {
            confidence: 0.95,
            max_relative_error: 0.05,
            min_batch_size: 100,
        }
    }
}

// =============================================================================
// RawResults
// =============================================================================

/// Raw measurement results from hardware.
#[derive(Debug, Clone)]
pub struct RawResults {
    /// Bitstring -> count mapping.
    pub counts: BTreeMap<u64, u32>,
    /// Total shots executed.
    pub total_shots: u32,
    /// Hardware-reported metrics.
    pub metrics: QpuMetrics,
}

// =============================================================================
// QpuBackend trait
// =============================================================================

/// Hardware-facing QPU backend trait.
///
/// Receives fully decomposed, routed, optimized native circuits
/// and submits them to hardware.
pub trait QpuBackend: Send {
    /// The native gate set this backend supports.
    fn gate_set(&self) -> &native_ir::NativeGateSet;

    /// Device connectivity graph (for routing).
    fn connectivity(&self) -> &ConnectivityGraph;

    /// Maximum number of physical qubits.
    fn max_qubits(&self) -> u32;

    /// Compile a native circuit for submission.
    fn compile(&self, circuit: &native_ir::Circuit) -> Result<(), CqamError>;

    /// Submit a native circuit for execution.
    fn submit(
        &mut self,
        circuit: &native_ir::Circuit,
        convergence: &ConvergenceCriterion,
        shot_budget: u32,
    ) -> Result<RawResults, CqamError>;

    /// Poll for results of a previously submitted job.
    fn poll_results(&self, job_id: &str) -> Result<Option<RawResults>, CqamError>;

    /// Query fresh calibration data from the device.
    fn calibration(&self) -> Result<Box<dyn CalibrationData>, CqamError>;
}

// =============================================================================
// CalibrationData trait
// =============================================================================

/// Device calibration data queried at runtime.
pub trait CalibrationData: Send + Sync {
    /// Per-qubit T1 relaxation time (seconds).
    fn t1(&self, qubit: u32) -> f64;

    /// Per-qubit T2 dephasing time (seconds).
    fn t2(&self, qubit: u32) -> f64;

    /// Single-qubit gate error rate for a specific qubit.
    fn single_gate_error(&self, qubit: u32) -> f64;

    /// Two-qubit gate error rate for a specific edge.
    fn two_gate_error(&self, qubit_a: u32, qubit_b: u32) -> f64;

    /// Readout error rate for a specific qubit.
    fn readout_error(&self, qubit: u32) -> f64;

    /// Single-qubit gate time (seconds).
    fn single_gate_time(&self) -> f64;

    /// Two-qubit gate time (seconds).
    fn two_gate_time(&self) -> f64;

    /// Estimated circuit fidelity based on accumulated gate errors.
    fn estimate_circuit_fidelity(&self, circuit: &native_ir::Circuit) -> f64;
}

// =============================================================================
// CircuitQuantumBackend trait
// =============================================================================

/// Circuit-submission backend for QPU execution.
///
/// Extends QuantumBackend -- a CircuitQuantumBackend IS a QuantumBackend.
/// The VM dispatches through the same execute_qop function.
pub trait CircuitQuantumBackend: QuantumBackend {
    /// Access the QpuMetrics accumulated during execution.
    fn metrics(&self) -> &QpuMetrics;

    /// Force a circuit flush without observation (for debugging).
    fn force_flush(&mut self) -> Result<(), CqamError>;
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_to_all_4() {
        let g = ConnectivityGraph::all_to_all(4);
        assert_eq!(g.num_qubits, 4);
        assert_eq!(g.num_edges(), 6);
        assert!(g.are_connected(0, 1));
        assert!(g.are_connected(0, 3));
        assert!(g.are_connected(2, 3));
    }

    #[test]
    fn test_all_to_all_1() {
        let g = ConnectivityGraph::all_to_all(1);
        assert_eq!(g.num_edges(), 0);
    }

    #[test]
    fn test_linear_5() {
        let g = ConnectivityGraph::linear(5);
        assert_eq!(g.num_qubits, 5);
        assert_eq!(g.num_edges(), 4);
        assert!(g.are_connected(0, 1));
        assert!(g.are_connected(3, 4));
        assert!(!g.are_connected(0, 2));
        assert!(!g.are_connected(0, 4));
    }

    #[test]
    fn test_linear_1() {
        let g = ConnectivityGraph::linear(1);
        assert_eq!(g.num_edges(), 0);
    }

    #[test]
    fn test_are_connected_symmetric() {
        let g = ConnectivityGraph::from_edges(3, &[(0, 2)]);
        assert!(g.are_connected(0, 2));
        assert!(g.are_connected(2, 0));
    }

    #[test]
    fn test_shortest_path_linear() {
        let g = ConnectivityGraph::linear(5);
        let path = g.shortest_path(0, 4);
        assert_eq!(path, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_shortest_path_same_node() {
        let g = ConnectivityGraph::linear(5);
        assert_eq!(g.shortest_path(2, 2), vec![2]);
    }

    #[test]
    fn test_shortest_path_adjacent() {
        let g = ConnectivityGraph::linear(5);
        assert_eq!(g.shortest_path(1, 2), vec![1, 2]);
    }

    #[test]
    fn test_shortest_path_all_to_all() {
        let g = ConnectivityGraph::all_to_all(4);
        let path = g.shortest_path(0, 3);
        assert_eq!(path, vec![0, 3]);
    }

    #[test]
    fn test_disconnected_graph() {
        let g = ConnectivityGraph::from_edges(4, &[(0, 1), (2, 3)]);
        assert!(g.shortest_path(0, 3).is_empty());
    }

    #[test]
    fn test_heavy_hex_27() {
        let g = ConnectivityGraph::heavy_hex(27);
        assert_eq!(g.num_qubits, 27);
        assert!(g.num_edges() > 0);
        // Verify some known connections
        assert!(g.are_connected(0, 1));
        assert!(g.are_connected(1, 2));
    }

    #[test]
    fn test_from_edges_deduplicates() {
        let g = ConnectivityGraph::from_edges(3, &[(0, 1), (1, 0), (0, 1)]);
        assert_eq!(g.num_edges(), 1);
    }

    #[test]
    fn test_neighbors() {
        let g = ConnectivityGraph::linear(5);
        let n = g.neighbors(2);
        assert!(n.contains(&1));
        assert!(n.contains(&3));
        assert_eq!(n.len(), 2);
    }

    #[test]
    fn test_degree() {
        let g = ConnectivityGraph::all_to_all(4);
        assert_eq!(g.degree(0), 3);
    }

    #[test]
    fn test_convergence_criterion_default() {
        let c = ConvergenceCriterion::default();
        assert!((c.confidence - 0.95).abs() < 1e-10);
        assert!((c.max_relative_error - 0.05).abs() < 1e-10);
        assert_eq!(c.min_batch_size, 100);
    }

    #[test]
    fn test_qpu_metrics_default() {
        let m = QpuMetrics::default();
        assert_eq!(m.circuit_depth, 0);
        assert_eq!(m.shots_used, 0);
        assert_eq!(m.estimated_cost, None);
    }

    #[test]
    fn test_qpu_error_into_cqam_error() {
        let err = QpuError::SubmissionFailed {
            provider: "IBM".to_string(),
            detail: "timeout".to_string(),
        };
        let cqam_err: CqamError = err.into();
        let msg = format!("{}", cqam_err);
        assert!(msg.contains("IBM"));
    }

    #[test]
    fn test_cqam_error_into_qpu_error() {
        let err = CqamError::TypeMismatch {
            instruction: "test".to_string(),
            detail: "detail".to_string(),
        };
        let qpu_err: QpuError = err.into();
        assert!(matches!(qpu_err, QpuError::Core(_)));
    }

    #[test]
    fn test_qpu_error_display() {
        let err = QpuError::DeviceOffline { provider: "IonQ".to_string() };
        assert!(format!("{}", err).contains("IonQ"));

        let err = QpuError::ShotBudgetExhausted { budget: 1000, used: 1001 };
        assert!(format!("{}", err).contains("1000"));
    }
}
