// cqam-sim/src/qdist.rs

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
