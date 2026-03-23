//! Mock QPU backend for testing the compilation pipeline.
//!
//! `MockQpuBackend` implements `QpuBackend` using a standalone statevector
//! simulator that depends only on `C64` from `cqam-core`. It does NOT depend
//! on `cqam-sim`, avoiding a circular dependency.
//!
//! Enable with `features = ["mock"]` in Cargo.toml.

use std::collections::BTreeMap;
use rand::SeedableRng;
use rand::Rng as _;
use rand_chacha::ChaCha8Rng;

use cqam_core::complex::C64;
use cqam_core::error::CqamError;
use cqam_core::native_ir::{self, NativeGate1, NativeGate2, NativeGateSet, Op};

use crate::traits::{
    CalibrationData, ConnectivityGraph, ConvergenceCriterion,
    QpuBackend, QpuMetrics, RawResults,
};
use crate::estimator::BayesianEstimator;

// =============================================================================
// MockCalibrationData
// =============================================================================

/// Calibration data for MockQpuBackend. Uniform across all qubits.
#[derive(Debug, Clone)]
pub struct MockCalibrationData {
    /// T1 relaxation time (seconds). Default: 100 us.
    pub t1: f64,
    /// T2 dephasing time (seconds). Default: 50 us.
    pub t2: f64,
    /// Single-qubit gate error rate. Default: 1e-3.
    pub single_gate_error: f64,
    /// Two-qubit gate error rate. Default: 1e-2.
    pub two_gate_error: f64,
    /// Readout error rate. Default: 1e-2.
    pub readout_error: f64,
    /// Single-qubit gate time (seconds). Default: 35 ns.
    pub single_gate_time: f64,
    /// Two-qubit gate time (seconds). Default: 300 ns.
    pub two_gate_time: f64,
}

impl Default for MockCalibrationData {
    fn default() -> Self {
        Self {
            t1: 100e-6,
            t2: 50e-6,
            single_gate_error: 1e-3,
            two_gate_error: 1e-2,
            readout_error: 1e-2,
            single_gate_time: 35e-9,
            two_gate_time: 300e-9,
        }
    }
}

impl CalibrationData for MockCalibrationData {
    fn t1(&self, _qubit: u32) -> f64 { self.t1 }
    fn t2(&self, _qubit: u32) -> f64 { self.t2 }
    fn single_gate_error(&self, _qubit: u32) -> f64 { self.single_gate_error }
    fn two_gate_error(&self, _qubit_a: u32, _qubit_b: u32) -> f64 { self.two_gate_error }
    fn readout_error(&self, _qubit: u32) -> f64 { self.readout_error }
    fn single_gate_time(&self) -> f64 { self.single_gate_time }
    fn two_gate_time(&self) -> f64 { self.two_gate_time }

    fn estimate_circuit_fidelity(&self, circuit: &native_ir::Circuit) -> f64 {
        let g1 = circuit.gate1q_count();
        let g2 = circuit.gate2q_count();
        let m = circuit.ops.iter().filter(|op| matches!(op, Op::Measure(_))).count();
        (1.0 - self.single_gate_error).powi(g1 as i32)
            * (1.0 - self.two_gate_error).powi(g2 as i32)
            * (1.0 - self.readout_error).powi(m as i32)
    }
}

// =============================================================================
// MockQpuBackend
// =============================================================================

/// A mock QPU backend for pipeline testing.
///
/// Executes circuits using standalone statevector simulation (no dependency
/// on cqam-sim). Supports deterministic output via seeded RNG.
pub struct MockQpuBackend {
    connectivity: ConnectivityGraph,
    gate_set: NativeGateSet,
    max_qubits: u32,
    calibration: MockCalibrationData,
    rng: ChaCha8Rng,
}

impl MockQpuBackend {
    /// Create a default mock backend: all-to-all(27), Superconducting, 27 qubits.
    pub fn new() -> Self {
        Self::with_config(
            ConnectivityGraph::all_to_all(27),
            NativeGateSet::Superconducting,
            27,
            MockCalibrationData::default(),
            None,
        )
    }

    /// Create a mock backend with explicit configuration.
    pub fn with_config(
        connectivity: ConnectivityGraph,
        gate_set: NativeGateSet,
        max_qubits: u32,
        calibration: MockCalibrationData,
        seed: Option<u64>,
    ) -> Self {
        let rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => ChaCha8Rng::from_entropy(),
        };
        Self { connectivity, gate_set, max_qubits, calibration, rng }
    }
}

impl Default for MockQpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MockQpuBackend {
    fn clone(&self) -> Self {
        Self {
            connectivity: self.connectivity.clone(),
            gate_set: self.gate_set.clone(),
            max_qubits: self.max_qubits,
            calibration: self.calibration.clone(),
            rng: self.rng.clone(),
        }
    }
}

impl QpuBackend for MockQpuBackend {
    fn gate_set(&self) -> &NativeGateSet {
        &self.gate_set
    }

    fn connectivity(&self) -> &ConnectivityGraph {
        &self.connectivity
    }

    fn max_qubits(&self) -> u32 {
        self.max_qubits
    }

    fn compile(&self, circuit: &native_ir::Circuit) -> Result<(), CqamError> {
        // Validate qubit count
        if circuit.num_physical_qubits > self.max_qubits {
            return Err(CqamError::QpuQubitAllocationFailed {
                required: circuit.num_physical_qubits,
                available: self.max_qubits,
            });
        }

        // Validate connectivity: every Gate2q must reference a connected pair
        for op in &circuit.ops {
            if let Op::Gate2q(g) = op {
                let a = g.qubit_a.0;
                let b = g.qubit_b.0;
                if !self.connectivity.are_connected(a, b) {
                    return Err(CqamError::QpuUnsupportedOperation {
                        operation: "compile".to_string(),
                        detail: format!(
                            "qubits {} and {} are not connected in the device topology",
                            a, b
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    fn submit(
        &mut self,
        circuit: &native_ir::Circuit,
        convergence: &ConvergenceCriterion,
        shot_budget: u32,
    ) -> Result<RawResults, CqamError> {
        // 1. Validate
        self.compile(circuit)?;

        // 2. Build statevector: 2^n amplitudes, initialized to |0...0>
        let n = circuit.num_physical_qubits;
        let dim = 1usize << n;
        let mut sv = vec![C64::ZERO; dim];
        sv[0] = C64::ONE;

        // 3. Apply gates
        for op in &circuit.ops {
            match op {
                Op::Gate1q(g) => {
                    let qubit = g.qubit.0;
                    let mat = gate1q_matrix(&g.gate);
                    apply_gate1q(&mut sv, n, qubit, mat);
                }
                Op::Gate2q(g) => {
                    let qa = g.qubit_a.0;
                    let qb = g.qubit_b.0;
                    let mat = gate2q_matrix(&g.gate);
                    apply_gate2q(&mut sv, n, qa, qb, mat);
                }
                // Mid-circuit measure/reset/barrier: skip in Phase 3 mock
                Op::Measure(_) | Op::Reset(_) | Op::Barrier(_) => {}
            }
        }

        // 4. Compute probabilities over the full statevector.
        let full_probs: Vec<f64> = sv.iter().map(|c| c.norm_sq()).collect();

        // 4b. Project probabilities onto measured qubits.
        //
        // Collect which physical qubits are measured (and their classical bit
        // positions) from any Measure ops in the circuit.  If there are no
        // Measure ops, fall back to measuring all n qubits (preserving prior
        // behaviour).  When Measure ops are present, compute the marginal
        // distribution over only those qubits so that ancilla qubits added by
        // the decomposition pipeline do not corrupt the classical result.
        let measured: Vec<(u32, u32)> = circuit.ops.iter()
            .filter_map(|op| if let Op::Measure(obs) = op {
                Some((obs.qubit.0, obs.clbit))
            } else {
                None
            })
            .collect();

        let probs: Vec<f64> = if measured.is_empty() {
            // No Measure ops: sample the full state (legacy behaviour).
            full_probs
        } else {
            // Determine the number of classical bits = max clbit + 1.
            let n_cbits = measured.iter().map(|&(_, c)| c + 1).max().unwrap_or(0) as usize;
            let n_states = 1usize << n_cbits;
            let mut marginal = vec![0.0f64; n_states];

            for (state_idx, &p) in full_probs.iter().enumerate() {
                // Compute the classical bitstring for this quantum state.
                // clbit k gets the value of the physical qubit assigned to it.
                let mut classical = 0u64;
                for &(phys_q, clbit) in &measured {
                    // In big-endian convention: qubit phys_q controls bit (n-1-phys_q).
                    let qubit_bit = (state_idx >> (n - 1 - phys_q as u32) as usize) & 1;
                    if qubit_bit != 0 {
                        // clbit 0 = MSB of classical register (big-endian classical).
                        classical |= 1u64 << (n_cbits - 1 - clbit as usize);
                    }
                }
                marginal[classical as usize] += p;
            }

            marginal
        };

        // 5. Adaptive shot sampling with BayesianEstimator
        let mut estimator = BayesianEstimator::new(convergence.clone());
        let mut shots_used: u32 = 0;

        loop {
            let suggested = estimator.suggest_batch_size().max(convergence.min_batch_size);
            let remaining = shot_budget.saturating_sub(shots_used);
            if remaining == 0 {
                break;
            }
            let batch_size = suggested.min(remaining);

            let batch_counts = sample_bitstrings(&probs, batch_size, &mut self.rng);
            estimator.update(&batch_counts);
            shots_used += batch_size;

            if estimator.is_converged() {
                break;
            }
        }

        // 6. Construct RawResults
        let counts = estimator.finalize();
        let estimated_fidelity = self.calibration.estimate_circuit_fidelity(circuit);

        Ok(RawResults {
            counts,
            total_shots: shots_used,
            metrics: QpuMetrics {
                circuit_depth: circuit.depth,
                swap_count: circuit.swap_count,
                shots_used,
                wall_time_secs: 0.0,
                estimated_cost: None,
                physical_qubits_used: circuit.num_physical_qubits,
                estimated_fidelity,
                cache_hits: 0,
                compilations: 0,
            },
        })
    }

    fn poll_results(&self, _job_id: &str) -> Result<Option<RawResults>, CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "poll_results".to_string(),
            detail: "MockQpuBackend executes synchronously".to_string(),
        })
    }

    fn calibration(&self) -> Result<Box<dyn CalibrationData>, CqamError> {
        Ok(Box::new(self.calibration.clone()))
    }
}

// =============================================================================
// Gate matrices
// =============================================================================

/// Returns the 2x2 matrix for a native 1-qubit gate.
fn gate1q_matrix(gate: &NativeGate1) -> [[C64; 2]; 2] {
    match gate {
        NativeGate1::X => [
            [C64::ZERO, C64::ONE],
            [C64::ONE, C64::ZERO],
        ],
        NativeGate1::Sx => {
            // Sx = 0.5 * [[1+i, 1-i], [1-i, 1+i]]
            let a = C64(0.5, 0.5);  // (1+i)/2
            let b = C64(0.5, -0.5); // (1-i)/2
            [[a, b], [b, a]]
        }
        NativeGate1::Rz(theta) => {
            // Rz(theta) = [[e^{-i*theta/2}, 0], [0, e^{i*theta/2}]]
            let half = theta / 2.0;
            let e_neg = C64((-half).cos(), (-half).sin());
            let e_pos = C64(half.cos(), half.sin());
            [[e_neg, C64::ZERO], [C64::ZERO, e_pos]]
        }
        NativeGate1::Id => [
            [C64::ONE, C64::ZERO],
            [C64::ZERO, C64::ONE],
        ],
    }
}

/// Returns the 4x4 matrix for a native 2-qubit gate (row-major, basis order |00>,|01>,|10>,|11>).
fn gate2q_matrix(gate: &NativeGate2) -> [[C64; 4]; 4] {
    match gate {
        NativeGate2::Cx => {
            // CNOT: control=q0, target=q1
            // |00>->|00>, |01>->|01>, |10>->|11>, |11>->|10>
            [
                [C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO],
                [C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO],
                [C64::ZERO, C64::ZERO, C64::ZERO, C64::ONE ],
                [C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO],
            ]
        }
    }
}

// =============================================================================
// Statevector gate application (standalone, no cqam-sim dependency)
// =============================================================================

/// Apply a 2x2 matrix to a single qubit in the statevector.
///
/// Uses big-endian qubit ordering: qubit 0 is the MSB.
/// For a state over n qubits, qubit `q` maps to bit position `n - 1 - q`
/// in the basis index.
fn apply_gate1q(sv: &mut [C64], n_qubits: u32, qubit: u32, matrix: [[C64; 2]; 2]) {
    let dim = sv.len();
    // Bit position in index corresponding to this qubit (big-endian: qubit 0 = MSB)
    let bit = n_qubits - 1 - qubit;
    let bit_val = 1usize << bit;

    let mut k = 0usize;
    while k < dim {
        // Skip blocks where this bit is set (we only process pairs where bit=0)
        if k & bit_val != 0 {
            k += 1;
            continue;
        }
        let k1 = k | bit_val;
        let a = sv[k];
        let b = sv[k1];
        sv[k]  = matrix[0][0] * a + matrix[0][1] * b;
        sv[k1] = matrix[1][0] * a + matrix[1][1] * b;
        k += 1;
    }
}

/// Apply a 4x4 matrix to two qubits in the statevector.
///
/// `q0` is treated as the "control-like" qubit (higher significance in the 2-qubit
/// subspace), `q1` as the "target-like" qubit (lower significance).
/// Big-endian ordering is preserved.
fn apply_gate2q(sv: &mut [C64], n_qubits: u32, q0: u32, q1: u32, matrix: [[C64; 4]; 4]) {
    let dim = sv.len();
    let bit0 = 1usize << (n_qubits - 1 - q0);
    let bit1 = 1usize << (n_qubits - 1 - q1);

    let mut base = 0usize;
    while base < dim {
        // Only process indices where both q0 and q1 bits are 0
        if base & bit0 != 0 || base & bit1 != 0 {
            base += 1;
            continue;
        }
        let i00 = base;
        let i01 = base | bit1;
        let i10 = base | bit0;
        let i11 = base | bit0 | bit1;

        let a = sv[i00];
        let b = sv[i01];
        let c = sv[i10];
        let d = sv[i11];

        sv[i00] = matrix[0][0]*a + matrix[0][1]*b + matrix[0][2]*c + matrix[0][3]*d;
        sv[i01] = matrix[1][0]*a + matrix[1][1]*b + matrix[1][2]*c + matrix[1][3]*d;
        sv[i10] = matrix[2][0]*a + matrix[2][1]*b + matrix[2][2]*c + matrix[2][3]*d;
        sv[i11] = matrix[3][0]*a + matrix[3][1]*b + matrix[3][2]*c + matrix[3][3]*d;

        base += 1;
    }
}

// =============================================================================
// Shot sampling
// =============================================================================

/// Sample `n_shots` bitstrings from a probability distribution using the given RNG.
/// Naive O(dim * n_shots) inverse-CDF approach (acceptable for mock testing).
fn sample_bitstrings(probs: &[f64], n_shots: u32, rng: &mut ChaCha8Rng) -> BTreeMap<u64, u32> {
    let mut counts = BTreeMap::new();

    for _ in 0..n_shots {
        let r: f64 = rng.r#gen::<f64>();
        let mut cumulative = 0.0;
        let mut selected = probs.len() - 1;
        for (i, &p) in probs.iter().enumerate() {
            cumulative += p;
            if r < cumulative {
                selected = i;
                break;
            }
        }
        *counts.entry(selected as u64).or_insert(0) += 1;
    }

    counts
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;
    use cqam_core::native_ir::{ApplyGate1q, ApplyGate2q, Circuit, Op, PhysicalQubit};

    fn make_circuit_x() -> Circuit {
        // X gate on qubit 0 of a 1-qubit circuit
        let mut c = Circuit::new(1);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::X,
        }));
        c.depth = 1;
        c
    }

    fn make_circuit_bell() -> Circuit {
        // H on qubit 0, then CX(0, 1) -- Bell state on 2 qubits.
        // H = Rz(pi/2) Sx Rz(pi/2)
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Rz(PI / 2.0),
        }));
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Sx,
        }));
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Rz(PI / 2.0),
        }));
        c.ops.push(Op::Gate2q(ApplyGate2q {
            qubit_a: PhysicalQubit(0),
            qubit_b: PhysicalQubit(1),
            gate: NativeGate2::Cx,
        }));
        c.depth = 2;
        c
    }

    fn make_empty_circuit() -> Circuit {
        // No gates -- identity on 1 qubit
        Circuit::new(1)
    }

    fn convergence() -> ConvergenceCriterion {
        ConvergenceCriterion { confidence: 0.95, max_relative_error: 0.05, min_batch_size: 100 }
    }

    fn make_backend_seeded(seed: u64) -> MockQpuBackend {
        MockQpuBackend::with_config(
            ConnectivityGraph::all_to_all(27),
            NativeGateSet::Superconducting,
            27,
            MockCalibrationData::default(),
            Some(seed),
        )
    }

    #[test]
    fn test_x_gate_measures_one() {
        let mut backend = make_backend_seeded(42);
        let circuit = make_circuit_x();
        let conv = convergence();
        let result = backend.submit(&circuit, &conv, 1000).unwrap();
        // X gate on |0> produces |1>; expect bitstring 1 to dominate
        let count_1 = *result.counts.get(&1).unwrap_or(&0);
        let count_0 = *result.counts.get(&0).unwrap_or(&0);
        assert!(count_1 > count_0, "X gate should produce |1> state; got 0:{} 1:{}", count_0, count_1);
        // All shots should be on |1> (no |0> expected)
        assert_eq!(count_0, 0, "X gate on |0> must not produce any |0> outcomes");
        assert_eq!(count_1, result.total_shots, "X gate should produce 100% |1>; got {}/{}", count_1, result.total_shots);
    }

    #[test]
    fn test_bell_circuit_correlated() {
        let mut backend = make_backend_seeded(7);
        let circuit = make_circuit_bell();
        let conv = convergence();
        let result = backend.submit(&circuit, &conv, 2000).unwrap();
        // Bell state: only 00 (0) and 11 (3) should appear
        for (&bitstring, &count) in &result.counts {
            assert!(
                bitstring == 0 || bitstring == 3,
                "Bell state should only produce 00 or 11, got bitstring {} with {} shots",
                bitstring, count
            );
        }
        let count_00 = *result.counts.get(&0).unwrap_or(&0);
        let count_11 = *result.counts.get(&3).unwrap_or(&0);
        assert!(count_00 > 0 && count_11 > 0, "Both 00 and 11 must appear in Bell state");
    }

    #[test]
    fn test_shot_budget_respected() {
        let mut backend = make_backend_seeded(1);
        let circuit = make_circuit_x();
        let conv = convergence();
        let budget = 500;
        let result = backend.submit(&circuit, &conv, budget).unwrap();
        assert!(
            result.total_shots <= budget,
            "total_shots {} must not exceed budget {}",
            result.total_shots, budget
        );
    }

    #[test]
    fn test_connectivity_validation() {
        // Use a linear-2 topology: only qubits 0-1 connected
        let backend = MockQpuBackend::with_config(
            ConnectivityGraph::linear(3),
            NativeGateSet::Superconducting,
            3,
            MockCalibrationData::default(),
            Some(0),
        );
        // Circuit with CX(0, 2) -- qubits 0 and 2 are NOT connected on linear(3)
        let mut circuit = Circuit::new(3);
        circuit.ops.push(Op::Gate2q(ApplyGate2q {
            qubit_a: PhysicalQubit(0),
            qubit_b: PhysicalQubit(2),
            gate: NativeGate2::Cx,
        }));
        let err = backend.compile(&circuit);
        assert!(err.is_err(), "Should reject non-adjacent qubit pair on linear topology");
    }

    #[test]
    fn test_qubit_allocation_limit() {
        let backend = MockQpuBackend::with_config(
            ConnectivityGraph::all_to_all(5),
            NativeGateSet::Superconducting,
            5,
            MockCalibrationData::default(),
            Some(0),
        );
        // Circuit requiring 6 qubits but backend only has 5
        let circuit = Circuit::new(6);
        let err = backend.compile(&circuit);
        assert!(err.is_err(), "Should reject circuit exceeding max_qubits");
    }

    #[test]
    fn test_calibration_defaults_reasonable() {
        let data = MockCalibrationData::default();
        assert!(data.t1 > 0.0 && data.t1 < 1.0, "T1 should be in microsecond range");
        assert!(data.t2 > 0.0 && data.t2 < data.t1, "T2 should be positive and < T1");
        assert!(data.single_gate_error > 0.0 && data.single_gate_error < 0.1);
        assert!(data.two_gate_error > 0.0 && data.two_gate_error < 0.5);
        assert!(data.readout_error > 0.0 && data.readout_error < 0.5);
        assert!(data.single_gate_time > 0.0 && data.single_gate_time < 1e-6);
        assert!(data.two_gate_time > 0.0 && data.two_gate_time < 1e-5);
    }

    #[test]
    fn test_fidelity_decreases_with_depth() {
        let data = MockCalibrationData::default();
        let mut shallow = Circuit::new(2);
        shallow.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::X,
        }));

        let mut deep = Circuit::new(2);
        for _ in 0..20 {
            deep.ops.push(Op::Gate1q(ApplyGate1q {
                qubit: PhysicalQubit(0),
                gate: NativeGate1::X,
            }));
            deep.ops.push(Op::Gate2q(ApplyGate2q {
                qubit_a: PhysicalQubit(0),
                qubit_b: PhysicalQubit(1),
                gate: NativeGate2::Cx,
            }));
        }

        let f_shallow = data.estimate_circuit_fidelity(&shallow);
        let f_deep = data.estimate_circuit_fidelity(&deep);
        assert!(f_shallow > f_deep, "deeper circuit should have lower fidelity");
    }

    #[test]
    fn test_deterministic_with_seed() {
        let circuit = make_circuit_bell();
        let conv = convergence();

        let mut b1 = make_backend_seeded(12345);
        let r1 = b1.submit(&circuit, &conv, 500).unwrap();

        let mut b2 = make_backend_seeded(12345);
        let r2 = b2.submit(&circuit, &conv, 500).unwrap();

        assert_eq!(r1.counts, r2.counts, "same seed must produce identical results");
        assert_eq!(r1.total_shots, r2.total_shots);
    }

    #[test]
    fn test_identity_circuit_measures_zero() {
        let mut backend = make_backend_seeded(99);
        let circuit = make_empty_circuit();
        let conv = convergence();
        let result = backend.submit(&circuit, &conv, 1000).unwrap();
        // No gates applied: should measure |0> every time
        let count_0 = *result.counts.get(&0).unwrap_or(&0);
        assert_eq!(
            count_0, result.total_shots,
            "Identity circuit should produce 100% |0>; got {}/{}",
            count_0, result.total_shots
        );
        // Ensure no other outcomes appear
        assert_eq!(result.counts.len(), 1, "Only |0> should appear in counts");
    }

    #[test]
    fn test_mock_qpu_clone_independent_rng() {
        // Two clones from the same seeded backend must produce identical results
        // (each clone gets its own copy of the RNG state at clone time).
        let circuit = make_circuit_bell();
        let conv = convergence();

        let base = make_backend_seeded(99999);
        let mut c1 = base.clone();
        let mut c2 = base.clone();

        let r1 = c1.submit(&circuit, &conv, 500).unwrap();
        let r2 = c2.submit(&circuit, &conv, 500).unwrap();

        // Both clones start from identical RNG state, so results must match
        assert_eq!(r1.counts, r2.counts, "clones from same state must produce identical results");
        assert_eq!(r1.total_shots, r2.total_shots);
    }

    #[test]
    fn test_rz_sx_decomposition() {
        // Verify that Rz(pi/2) -> Sx -> Rz(pi/2) produces H-like behavior.
        // H|0> = |+> = (|0> + |1>) / sqrt(2), so P(0) = P(1) = 0.5.
        let mut backend = make_backend_seeded(55);
        let mut circuit = Circuit::new(1);
        circuit.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Rz(PI / 2.0),
        }));
        circuit.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Sx,
        }));
        circuit.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Rz(PI / 2.0),
        }));

        let conv = ConvergenceCriterion { confidence: 0.95, max_relative_error: 0.10, min_batch_size: 200 };
        let result = backend.submit(&circuit, &conv, 5000).unwrap();
        let total = result.total_shots as f64;
        let p0 = *result.counts.get(&0).unwrap_or(&0) as f64 / total;
        let p1 = *result.counts.get(&1).unwrap_or(&0) as f64 / total;
        assert!((p0 - 0.5).abs() < 0.10, "Rz-Sx-Rz should produce ~50% |0>; got {:.3}", p0);
        assert!((p1 - 0.5).abs() < 0.10, "Rz-Sx-Rz should produce ~50% |1>; got {:.3}", p1);
    }
}
