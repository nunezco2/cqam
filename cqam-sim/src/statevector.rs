//! Pure-state quantum register represented as a statevector.
//!
//! Stores |psi> = sum_k alpha_k |k> as a Vec<C64> of length 2^num_qubits.
//! All operations preserve normalization: sum_k |alpha_k|^2 = 1.
//!
//! Advantages over DensityMatrix for pure states:
//! - O(2^n) memory vs O(4^n)
//! - O(2^n) per gate application vs O(4^n)
//!
//! Limitations:
//! - Cannot represent mixed states
//! - Measurement produces a new Statevector (projective)
//! - Partial trace requires conversion to DensityMatrix

use crate::complex::{self, C64, cx_add, cx_mul, cx_scale, cx_norm_sq};
use crate::density_matrix::DensityMatrix;
use cqam_core::quantum_state::QuantumState;
use rand::Rng;
use rayon::prelude::*;

/// Minimum dimension to use parallel iteration.
const PAR_THRESHOLD: usize = 256;

/// Tolerance for entanglement detection via single-qubit reduced purity.
const EF_EPSILON: f64 = 1e-10;

/// Tolerance for superposition detection: amplitudes with |ψ|² below this are
/// treated as zero (not contributing to the computational basis decomposition).
const SF_EPSILON: f64 = 1e-12;

/// Maximum qubits for statevector backend.
pub const MAX_SV_QUBITS: u8 = 24;

/// Pure-state quantum register represented as a statevector.
#[derive(Debug, Clone)]
pub struct Statevector {
    num_qubits: u8,
    amplitudes: Vec<C64>,
}

// =============================================================================
// Construction
// =============================================================================

impl Statevector {
    /// Create the computational zero state |0...0>.
    pub fn new_zero_state(num_qubits: u8) -> Self {
        assert!(
            (1..=MAX_SV_QUBITS).contains(&num_qubits),
            "num_qubits must be 1..={}, got {}",
            MAX_SV_QUBITS, num_qubits
        );
        let dim = 1usize << num_qubits;
        let mut amplitudes = vec![complex::ZERO; dim];
        amplitudes[0] = complex::ONE;
        Self { num_qubits, amplitudes }
    }

    /// Create the uniform superposition H^n|0...0>.
    pub fn new_uniform(num_qubits: u8) -> Self {
        assert!(
            (1..=MAX_SV_QUBITS).contains(&num_qubits),
            "num_qubits must be 1..={}, got {}",
            MAX_SV_QUBITS, num_qubits
        );
        let dim = 1usize << num_qubits;
        let amp = 1.0 / (dim as f64).sqrt();
        let amplitudes = vec![(amp, 0.0); dim];
        Self { num_qubits, amplitudes }
    }

    /// Create a Bell state (|00> + |11>)/sqrt(2).
    pub fn new_bell() -> Self {
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let amplitudes = vec![
            (inv_sqrt2, 0.0),
            complex::ZERO,
            complex::ZERO,
            (inv_sqrt2, 0.0),
        ];
        Self { num_qubits: 2, amplitudes }
    }

    /// Create a GHZ state (|0...0> + |1...1>)/sqrt(2).
    ///
    /// Returns Err if num_qubits < 2 or > MAX_SV_QUBITS.
    pub fn new_ghz(num_qubits: u8) -> Result<Self, String> {
        if num_qubits < 2 || num_qubits > MAX_SV_QUBITS {
            return Err(format!(
                "GHZ state requires 2..={} qubits, got {}",
                MAX_SV_QUBITS, num_qubits
            ));
        }
        let dim = 1usize << num_qubits;
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let mut amplitudes = vec![complex::ZERO; dim];
        amplitudes[0] = (inv_sqrt2, 0.0);
        amplitudes[dim - 1] = (inv_sqrt2, 0.0);
        Ok(Self { num_qubits, amplitudes })
    }

    /// Construct from an explicit amplitude vector.
    ///
    /// The length must be a power of 2 and the vector will be normalized.
    pub fn from_amplitudes(amplitudes: Vec<C64>) -> Result<Self, String> {
        let len = amplitudes.len();
        if len == 0 || (len & (len - 1)) != 0 {
            return Err(format!(
                "Amplitude vector length {} is not a power of 2", len
            ));
        }
        let num_qubits = len.trailing_zeros() as u8;
        if num_qubits > MAX_SV_QUBITS {
            return Err(format!(
                "Amplitude vector implies {} qubits, max is {}",
                num_qubits, MAX_SV_QUBITS
            ));
        }

        // Normalize
        let norm_sq: f64 = amplitudes.iter().map(|z| cx_norm_sq(*z)).sum();
        let norm = norm_sq.sqrt();
        let amplitudes = if (norm - 1.0).abs() > 1e-12 && norm > 1e-30 {
            amplitudes.iter().map(|z| cx_scale(1.0 / norm, *z)).collect()
        } else {
            amplitudes
        };

        Ok(Self { num_qubits, amplitudes })
    }
}

// =============================================================================
// Element Access
// =============================================================================

impl Statevector {
    /// Get the amplitude of basis state |k>.
    #[inline]
    pub fn amplitude(&self, k: usize) -> C64 {
        self.amplitudes[k]
    }

    /// Get a reference to the full amplitude vector.
    #[inline]
    pub fn amplitudes(&self) -> &[C64] {
        &self.amplitudes
    }

    /// Number of qubits.
    #[inline]
    pub fn num_qubits(&self) -> u8 {
        self.num_qubits
    }

    /// Hilbert space dimension: 2^num_qubits.
    #[inline]
    pub fn dimension(&self) -> usize {
        1 << self.num_qubits
    }
}

// =============================================================================
// Unitary Application
// =============================================================================

impl Statevector {
    /// Apply a unitary transformation: |psi'> = U|psi>.
    ///
    /// The `unitary` slice must contain dim*dim elements in row-major order.
    /// O(dim^2) instead of O(dim^3) for density matrix.
    pub fn apply_unitary(&mut self, unitary: &[C64]) {
        let dim = self.dimension();
        assert_eq!(unitary.len(), dim * dim,
            "Unitary size mismatch: expected {}, got {}", dim * dim, unitary.len());

        let amps = &self.amplitudes;
        let result: Vec<C64> = if dim >= PAR_THRESHOLD {
            (0..dim).into_par_iter().map(|i| {
                let mut sum = complex::ZERO;
                for k in 0..dim {
                    sum = cx_add(sum, cx_mul(unitary[i * dim + k], amps[k]));
                }
                sum
            }).collect()
        } else {
            let mut result = vec![complex::ZERO; dim];
            for i in 0..dim {
                let mut sum = complex::ZERO;
                for k in 0..dim {
                    sum = cx_add(sum, cx_mul(unitary[i * dim + k], amps[k]));
                }
                result[i] = sum;
            }
            result
        };
        self.amplitudes = result;
    }

    /// Apply a single-qubit gate to a specific qubit.
    ///
    /// O(2^n) operations (two multiplications per amplitude pair).
    pub fn apply_single_qubit_gate(&mut self, target: u8, gate: &[C64; 4]) {
        let n = self.num_qubits as usize;
        let dim = self.dimension();
        assert!(
            (target as usize) < n,
            "target qubit {} out of range for {}-qubit system", target, n
        );

        let bit = n - 1 - target as usize;
        let mask = 1usize << bit;

        let [g00, g01, g10, g11] = *gate;

        if dim >= PAR_THRESHOLD {
            let pairs: Vec<usize> = (0..dim).filter(|&i0| i0 & mask == 0).collect();
            let updates: Vec<(usize, C64, C64)> = pairs.par_iter().map(|&i0| {
                let i1 = i0 | mask;
                let a0 = self.amplitudes[i0];
                let a1 = self.amplitudes[i1];
                (i0,
                 cx_add(cx_mul(g00, a0), cx_mul(g01, a1)),
                 cx_add(cx_mul(g10, a0), cx_mul(g11, a1)))
            }).collect();
            for (i0, v0, v1) in updates {
                let i1 = i0 | mask;
                self.amplitudes[i0] = v0;
                self.amplitudes[i1] = v1;
            }
        } else {
            for i0 in 0..dim {
                if i0 & mask != 0 {
                    continue;
                }
                let i1 = i0 | mask;
                let a0 = self.amplitudes[i0];
                let a1 = self.amplitudes[i1];
                self.amplitudes[i0] = cx_add(cx_mul(g00, a0), cx_mul(g01, a1));
                self.amplitudes[i1] = cx_add(cx_mul(g10, a0), cx_mul(g11, a1));
            }
        }
    }

    /// Apply a two-qubit gate to specific control and target qubits.
    ///
    /// O(2^n) operations.
    pub fn apply_two_qubit_gate(&mut self, ctrl: u8, tgt: u8, gate: &[C64; 16]) {
        let n = self.num_qubits as usize;
        let dim = self.dimension();
        assert!(
            (ctrl as usize) < n && (tgt as usize) < n,
            "qubit indices ({}, {}) out of range for {}-qubit system", ctrl, tgt, n
        );
        assert!(ctrl != tgt, "ctrl ({}) must differ from tgt ({})", ctrl, tgt);

        let ctrl_bit = n - 1 - ctrl as usize;
        let tgt_bit = n - 1 - tgt as usize;
        let ctrl_mask = 1usize << ctrl_bit;
        let tgt_mask = 1usize << tgt_bit;

        let bases: Vec<usize> = (0..dim)
            .filter(|&base| base & (ctrl_mask | tgt_mask) == 0)
            .collect();

        if dim >= PAR_THRESHOLD {
            let updates: Vec<([usize; 4], [C64; 4])> = bases.par_iter().map(|&base| {
                let i00 = base;
                let i01 = base | tgt_mask;
                let i10 = base | ctrl_mask;
                let i11 = base | ctrl_mask | tgt_mask;
                let idxs = [i00, i01, i10, i11];
                let orig = [
                    self.amplitudes[i00],
                    self.amplitudes[i01],
                    self.amplitudes[i10],
                    self.amplitudes[i11],
                ];
                let mut results = [complex::ZERO; 4];
                for a in 0..4 {
                    let mut sum = complex::ZERO;
                    for b in 0..4 {
                        sum = cx_add(sum, cx_mul(gate[a * 4 + b], orig[b]));
                    }
                    results[a] = sum;
                }
                (idxs, results)
            }).collect();
            for (idxs, results) in updates {
                for (i, &idx) in idxs.iter().enumerate() {
                    self.amplitudes[idx] = results[i];
                }
            }
        } else {
            for &base in &bases {
                let i00 = base;
                let i01 = base | tgt_mask;
                let i10 = base | ctrl_mask;
                let i11 = base | ctrl_mask | tgt_mask;
                let idxs = [i00, i01, i10, i11];
                let orig = [
                    self.amplitudes[i00],
                    self.amplitudes[i01],
                    self.amplitudes[i10],
                    self.amplitudes[i11],
                ];
                for (a, &idx) in idxs.iter().enumerate() {
                    let mut sum = complex::ZERO;
                    for b in 0..4 {
                        sum = cx_add(sum, cx_mul(gate[a * 4 + b], orig[b]));
                    }
                    self.amplitudes[idx] = sum;
                }
            }
        }
    }
}

// =============================================================================
// Measurement
// =============================================================================

impl Statevector {
    /// Measure a single qubit using a caller-supplied RNG, returning (outcome, post-measurement state).
    pub fn measure_qubit_with_rng(&self, target: u8, rng: &mut impl Rng) -> (u8, Statevector) {
        let n = self.num_qubits as usize;
        let dim = self.dimension();
        assert!(
            (target as usize) < n,
            "target qubit {} out of range for {}-qubit system", target, n
        );

        let bit = n - 1 - target as usize;
        let mask = 1usize << bit;

        // Compute p(0)
        let mut p0: f64 = if dim >= PAR_THRESHOLD {
            (0..dim).into_par_iter()
                .filter(|&k| k & mask == 0)
                .map(|k| cx_norm_sq(self.amplitudes[k]))
                .sum()
        } else {
            let mut p0 = 0.0;
            for k in 0..dim {
                if k & mask == 0 {
                    p0 += cx_norm_sq(self.amplitudes[k]);
                }
            }
            p0
        };
        p0 = p0.clamp(0.0, 1.0);
        let p1 = 1.0 - p0;

        // Sample outcome
        let r: f64 = rng.r#gen();
        let outcome: u8 = if r < p0 { 0 } else { 1 };
        let p_outcome = if outcome == 0 { p0 } else { p1 };

        // Project and renormalize
        let outcome_bit = if outcome == 0 { 0 } else { mask };
        let mut result = if dim >= PAR_THRESHOLD {
            self.amplitudes.par_iter().enumerate().map(|(k, &val)| {
                if (k & mask) != outcome_bit {
                    complex::ZERO
                } else {
                    val
                }
            }).collect::<Vec<C64>>()
        } else {
            let mut result = self.amplitudes.clone();
            for (k, val) in result.iter_mut().enumerate().take(dim) {
                if (k & mask) != outcome_bit {
                    *val = complex::ZERO;
                }
            }
            result
        };

        // Renormalize
        if p_outcome > 1e-30 {
            let inv_norm = 1.0 / p_outcome.sqrt();
            if dim >= PAR_THRESHOLD {
                result.par_iter_mut().for_each(|amp| {
                    *amp = cx_scale(inv_norm, *amp);
                });
            } else {
                for amp in result.iter_mut() {
                    *amp = cx_scale(inv_norm, *amp);
                }
            }
        }

        (outcome, Statevector { num_qubits: self.num_qubits, amplitudes: result })
    }

    /// Measure a single qubit using thread-local RNG (non-reproducible).
    pub fn measure_qubit(&self, target: u8) -> (u8, Statevector) {
        self.measure_qubit_with_rng(target, &mut rand::thread_rng())
    }

    /// Measure all qubits, returning (outcome, collapsed state).
    pub fn measure_all(&self) -> (u16, Statevector) {
        let dim = self.dimension();
        let probs = self.diagonal_probabilities();

        let mut rng = rand::thread_rng();
        let r: f64 = rng.r#gen();

        let mut cumulative = 0.0;
        let mut outcome = dim - 1;
        for (k, &p) in probs.iter().enumerate() {
            cumulative += p;
            if r < cumulative {
                outcome = k;
                break;
            }
        }

        // Collapsed state
        let mut amplitudes = vec![complex::ZERO; dim];
        amplitudes[outcome] = complex::ONE;

        (outcome as u16, Statevector { num_qubits: self.num_qubits, amplitudes })
    }

    /// Deterministic measurement: argmax of |alpha_k|^2.
    pub fn measure_deterministic(&self) -> u16 {
        let mut max_idx = 0;
        let mut max_prob = f64::NEG_INFINITY;
        for (k, amp) in self.amplitudes.iter().enumerate() {
            let p = cx_norm_sq(*amp);
            if p > max_prob {
                max_prob = p;
                max_idx = k;
            }
        }
        max_idx as u16
    }

    /// Extract diagonal probabilities: |alpha_k|^2 for each k.
    pub fn diagonal_probabilities(&self) -> Vec<f64> {
        let dim = self.dimension();
        if dim >= PAR_THRESHOLD {
            self.amplitudes.par_iter().map(|z| cx_norm_sq(*z)).collect()
        } else {
            self.amplitudes.iter().map(|z| cx_norm_sq(*z)).collect()
        }
    }
}

// =============================================================================
// Metrics and Conversion
// =============================================================================

impl Statevector {
    /// Purity is always 1.0 for a pure state.
    pub fn purity(&self) -> f64 {
        1.0
    }

    /// Convert to a DensityMatrix: rho = |psi><psi|.
    ///
    /// This is O(4^n) and should be avoided for large n.
    pub fn to_density_matrix(&self) -> DensityMatrix {
        DensityMatrix::from_statevector(&self.amplitudes)
            .expect("Statevector should always produce a valid DensityMatrix")
    }

    /// Tensor product: |psi_A> tensor |psi_B>.
    ///
    /// Returns Err if the combined qubit count exceeds MAX_SV_QUBITS.
    pub fn tensor_product(&self, other: &Statevector) -> Result<Statevector, String> {
        let n_total = self.num_qubits + other.num_qubits;
        if n_total > MAX_SV_QUBITS {
            return Err(format!(
                "tensor product: {} + {} = {} qubits exceeds maximum {}",
                self.num_qubits, other.num_qubits, n_total, MAX_SV_QUBITS
            ));
        }

        let dim_a = self.dimension();
        let dim_b = other.dimension();
        let total_dim = dim_a * dim_b;

        let amplitudes: Vec<C64> = if total_dim >= PAR_THRESHOLD {
            let self_amps = &self.amplitudes;
            let other_amps = &other.amplitudes;
            (0..dim_a).into_par_iter().flat_map(|i| {
                let a_i = self_amps[i];
                (0..dim_b).map(|j| cx_mul(a_i, other_amps[j])).collect::<Vec<_>>()
            }).collect()
        } else {
            let mut amplitudes = Vec::with_capacity(total_dim);
            for i in 0..dim_a {
                for j in 0..dim_b {
                    amplitudes.push(cx_mul(self.amplitudes[i], other.amplitudes[j]));
                }
            }
            amplitudes
        };

        Ok(Statevector {
            num_qubits: n_total,
            amplitudes,
        })
    }

    /// Partial trace over subsystem B, returning the reduced density matrix ρ_A.
    ///
    /// For a pure state |ψ⟩ of n qubits, subsystem A = first `num_qubits_a` qubits,
    /// subsystem B = remaining (n - num_qubits_a) qubits.
    ///
    /// ρ_A[i,j] = Σ_k ψ[i·dim_b + k] · conj(ψ[j·dim_b + k])
    ///
    /// This is O(dim_a² × dim_b) and does NOT require building the full density matrix.
    pub fn partial_trace_b(&self, num_qubits_a: u8) -> Result<DensityMatrix, String> {
        if num_qubits_a == 0 || num_qubits_a >= self.num_qubits {
            return Err(format!(
                "partial_trace_b: num_qubits_a must be 1..{}, got {}",
                self.num_qubits, num_qubits_a
            ));
        }

        let dim_a = 1usize << num_qubits_a;
        let dim_b = 1usize << (self.num_qubits - num_qubits_a);

        let rho_a: Vec<C64> = if dim_a * dim_a >= PAR_THRESHOLD {
            let amps = &self.amplitudes;
            (0..dim_a).into_par_iter().flat_map(|i| {
                (0..dim_a).map(|j| {
                    let mut sum = complex::ZERO;
                    for k in 0..dim_b {
                        let psi_ik = amps[i * dim_b + k];
                        let psi_jk = amps[j * dim_b + k];
                        // conj(psi_jk) * psi_ik  (note: ρ_A[i,j] = Σ_k ψ_ik · ψ_jk*)
                        let conj_jk = (psi_jk.0, -psi_jk.1);
                        sum = cx_add(sum, cx_mul(psi_ik, conj_jk));
                    }
                    sum
                }).collect::<Vec<_>>()
            }).collect()
        } else {
            let mut rho_a = vec![complex::ZERO; dim_a * dim_a];
            for i in 0..dim_a {
                for j in 0..dim_a {
                    let mut sum = complex::ZERO;
                    for k in 0..dim_b {
                        let psi_ik = self.amplitudes[i * dim_b + k];
                        let psi_jk = self.amplitudes[j * dim_b + k];
                        let conj_jk = (psi_jk.0, -psi_jk.1);
                        sum = cx_add(sum, cx_mul(psi_ik, conj_jk));
                    }
                    rho_a[i * dim_a + j] = sum;
                }
            }
            rho_a
        };

        DensityMatrix::from_raw(num_qubits_a, rho_a)
    }

    /// Returns true if the state is in superposition in the computational basis.
    ///
    /// A state is in superposition iff more than one computational basis state
    /// has nonzero probability (|ψ_k|² > SF_EPSILON). Returns false for single
    /// basis states (measurement outcome is deterministic).
    ///
    /// Cost: O(2^n) worst case, O(1) best case (early exit after second nonzero).
    pub fn is_in_superposition(&self) -> bool {
        let mut nonzero_count = 0usize;
        for &(re, im) in &self.amplitudes {
            if re * re + im * im > SF_EPSILON {
                nonzero_count += 1;
                if nonzero_count > 1 {
                    return true;
                }
            }
        }
        false
    }

    /// Returns true if any qubit is entangled with the rest of the register.
    ///
    /// Uses single-qubit reduced purity scan: for each qubit k, compute the
    /// 2x2 reduced density matrix by tracing out all other qubits, then check
    /// if purity < 1 - epsilon.
    ///
    /// O(2^n) best case (early exit on first entangled qubit),
    /// O(n * 2^n) worst case. Zero heap allocation.
    pub fn is_any_qubit_entangled(&self) -> bool {
        let n = self.num_qubits as usize;
        if n < 2 {
            return false;
        }
        let dim = self.dimension();
        let amps = &self.amplitudes;

        for k in 0..n {
            // Bit position for qubit k (MSB-first convention matching gate application)
            let bit = n - 1 - k;
            let mask = 1usize << bit;

            let mut rho_00: f64 = 0.0;
            let mut rho_11: f64 = 0.0;
            let mut rho_01_re: f64 = 0.0;
            let mut rho_01_im: f64 = 0.0;

            // Iterate over all 2^(n-1) basis-state pairs where bit `bit` is 0
            for j0 in 0..dim {
                if j0 & mask != 0 {
                    continue;
                }
                let j1 = j0 | mask;
                let a0 = amps[j0]; // psi[j0]
                let a1 = amps[j1]; // psi[j1]

                // rho_00 += |psi[j0]|^2
                rho_00 += a0.0 * a0.0 + a0.1 * a0.1;
                // rho_11 += |psi[j1]|^2
                rho_11 += a1.0 * a1.0 + a1.1 * a1.1;
                // rho_01 += psi[j0] * conj(psi[j1])
                rho_01_re += a0.0 * a1.0 + a0.1 * a1.1;
                rho_01_im += a0.1 * a1.0 - a0.0 * a1.1;
            }

            // purity = rho_00^2 + rho_11^2 + 2*|rho_01|^2
            let purity = rho_00 * rho_00 + rho_11 * rho_11
                + 2.0 * (rho_01_re * rho_01_re + rho_01_im * rho_01_im);

            if purity < 1.0 - EF_EPSILON {
                return true;
            }
        }
        false
    }

    /// Compute the minimum single-qubit reduced purity across all qubits.
    ///
    /// Returns 1.0 for product states, 0.5 for maximally entangled states
    /// (Bell/GHZ). Useful for diagnostics and testing.
    pub fn min_single_qubit_purity(&self) -> f64 {
        let n = self.num_qubits as usize;
        if n < 2 {
            return 1.0;
        }
        let dim = self.dimension();
        let amps = &self.amplitudes;
        let mut min_purity = 1.0_f64;

        for k in 0..n {
            let bit = n - 1 - k;
            let mask = 1usize << bit;

            let mut rho_00: f64 = 0.0;
            let mut rho_11: f64 = 0.0;
            let mut rho_01_re: f64 = 0.0;
            let mut rho_01_im: f64 = 0.0;

            for j0 in 0..dim {
                if j0 & mask != 0 {
                    continue;
                }
                let j1 = j0 | mask;
                let a0 = amps[j0];
                let a1 = amps[j1];

                rho_00 += a0.0 * a0.0 + a0.1 * a0.1;
                rho_11 += a1.0 * a1.0 + a1.1 * a1.1;
                rho_01_re += a0.0 * a1.0 + a0.1 * a1.1;
                rho_01_im += a0.1 * a1.0 - a0.0 * a1.1;
            }

            let purity = rho_00 * rho_00 + rho_11 * rho_11
                + 2.0 * (rho_01_re * rho_01_re + rho_01_im * rho_01_im);

            if purity < min_purity {
                min_purity = purity;
            }
        }
        min_purity
    }

    /// Convert to a DensityMatrix: rho = |psi><psi|.
    ///
    /// Returns Err if the qubit count exceeds the DensityMatrix limit.
    pub fn try_to_density_matrix(&self) -> Result<DensityMatrix, String> {
        DensityMatrix::from_statevector(&self.amplitudes)
    }
}

// =============================================================================
// QuantumState trait implementation
// =============================================================================

impl QuantumState for Statevector {
    fn num_qubits(&self) -> u8 {
        self.num_qubits
    }

    fn dimension(&self) -> usize {
        Statevector::dimension(self)
    }

    fn diagonal_probabilities(&self) -> Vec<f64> {
        Statevector::diagonal_probabilities(self)
    }

    fn purity(&self) -> f64 {
        1.0
    }
}
