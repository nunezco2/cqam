//! Shot histogram type for `--shots N` QPU-realistic sampling.

use std::collections::BTreeMap;

/// A histogram of measurement outcomes from N shots.
///
/// Keys are basis state indices (u32), values are counts.
/// The sum of all counts equals the total number of shots.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotHistogram {
    /// Outcome counts: basis_state -> count.
    pub counts: BTreeMap<u32, u32>,
    /// Total number of shots (sum of all counts).
    pub total_shots: u32,
}

impl ShotHistogram {
    pub fn new(total_shots: u32) -> Self {
        Self {
            counts: BTreeMap::new(),
            total_shots,
        }
    }

    /// Record a single shot outcome.
    pub fn record(&mut self, outcome: u32) {
        *self.counts.entry(outcome).or_insert(0) += 1;
    }

    /// Get the empirical probability of a basis state.
    pub fn probability(&self, state: u32) -> f64 {
        let count = self.counts.get(&state).copied().unwrap_or(0);
        count as f64 / self.total_shots as f64
    }

    /// Convert to a probability distribution (same format as HybridValue::Dist).
    pub fn to_dist(&self) -> Vec<(u32, f64)> {
        self.counts
            .iter()
            .map(|(&state, &count)| (state, count as f64 / self.total_shots as f64))
            .collect()
    }

    /// Number of distinct outcomes observed.
    pub fn num_outcomes(&self) -> usize {
        self.counts.len()
    }
}
