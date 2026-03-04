// cqam-sim/src/joint_qdist.rs
//
// Phase 6.2: Joint probability distribution for entangled quantum register pairs.
// When one register is measured, the other's distribution is updated to the
// conditional marginal.

use rand::Rng;
use crate::qdist::QDist;

/// A joint probability distribution over two entangled quantum registers.
///
/// Represents correlated distributions where measuring one register
/// updates the other's distribution to the conditional marginal.
#[derive(Debug, Clone)]
pub struct JointQDist {
    /// Labels for the two registers (e.g., ("Q0", "Q1")).
    pub labels: (String, String),

    /// Domain: pairs of (state_a, state_b) basis states.
    pub domain: Vec<(u16, u16)>,

    /// Probability of each (state_a, state_b) pair.
    pub probabilities: Vec<f64>,
}

impl JointQDist {
    /// Create a new joint distribution from domain pairs and probabilities.
    ///
    /// The probabilities are normalized automatically.
    pub fn new(
        labels: (String, String),
        domain: Vec<(u16, u16)>,
        probabilities: Vec<f64>,
    ) -> Result<Self, String> {
        if domain.len() != probabilities.len() {
            return Err(format!(
                "JointQDist: domain length ({}) != probabilities length ({})",
                domain.len(), probabilities.len()
            ));
        }
        let mut jd = Self {
            labels,
            domain,
            probabilities,
        };
        jd.normalize();
        Ok(jd)
    }

    /// Create a joint distribution from two independent QDist<u16> distributions.
    ///
    /// The joint probability is the product of marginal probabilities:
    /// P(a, b) = P(a) * P(b).
    pub fn from_independent(a: &QDist<u16>, b: &QDist<u16>) -> Result<Self, String> {
        let mut domain = Vec::with_capacity(a.domain.len() * b.domain.len());
        let mut probs = Vec::with_capacity(a.domain.len() * b.domain.len());

        for (i, &sa) in a.domain.iter().enumerate() {
            for (j, &sb) in b.domain.iter().enumerate() {
                domain.push((sa, sb));
                probs.push(a.probabilities[i] * b.probabilities[j]);
            }
        }

        Self::new(
            (a.label.clone(), b.label.clone()),
            domain,
            probs,
        )
    }

    /// Normalize the probability distribution so probabilities sum to 1.
    pub fn normalize(&mut self) {
        let total: f64 = self.probabilities.iter().sum();
        if total > 0.0 {
            for p in self.probabilities.iter_mut() {
                *p /= total;
            }
        }
    }

    /// Compute the marginal distribution for register A (first register).
    ///
    /// Sums over all values of register B for each value of register A.
    pub fn marginal_a(&self) -> QDist<u16> {
        let mut state_map: Vec<(u16, f64)> = Vec::new();

        for (i, &(sa, _sb)) in self.domain.iter().enumerate() {
            if let Some(entry) = state_map.iter_mut().find(|(s, _)| *s == sa) {
                entry.1 += self.probabilities[i];
            } else {
                state_map.push((sa, self.probabilities[i]));
            }
        }

        // Sort by state value for deterministic ordering
        state_map.sort_by_key(|&(s, _)| s);

        let domain: Vec<u16> = state_map.iter().map(|&(s, _)| s).collect();
        let probs: Vec<f64> = state_map.iter().map(|&(_, p)| p).collect();

        QDist::new(&self.labels.0, domain, probs)
            .expect("internal: marginal domain/probability mismatch")
    }

    /// Compute the marginal distribution for register B (second register).
    ///
    /// Sums over all values of register A for each value of register B.
    pub fn marginal_b(&self) -> QDist<u16> {
        let mut state_map: Vec<(u16, f64)> = Vec::new();

        for (i, &(_sa, sb)) in self.domain.iter().enumerate() {
            if let Some(entry) = state_map.iter_mut().find(|(s, _)| *s == sb) {
                entry.1 += self.probabilities[i];
            } else {
                state_map.push((sb, self.probabilities[i]));
            }
        }

        // Sort by state value for deterministic ordering
        state_map.sort_by_key(|&(s, _)| s);

        let domain: Vec<u16> = state_map.iter().map(|&(s, _)| s).collect();
        let probs: Vec<f64> = state_map.iter().map(|&(_, p)| p).collect();

        QDist::new(&self.labels.1, domain, probs)
            .expect("internal: marginal domain/probability mismatch")
    }

    /// Stochastically measure the joint distribution.
    ///
    /// Returns a sampled (state_a, state_b) pair according to the joint
    /// probability distribution.
    pub fn measure(&self) -> Option<(u16, u16)> {
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

        // Fallback to last element
        Some(self.domain[self.domain.len() - 1])
    }

    /// Deterministic measurement: return the pair with highest probability.
    pub fn measure_deterministic(&self) -> Option<(u16, u16)> {
        let max_idx = self
            .probabilities
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))?
            .0;
        Some(self.domain[max_idx])
    }

    /// Measure register A and return its observed value along with the
    /// conditional distribution of register B given the observed value.
    ///
    /// This simulates the quantum effect: measuring one entangled register
    /// collapses the other to a conditional distribution.
    pub fn measure_a(&self) -> Option<(u16, QDist<u16>)> {
        // First, sample the marginal of A
        let marginal_a = self.marginal_a();
        use crate::qdist::Measurable;
        let observed_a = marginal_a.measure()?;

        // Now compute the conditional distribution P(B | A = observed_a)
        let conditional_b = self.conditional_b_given_a(observed_a);
        Some((observed_a, conditional_b))
    }

    /// Measure register B and return its observed value along with the
    /// conditional distribution of register A given the observed value.
    pub fn measure_b(&self) -> Option<(u16, QDist<u16>)> {
        let marginal_b = self.marginal_b();
        use crate::qdist::Measurable;
        let observed_b = marginal_b.measure()?;

        let conditional_a = self.conditional_a_given_b(observed_b);
        Some((observed_b, conditional_a))
    }

    /// Compute P(B | A = a): the conditional distribution of register B
    /// given that register A was observed to be `a`.
    pub fn conditional_b_given_a(&self, a: u16) -> QDist<u16> {
        let mut state_map: Vec<(u16, f64)> = Vec::new();

        for (i, &(sa, sb)) in self.domain.iter().enumerate() {
            if sa == a {
                if let Some(entry) = state_map.iter_mut().find(|(s, _)| *s == sb) {
                    entry.1 += self.probabilities[i];
                } else {
                    state_map.push((sb, self.probabilities[i]));
                }
            }
        }

        state_map.sort_by_key(|&(s, _)| s);

        let domain: Vec<u16> = state_map.iter().map(|&(s, _)| s).collect();
        let probs: Vec<f64> = state_map.iter().map(|&(_, p)| p).collect();

        let mut result = QDist::new(&self.labels.1, domain, probs)
            .expect("internal: conditional domain/probability mismatch");
        result.normalize();
        result
    }

    /// Compute P(A | B = b): the conditional distribution of register A
    /// given that register B was observed to be `b`.
    pub fn conditional_a_given_b(&self, b: u16) -> QDist<u16> {
        let mut state_map: Vec<(u16, f64)> = Vec::new();

        for (i, &(sa, sb)) in self.domain.iter().enumerate() {
            if sb == b {
                if let Some(entry) = state_map.iter_mut().find(|(s, _)| *s == sa) {
                    entry.1 += self.probabilities[i];
                } else {
                    state_map.push((sa, self.probabilities[i]));
                }
            }
        }

        state_map.sort_by_key(|&(s, _)| s);

        let domain: Vec<u16> = state_map.iter().map(|&(s, _)| s).collect();
        let probs: Vec<f64> = state_map.iter().map(|&(_, p)| p).collect();

        let mut result = QDist::new(&self.labels.0, domain, probs)
            .expect("internal: conditional domain/probability mismatch");
        result.normalize();
        result
    }
}
