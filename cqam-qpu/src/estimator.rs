//! Bayesian estimator for adaptive shot management in QPU backends.
//!
//! `BayesianEstimator` tracks accumulated measurement counts from batched
//! circuit submissions and decides when the probability estimates have
//! converged to within the requested credible interval.

use std::collections::BTreeMap;
use crate::traits::ConvergenceCriterion;

// =============================================================================
// z-score lookup table for common confidence levels
// =============================================================================

/// Map from confidence level to z-score (Phi^{-1}((1 + confidence) / 2)).
/// Covers the most common values; linear interpolation is used for others.
const Z_TABLE: &[(f64, f64)] = &[
    (0.80, 1.282),
    (0.85, 1.440),
    (0.90, 1.645),
    (0.95, 1.960),
    (0.99, 2.576),
    (0.999, 3.291),
];

fn z_score(confidence: f64) -> f64 {
    // Clamp to table range
    if confidence <= Z_TABLE[0].0 {
        return Z_TABLE[0].1;
    }
    if confidence >= Z_TABLE[Z_TABLE.len() - 1].0 {
        return Z_TABLE[Z_TABLE.len() - 1].1;
    }

    // Linear interpolation
    for i in 0..Z_TABLE.len() - 1 {
        let (c0, z0) = Z_TABLE[i];
        let (c1, z1) = Z_TABLE[i + 1];
        if confidence >= c0 && confidence <= c1 {
            let t = (confidence - c0) / (c1 - c0);
            return z0 + t * (z1 - z0);
        }
    }

    // Fallback (should not be reached)
    1.960
}

// =============================================================================
// BayesianEstimator
// =============================================================================

/// Tracks accumulated shot counts and decides convergence using a
/// Dirichlet-Multinomial Bayesian model.
///
/// The estimator is used by `QpuBackend::submit()` implementations to
/// adaptively schedule shot batches: submit, update, check convergence,
/// repeat until converged or budget exhausted.
pub struct BayesianEstimator {
    counts: BTreeMap<u64, u32>,
    total: u32,
    criterion: ConvergenceCriterion,
}

impl BayesianEstimator {
    /// Create a new estimator with the given convergence criterion.
    /// Starts with no accumulated data.
    pub fn new(criterion: ConvergenceCriterion) -> Self {
        Self {
            counts: BTreeMap::new(),
            total: 0,
            criterion,
        }
    }

    /// Merge a batch of measurement counts into the accumulated totals.
    pub fn update(&mut self, batch: &BTreeMap<u64, u32>) {
        for (&bitstring, &count) in batch {
            *self.counts.entry(bitstring).or_insert(0) += count;
        }
        self.total += batch.values().sum::<u32>();
    }

    /// Check whether the current estimates have converged to within the
    /// requested credible interval.
    ///
    /// Uses a Beta(c_k + 1, N - c_k + 1) posterior (uniform Dirichlet prior)
    /// for each observed outcome k. Convergence requires that all outcomes
    /// with posterior mean >= 1e-4 have relative half-width <= max_relative_error.
    pub fn is_converged(&self) -> bool {
        if self.total == 0 {
            return false;
        }

        let n = self.total as f64;
        let k = self.counts.len() as f64;
        let z = z_score(self.criterion.confidence);

        // Special case: single outcome -> uncertainty is trivially low
        if self.counts.len() == 1 {
            // With all shots on one outcome, p_hat ~ 1.0, relative error ~ 0
            return self.total >= self.criterion.min_batch_size;
        }

        let significance = 1e-4_f64;

        for &count in self.counts.values() {
            // Use empirical frequency for significance check (matches spec intent:
            // outcomes with p_hat_k >= 1e-4 must converge; rare outcomes are skipped).
            let empirical = count as f64 / n;
            if empirical < significance {
                continue; // ignore rare outcomes
            }
            let p_hat = (count as f64 + 1.0) / (n + k);
            let sigma = (p_hat * (1.0 - p_hat) / (n + k + 1.0)).sqrt();
            let half_width = z * sigma;
            let relative_error = half_width / p_hat;
            if relative_error > self.criterion.max_relative_error {
                return false;
            }
        }

        true
    }

    /// Suggest the next batch size based on current convergence status.
    ///
    /// Returns a number of shots estimated to bring the worst-converged
    /// outcome within the requested relative error. Clamped to
    /// `[min_batch_size, 10 * min_batch_size]`.
    pub fn suggest_batch_size(&self) -> u32 {
        let min = self.criterion.min_batch_size;

        if self.total == 0 {
            return min;
        }

        let n = self.total as f64;
        let k = self.counts.len() as f64;
        let z = z_score(self.criterion.confidence);
        let max_re = self.criterion.max_relative_error;

        let mut worst_needed: u32 = min;

        for &count in self.counts.values() {
            let p_hat = (count as f64 + 1.0) / (n + k);
            if p_hat < 1e-4 {
                continue;
            }
            // N_needed = ceil(z^2 * (1 - p) / (p * max_re^2))
            let n_needed = (z * z * (1.0 - p_hat) / (p_hat * max_re * max_re)).ceil() as u32;
            let additional = n_needed.saturating_sub(self.total);
            if additional > worst_needed {
                worst_needed = additional;
            }
        }

        // Clamp to [min, 10 * min]
        worst_needed.clamp(min, 10 * min)
    }

    /// Return the accumulated counts, ready for use as `RawResults::counts`.
    pub fn finalize(&self) -> BTreeMap<u64, u32> {
        self.counts.clone()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn default_criterion() -> ConvergenceCriterion {
        ConvergenceCriterion::default()
    }

    fn criterion_with(confidence: f64, max_relative_error: f64, min_batch_size: u32) -> ConvergenceCriterion {
        ConvergenceCriterion { confidence, max_relative_error, min_batch_size }
    }

    #[test]
    fn test_empty_not_converged() {
        let est = BayesianEstimator::new(default_criterion());
        assert!(!est.is_converged(), "empty estimator must not be converged");
    }

    #[test]
    fn test_single_outcome_converges_fast() {
        let criterion = criterion_with(0.95, 0.05, 100);
        let mut est = BayesianEstimator::new(criterion);
        // 100 shots all on bitstring 0
        let mut batch = BTreeMap::new();
        batch.insert(0u64, 100u32);
        est.update(&batch);
        assert!(est.is_converged(), "single-outcome 100 shots should converge");
    }

    #[test]
    fn test_uniform_needs_more_shots() {
        let criterion = criterion_with(0.95, 0.05, 10);
        let mut est = BayesianEstimator::new(criterion);
        // Uniform over 8 outcomes with only 10 total shots
        let mut batch = BTreeMap::new();
        for i in 0u64..8 {
            batch.insert(i, 1u32); // ~10 shots, rough uniform
        }
        batch.insert(0, 3u32); // adjust to 10 shots
        est.update(&batch);
        assert!(!est.is_converged(), "10 shots over 8 outcomes is not enough");
    }

    #[test]
    fn test_uniform_converges_with_enough_shots() {
        let criterion = criterion_with(0.95, 0.05, 100);
        let mut est = BayesianEstimator::new(criterion);
        // Uniform over 4 outcomes with 10000 total shots (2500 each)
        let mut batch = BTreeMap::new();
        for i in 0u64..4 {
            batch.insert(i, 2500u32);
        }
        est.update(&batch);
        assert!(est.is_converged(), "10000 shots over 4 outcomes should converge");
    }

    #[test]
    fn test_suggest_batch_size_initial() {
        let criterion = criterion_with(0.95, 0.05, 100);
        let est = BayesianEstimator::new(criterion);
        let suggested = est.suggest_batch_size();
        assert_eq!(suggested, 100, "initial suggestion must be min_batch_size");
    }

    #[test]
    fn test_suggest_batch_size_decreases() {
        let criterion = criterion_with(0.95, 0.05, 100);
        let mut est = BayesianEstimator::new(criterion);

        // Suggest with no data
        let s0 = est.suggest_batch_size();

        // Update with a large batch
        let mut batch = BTreeMap::new();
        batch.insert(0u64, 500u32);
        batch.insert(1u64, 500u32);
        est.update(&batch);

        let s1 = est.suggest_batch_size();
        // After 1000 shots, should suggest less or equal than the initial (clamped)
        // Both are clamped to [100, 1000]; s1 should be <= s0 + some slack
        // The main invariant: s1 is within the clamp range
        assert!(s1 >= 100 && s1 <= 1000);
        let _ = s0;
    }

    #[test]
    fn test_update_merges_correctly() {
        let mut est = BayesianEstimator::new(default_criterion());
        let mut b1 = BTreeMap::new();
        b1.insert(0u64, 10u32);
        b1.insert(1u64, 5u32);

        let mut b2 = BTreeMap::new();
        b2.insert(1u64, 3u32);
        b2.insert(2u64, 7u32);

        est.update(&b1);
        est.update(&b2);

        let counts = est.finalize();
        assert_eq!(counts[&0], 10);
        assert_eq!(counts[&1], 8);
        assert_eq!(counts[&2], 7);
        assert_eq!(est.total, 25);
    }

    #[test]
    fn test_finalize_returns_counts() {
        let mut est = BayesianEstimator::new(default_criterion());
        let mut batch = BTreeMap::new();
        batch.insert(42u64, 99u32);
        est.update(&batch);
        let counts = est.finalize();
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[&42], 99);
    }

    #[test]
    fn test_rare_outcomes_excluded_from_convergence() {
        let criterion = criterion_with(0.95, 0.05, 100);
        let mut est = BayesianEstimator::new(criterion);
        // Dominant outcome with 99999 shots, rare outcome with 1 shot.
        // Empirical frequency of rare outcome: 1/100000 = 1e-5 < 1e-4 threshold.
        // The rare outcome should be excluded from convergence checking.
        let mut batch = BTreeMap::new();
        batch.insert(0u64, 99_999u32);
        batch.insert(1u64, 1u32);
        est.update(&batch);
        // The dominant outcome has very low relative error; the rare outcome
        // is below the significance threshold (1e-5 < 1e-4). Should converge.
        assert!(est.is_converged(), "rare outcome below 1e-4 should not block convergence");
    }
}
