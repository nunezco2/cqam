//! Statistical Validation Test Suite (Task 6.2)
//!
//! Validates the QPU circuit compilation pipeline by comparing exact simulation
//! distributions (SimulationBackend) against shot-sampled results from
//! CircuitBackend<MockQpuBackend> using chi-squared goodness-of-fit tests.
//!
//! Architecture: calls QuantumBackend trait methods directly -- no runner,
//! no ExecutionContext, no ForkManager -- to isolate the quantum distribution
//! preservation property of the compilation pipeline.
//!
//! # Test programs
//!
//! Five programs covering entanglement, oracle-based search, and the QFT:
//!   1. Bell state      -- Prep(Bell) only; validates H + CX native decomposition.
//!   2. GHZ state       -- Prep(Ghz) only; validates H + CX chain.
//!   3. Grover search   -- Prep(Uniform) + GroverIter(target=3, 2 qubits).
//!                         One iteration is optimal: P(|11>) = 1.0 exactly.
//!   4. QFT of |0>      -- Prep(Zero) + Fourier. QFT|0> = uniform superposition.
//!   5. IQFT of uniform -- Prep(Uniform) + FourierInv. IQFT(uniform) = |0>.

use std::collections::BTreeMap;

use cqam_core::instruction::{DistId, KernelId, ObserveMode};
use cqam_core::native_ir::NativeGateSet;
use cqam_core::quantum_backend::{KernelParams, ObserveResult, QuantumBackend};

use cqam_sim::backend::SimulationBackend;
use cqam_sim::circuit_backend::CircuitBackend;

use cqam_qpu::mock::{MockCalibrationData, MockQpuBackend};
use cqam_qpu::traits::{CircuitQuantumBackend, ConnectivityGraph, ConvergenceCriterion};

// =============================================================================
// Constants
// =============================================================================

/// Total shots per statistical test.  Large enough that E_i >= 2500 for any
/// 4-outcome (2-qubit) uniform distribution, giving good chi-squared power.
const SHOT_BUDGET: u32 = 10_000;

// =============================================================================
// Backend constructors
// =============================================================================

fn make_sim_backend() -> SimulationBackend {
    let mut backend = SimulationBackend::new();
    backend.set_rng_seed(42);
    backend
}

fn make_circuit_backend(shot_budget: u32) -> CircuitBackend<MockQpuBackend> {
    let qpu = MockQpuBackend::with_config(
        // All-to-all avoids routing SWAPs that could introduce numerical drift.
        ConnectivityGraph::all_to_all(8),
        NativeGateSet::Superconducting,
        8,
        MockCalibrationData::default(),
        Some(42),
    );
    // Tight convergence so the Bayesian estimator uses the full shot budget
    // rather than converging early with a small fraction of the shots.
    let convergence = ConvergenceCriterion {
        confidence: 0.99,
        max_relative_error: 0.02,
        min_batch_size: 200,
    };
    CircuitBackend::new(qpu, convergence, shot_budget)
}

// =============================================================================
// Result extraction
// =============================================================================

/// Extract (outcome -> probability) map from an ObserveResult::Dist.
fn extract_dist(result: ObserveResult) -> BTreeMap<u32, f64> {
    match result {
        ObserveResult::Dist(pairs) => pairs.into_iter().collect(),
        other => panic!("expected ObserveResult::Dist, got {:?}", other),
    }
}

// =============================================================================
// Statistical test: chi-squared goodness-of-fit
// =============================================================================

/// Run a chi-squared goodness-of-fit test comparing the circuit backend's
/// empirical distribution against the exact simulation probabilities.
///
/// `exact`          - exact probability distribution from SimulationBackend
/// `observed_probs` - frequency distribution (count/total) from CircuitBackend
/// `total_shots`    - total shots used by CircuitBackend (for count recovery)
/// `label`          - test label for assertion messages
fn chi_squared_test(
    exact: &BTreeMap<u32, f64>,
    observed_probs: &BTreeMap<u32, f64>,
    total_shots: u32,
    label: &str,
) {
    let n = total_shots as f64;

    // Collect all unique outcomes from both distributions
    let mut all_outcomes: Vec<u32> = exact
        .keys()
        .chain(observed_probs.keys())
        .copied()
        .collect();
    all_outcomes.sort();
    all_outcomes.dedup();

    // Accumulate chi-squared statistic; merge low-expected-count bins.
    // The chi-squared approximation requires E_i >= 5 per bin.
    let mut chi2 = 0.0_f64;
    let mut df = 0_i32;
    let mut other_expected = 0.0_f64;
    let mut other_observed = 0.0_f64;

    for &outcome in &all_outcomes {
        let p_exact = exact.get(&outcome).copied().unwrap_or(0.0);
        let e_i = n * p_exact;
        // Observed counts recovered from prob * total (exact because prob = count/total).
        let o_i = observed_probs.get(&outcome).copied().unwrap_or(0.0) * n;

        if e_i >= 5.0 {
            chi2 += (o_i - e_i).powi(2) / e_i;
            df += 1;
        } else {
            // Merge into the "other" bin to satisfy chi-squared requirements.
            other_expected += e_i;
            other_observed += o_i;
        }
    }

    // Add the merged "other" bin only if it has sufficient expected count.
    if other_expected >= 5.0 {
        chi2 += (other_observed - other_expected).powi(2) / other_expected;
        df += 1;
    }

    // Degrees of freedom = k - 1 (where k = number of valid bins above).
    df -= 1;

    if df < 1 {
        // Degenerate case: single-outcome (deterministic) distribution.
        // The chi-squared test does not apply. Instead verify the dominant
        // outcome is dominant in the circuit output too.
        let (&expected_outcome, _) = exact
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .expect("exact distribution must be non-empty");
        let observed_p = observed_probs
            .get(&expected_outcome)
            .copied()
            .unwrap_or(0.0);
        assert!(
            observed_p > 0.95,
            "{label}: expected outcome {expected_outcome} should dominate with p > 0.95; \
             circuit backend reported p = {observed_p:.4}. \
             This indicates the circuit pipeline diverges from exact simulation.",
        );
        return;
    }

    let p_value = chi_squared_survival(chi2, df as f64);

    assert!(
        p_value > 0.01,
        "{label}: chi-squared test FAILED. chi2 = {chi2:.4}, df = {df}, \
         p-value = {p_value:.6}. \
         This indicates the circuit compilation pipeline output diverges from \
         exact simulation.",
    );
}

// =============================================================================
// Chi-squared CDF via regularized incomplete gamma function
// =============================================================================

/// Survival function (1 - CDF) of the chi-squared distribution.
/// Returns P(chi2 > x | df degrees of freedom).
fn chi_squared_survival(x: f64, df: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    let a = df / 2.0;
    let z = x / 2.0;
    1.0 - regularized_lower_gamma(a, z)
}

/// Regularized lower incomplete gamma function P(a, x) = gamma(a, x) / Gamma(a).
/// Uses series expansion for x < a + 1, Lentz continued fraction otherwise.
fn regularized_lower_gamma(a: f64, x: f64) -> f64 {
    if x < 0.0 {
        return 0.0;
    }
    if x == 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        gamma_series(a, x)
    } else {
        1.0 - gamma_cf(a, x)
    }
}

/// Series expansion for P(a, x):
///   P(a,x) = e^{-x} x^a * sum_{n=0}^{inf} x^n / (a*(a+1)*...*(a+n))
fn gamma_series(a: f64, x: f64) -> f64 {
    let max_iter = 200;
    let eps = 1e-12;

    let mut term = 1.0 / a;
    let mut sum = term;
    for n in 1..max_iter {
        term *= x / (a + n as f64);
        sum += term;
        if term.abs() < eps * sum.abs() {
            break;
        }
    }
    sum * (-x + a * x.ln() - ln_gamma(a)).exp()
}

/// Lentz continued fraction for the upper incomplete gamma Q(a, x).
/// Returns Q(a, x) = 1 - P(a, x).
fn gamma_cf(a: f64, x: f64) -> f64 {
    let max_iter = 200;
    let eps = 1e-12;
    let tiny = 1e-30;

    let mut c = tiny;
    let mut d = 1.0 / (x + 1.0 - a);
    let mut f = d;

    for n in 1_i32..max_iter {
        let nf = n as f64;
        let an = -nf * (nf - a);
        let bn = x + 2.0 * nf + 1.0 - a;
        d = bn + an * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = bn + an / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < eps {
            break;
        }
    }

    (-x + a * x.ln() - ln_gamma(a)).exp() * f
}

/// Natural log of the Gamma function via Lanczos approximation (g=7, n=9).
fn ln_gamma(x: f64) -> f64 {
    // Lanczos coefficients (Spouge, g=7)
    const COEFFS: [f64; 9] = [
        0.999_999_999_999_809_93,
        676.520_368_121_885_1,
        -1259.139_216_722_402_8,
        771.323_428_777_653_13,
        -176.615_029_162_140_59,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let g = 7.0_f64;

    if x < 0.5 {
        // Reflection formula: ln(Gamma(x)) = ln(pi/sin(pi*x)) - ln(Gamma(1-x))
        let pi = std::f64::consts::PI;
        (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x)
    } else {
        let z = x - 1.0;
        let mut t = COEFFS[0];
        for (i, &c) in COEFFS[1..].iter().enumerate() {
            t += c / (z + i as f64 + 1.0);
        }
        let w = z + g + 0.5;
        0.5 * (2.0 * std::f64::consts::PI).ln() + (z + 0.5) * w.ln() - w + t.ln()
    }
}

// =============================================================================
// Quantum program runners
// =============================================================================

/// Bell state: |Phi+> = (|00> + |11>) / sqrt(2).
///
/// Uses DistId::Bell which decomposes to Prep(Zero) + H(wire0) + CX(wire0, wire1).
/// Expected distribution: { 0: 0.5, 3: 0.5 }.
fn run_bell<B: QuantumBackend>(b: &mut B) -> ObserveResult {
    let (h, _) = b.prep(DistId::Bell, 2, false).unwrap();
    b.observe(h, ObserveMode::Dist, 0, 0).unwrap()
}

/// GHZ state (3 qubits): (|000> + |111>) / sqrt(2).
///
/// Uses DistId::Ghz which decomposes to Prep(Zero) + H(wire0) + CX(0,1) + CX(0,2).
/// Expected distribution: { 0: 0.5, 7: 0.5 }.
fn run_ghz<B: QuantumBackend>(b: &mut B) -> ObserveResult {
    let (h, _) = b.prep(DistId::Ghz, 3, false).unwrap();
    b.observe(h, ObserveMode::Dist, 0, 0).unwrap()
}

/// Grover search (2 qubits, target = 3 = |11>).
///
/// Uses Prep(Uniform) to create the initial superposition, then GroverIter.
/// The oracle for target=3 emits CZ(0,1) (no X gates needed since all bits are 1),
/// followed by the Diffuse operator (H.X.CZ.X.H).
///
/// For N=4, M=1, one iteration is optimal:
///   sin((2*1+1) * arcsin(sqrt(1/4))) = sin(pi/2) = 1.0
/// so P(|11>) = 1.0 exactly.
///
/// Expected distribution: { 3: 1.0 }.
fn run_grover<B: QuantumBackend>(b: &mut B) -> ObserveResult {
    let (h, _) = b.prep(DistId::Uniform, 2, false).unwrap();
    let p = KernelParams::Int { param0: 3, param1: 0, cmem_data: vec![] };
    let (h, _) = b.apply_kernel(h, KernelId::GroverIter, &p).unwrap();
    b.observe(h, ObserveMode::Dist, 0, 0).unwrap()
}

/// QFT applied to |0> state (2 qubits).
///
/// QFT |00> = (1/2) * sum_k |k> = uniform distribution over all 4 outcomes.
/// Prep(Zero) has no Hadamard gates, so there is no H.H optimizer interaction
/// with QFT's initial H(wire0).
///
/// Expected distribution: { 0: 0.25, 1: 0.25, 2: 0.25, 3: 0.25 }.
fn run_qft_of_zero<B: QuantumBackend>(b: &mut B) -> ObserveResult {
    let (h, _) = b.prep(DistId::Zero, 2, false).unwrap();
    let p = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
    let (h, _) = b.apply_kernel(h, KernelId::Fourier, &p).unwrap();
    b.observe(h, ObserveMode::Dist, 0, 0).unwrap()
}

/// Inverse QFT applied to the uniform superposition (2 qubits).
///
/// |uniform> = QFT |0>, so QFT^{-1}(|uniform>) = |0>.
///
/// Expected distribution: { 0: 1.0 } (deterministic).
fn run_iqft_of_uniform<B: QuantumBackend>(b: &mut B) -> ObserveResult {
    let (h, _) = b.prep(DistId::Uniform, 2, false).unwrap();
    let p = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
    let (h, _) = b.apply_kernel(h, KernelId::FourierInv, &p).unwrap();
    b.observe(h, ObserveMode::Dist, 0, 0).unwrap()
}

// =============================================================================
// Tests
// =============================================================================

/// Bell state: validates that Prep(Bell) decomposes correctly and the 50/50
/// distribution is preserved through the circuit pipeline.
#[test]
fn statistical_bell_state() {
    let exact = extract_dist(run_bell(&mut make_sim_backend()));
    let mut cb = make_circuit_backend(SHOT_BUDGET);
    let observed = extract_dist(run_bell(&mut cb));
    let shots = cb.metrics().shots_used;
    chi_squared_test(&exact, &observed, shots, "bell_state");
}

/// GHZ state (3 qubits): validates H + CX + CX decomposition chain preserves
/// the two-peak distribution { |000>: 0.5, |111>: 0.5 }.
#[test]
fn statistical_ghz_state() {
    let exact = extract_dist(run_ghz(&mut make_sim_backend()));
    let mut cb = make_circuit_backend(SHOT_BUDGET);
    let observed = extract_dist(run_ghz(&mut cb));
    let shots = cb.metrics().shots_used;
    chi_squared_test(&exact, &observed, shots, "ghz_state");
}

/// Grover search (2 qubits, target=3): validates oracle + diffusion decomposition
/// yields a near-deterministic outcome (P(|11>) > 0.95 with 1 iteration).
#[test]
fn statistical_grover() {
    let exact = extract_dist(run_grover(&mut make_sim_backend()));
    let mut cb = make_circuit_backend(SHOT_BUDGET);
    let observed = extract_dist(run_grover(&mut cb));
    let shots = cb.metrics().shots_used;
    chi_squared_test(&exact, &observed, shots, "grover_2q");
}

/// QFT of |0> state (2 qubits): validates QFT decomposition creates the
/// 4-outcome uniform distribution from the zero state.
#[test]
fn statistical_qft_of_zero() {
    let exact = extract_dist(run_qft_of_zero(&mut make_sim_backend()));
    let mut cb = make_circuit_backend(SHOT_BUDGET);
    let observed = extract_dist(run_qft_of_zero(&mut cb));
    let shots = cb.metrics().shots_used;
    chi_squared_test(&exact, &observed, shots, "qft_of_zero");
}

/// IQFT of uniform superposition (2 qubits): validates inverse QFT decomposition
/// maps uniform -> |0> (deterministic output). Uses Prep(Uniform) which is
/// safe because IQFT starts with SWAP (not H), avoiding H.H cancellations.
#[test]
fn statistical_iqft_of_uniform() {
    let exact = extract_dist(run_iqft_of_uniform(&mut make_sim_backend()));
    let mut cb = make_circuit_backend(SHOT_BUDGET);
    let observed = extract_dist(run_iqft_of_uniform(&mut cb));
    let shots = cb.metrics().shots_used;
    chi_squared_test(&exact, &observed, shots, "iqft_of_uniform");
}

