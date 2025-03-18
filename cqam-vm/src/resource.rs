// cqam-vm/src/resource.rs

/// Per-instruction or per-kernel resource cost delta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResourceDelta {
    pub time: usize,
    pub space: usize,
    pub superposition: f64,
    pub entanglement: f64,
    pub interference: f64,
}

/// Tracks cumulative resource usage across execution.
#[derive(Debug, Default)]
pub struct ResourceTracker {
    pub total_time: usize,
    pub total_space: usize,
    pub total_superposition: f64,
    pub total_entanglement: f64,
    pub total_interference: f64,
}

impl ResourceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_delta(&mut self, delta: &ResourceDelta) {
        self.total_time += delta.time;
        self.total_space += delta.space;
        self.total_superposition += delta.superposition;
        self.total_entanglement += delta.entanglement;
        self.total_interference += delta.interference;
    }
}
