//! Shot-mode sampling utilities and result types.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use cqam_core::shot::ShotHistogram;
use cqam_vm::context::ExecutionContext;

/// Result of program execution -- either exact or shot-sampled.
pub enum RunResult {
    /// Exact simulation (no --shots).
    Exact(ExecutionContext),
    /// Shot-sampled execution (--shots N). Histograms live in H registers as HybridValue::Hist.
    Shots(ExecutionContext),
}

impl RunResult {
    /// Get a reference to the underlying ExecutionContext.
    pub fn ctx(&self) -> &ExecutionContext {
        match self {
            RunResult::Exact(c) | RunResult::Shots(c) => c,
        }
    }
}

/// Sample one outcome from a probability distribution.
pub fn sample_from_dist(dist: &[(u32, f64)], rng: &mut ChaCha8Rng) -> u32 {
    let r: f64 = rng.gen_range(0.0..1.0);
    let mut cumulative = 0.0;
    for &(state, prob) in dist {
        cumulative += prob;
        if r < cumulative {
            return state;
        }
    }
    dist.last().map(|&(s, _)| s).unwrap_or(0)
}

/// Sample one outcome from a probability distribution using a seed.
pub fn sample_from_dist_seeded(dist: &[(u32, f64)], seed: u64) -> u32 {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    sample_from_dist(dist, &mut rng)
}

/// Resample an exact distribution N times into a ShotHistogram.
pub fn resample_dist(
    dist: &[(u32, f64)],
    shots: u32,
    base_seed: u64,
) -> ShotHistogram {
    let mut rng = ChaCha8Rng::seed_from_u64(base_seed);
    let mut hist = ShotHistogram::new(shots);
    for _ in 0..shots {
        let outcome = sample_from_dist(dist, &mut rng);
        hist.record(outcome);
    }
    hist
}
