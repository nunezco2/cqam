#[derive(Debug, Clone)]
pub struct QDist<T> {
    pub label: String,
    pub domain: Vec<T>,
    pub probabilities: Vec<f64>,
}

impl<T: Clone> QDist<T> {
    pub fn new(label: &str, domain: Vec<T>, probabilities: Vec<f64>) -> Self {
        assert_eq!(domain.len(), probabilities.len(), "Domain and probability size mismatch");
        Self {
            label: label.to_string(),
            domain,
            probabilities,
        }
    }

    pub fn normalize(&mut self) {
        let total: f64 = self.probabilities.iter().sum();
        if total > 0.0 {
            for p in self.probabilities.iter_mut() {
                *p /= total;
            }
        }
    }
}

/// Trait defining classical measurement semantics from a QDist
pub trait Measurable<TOut> {
    fn measure(&self) -> Option<TOut>;
    fn expected_value(&self) -> Option<f64>;
}

/// Implement Measurable<i64> for QDist<i32>
impl Measurable<i64> for QDist<i32> {
    fn measure(&self) -> Option<i64> {
        let max_idx = self
            .probabilities
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())?
            .0;
        Some(self.domain[max_idx] as i64)
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
