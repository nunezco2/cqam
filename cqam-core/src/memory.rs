// cqam-core/src/memory.rs

/// Discrete memory model for classical and quantum data.

#[derive(Debug, Clone)]
pub struct CMEM {
    pub cells: std::collections::HashMap<String, i64>,
}

#[derive(Debug, Clone)]
pub struct QMEM {
    pub qdists: std::collections::HashMap<String, QDist>,
}

#[derive(Debug, Clone)]
pub struct HybridReg {
    pub states: std::collections::HashMap<String, f64>,
}

// Placeholder type for now until QDist is implemented in Phase 2
#[derive(Debug, Clone)]
pub struct QDist {
    pub label: String,
    pub domain_size: usize,
}

impl CMEM {
    pub fn new() -> Self {
        Self {
            cells: std::collections::HashMap::new(),
        }
    }

    pub fn load(&self, key: &str) -> Option<&i64> {
        self.cells.get(key)
    }

    pub fn store(&mut self, key: &str, val: i64) {
        self.cells.insert(key.to_string(), val);
    }
}

impl QMEM {
    pub fn new() -> Self {
        Self {
            qdists: std::collections::HashMap::new(),
        }
    }

    pub fn allocate(&mut self, label: &str, size: usize) {
        self.qdists.insert(label.to_string(), QDist {
            label: label.to_string(),
            domain_size: size,
        });
    }

    pub fn get(&self, label: &str) -> Option<&QDist> {
        self.qdists.get(label)
    }
}
