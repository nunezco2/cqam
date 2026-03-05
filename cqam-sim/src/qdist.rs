//! Probability distribution type (`QDist`) for quantum measurement outcomes.
//!
//! [`QDist<T>`] is a discrete probability distribution over a domain of values
//! of type `T`. In the CQAM context `T = u16` (basis state indices), and
//! distributions arise from the diagonal of a density matrix after a
//! `QOBSERVE` instruction.
//!
//! The [`Measurable`] trait provides stochastic (`measure`) and deterministic
//! (`measure_deterministic`) sampling, plus an `expected_value` computation.
//!
//! Fidelity helpers are implemented on `QDist<u16>`:
//! - [`QDist::superposition_metric`] — normalised Shannon entropy H/log2(n).
//! - [`QDist::concentration_metric`] — inverse HHI, measuring spread.

use rand::Rng;

/// A discrete probability distribution over a typed domain.
///
/// `QDist<T>` pairs a domain `Vec<T>` with a parallel `Vec<f64>` of
/// probabilities. In the CQAM context `T` is almost always `u16` (basis state
/// indices 0..2^n), and distributions are produced by reading the diagonal of
/// a [`cqam_sim::density_matrix::DensityMatrix`] after quantum measurement.
///
/// Invariant: `domain.len() == probabilities.len()` (enforced by [`QDist::new`]).
/// The probabilities are not automatically normalised; call [`QDist::normalize`]
/// if needed.
#[derive(Debug, Clone)]
pub struct QDist<T> {
    /// Human-readable label for display and debugging.
    pub label: String,
    /// Ordered list of outcome values.
    pub domain: Vec<T>,
    /// Probabilities parallel to `domain`; should sum to 1.0 after normalisation.
    pub probabilities: Vec<f64>,
}

impl<T: Clone> QDist<T> {
    /// Construct a new distribution.
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` if `domain.len() != probabilities.len()`.
    pub fn new(label: &str, domain: Vec<T>, probabilities: Vec<f64>) -> Result<Self, String> {
        if domain.len() != probabilities.len() {
            return Err(format!(
                "QDist '{}': domain length ({}) != probabilities length ({})",
                label, domain.len(), probabilities.len()
            ));
        }
        Ok(Self {
            label: label.to_string(),
            domain,
            probabilities,
        })
    }

    /// Normalise probabilities in-place so they sum to 1.0.
    ///
    /// If the total is zero (all probabilities are zero), the distribution is
    /// left unchanged.
    pub fn normalize(&mut self) {
        let total: f64 = self.probabilities.iter().sum();
        if total > 0.0 {
            for p in self.probabilities.iter_mut() {
                *p /= total;
            }
        }
    }
}

// --- Fidelity metrics ---

impl QDist<u16> {
    /// Shannon entropy of the probability distribution, normalized to [0,1].
    ///
    /// Returns H / log2(n), where H = -sum(p * log2(p)) and n is the number
    /// of states. A value of 0.0 means a delta distribution (no superposition),
    /// and 1.0 means maximally spread (uniform).
    ///
    /// Returns 0.0 for distributions with 0 or 1 states.
    pub fn superposition_metric(&self) -> f64 {
        let n = self.probabilities.len();
        if n <= 1 {
            return 0.0;
        }

        let entropy: f64 = self.probabilities.iter()
            .filter(|&&p| p > 0.0)
            .map(|&p| -p * p.log2())
            .sum();

        let max_entropy = (n as f64).log2();
        if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        }
    }

    /// Concentration metric based on the inverse Herfindahl-Hirschman index.
    ///
    /// Computes the effective number of states (1 / sum(p^2)) divided by the
    /// total number of states. A value near 0 means concentrated (few effective
    /// states), and 1.0 means uniform (maximally spread).
    ///
    /// This measures distribution concentration, NOT quantum entanglement.
    /// For entanglement measures, see `DensityMatrix::entanglement_entropy()`.
    ///
    /// Returns 0.0 for empty distributions.
    pub fn concentration_metric(&self) -> f64 {
        let n = self.probabilities.len();
        if n == 0 {
            return 0.0;
        }

        let hhi: f64 = self.probabilities.iter().map(|&p| p * p).sum();
        if hhi > 0.0 {
            let effective_states = 1.0 / hhi;
            effective_states / n as f64
        } else {
            0.0
        }
    }
}

/// Classical measurement semantics for a probability distribution.
///
/// Implemented on [`QDist<u16>`] for basis-state distributions arising from
/// quantum register measurement.
pub trait Measurable<TOut> {
    /// Sample one outcome stochastically according to the probability weights.
    ///
    /// Returns `None` if the domain is empty.
    fn measure(&self) -> Option<TOut>;

    /// Return the outcome with the highest probability (argmax).
    ///
    /// Ties are broken by index order. Returns `None` if the domain is empty.
    fn measure_deterministic(&self) -> Option<TOut>;

    /// Compute the probability-weighted expectation E[X] = sum_k x_k * p_k.
    ///
    /// Returns `None` if the domain is empty.
    fn expected_value(&self) -> Option<f64>;
}

/// Implement Measurable<u16> for QDist<u16>.
impl Measurable<u16> for QDist<u16> {
    /// Stochastic measurement: sample from the probability distribution
    /// using weighted random selection.
    fn measure(&self) -> Option<u16> {
        if self.domain.is_empty() {
            return None;
        }

        let mut rng = rand::thread_rng();
        let r: f64 = rng.r#gen();

        let mut cumulative = 0.0;
        for (i, &p) in self.probabilities.iter().enumerate() {
            cumulative += p;
            if r < cumulative {
                return Some(self.domain[i]);
            }
        }

        // Fallback to last element (handles floating-point rounding)
        Some(self.domain[self.domain.len() - 1])
    }

    /// Deterministic measurement: return the state with the highest probability (argmax).
    /// This preserves the old behavior for testing.
    fn measure_deterministic(&self) -> Option<u16> {
        let max_idx = self
            .probabilities
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))?
            .0;
        Some(self.domain[max_idx])
    }

    fn expected_value(&self) -> Option<f64> {
        Some(
            self.domain
                .iter()
                .zip(self.probabilities.iter())
                .map(|(x, p)| *x as f64 * p)
                .sum(),
        )
    }
}
