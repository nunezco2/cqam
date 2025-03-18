// cqam-core/src/memory.rs

use cqam_sim::qdist::QDist;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CMEM {
    pub cells: HashMap<String, i64>,
}

#[derive(Debug, Clone)]
pub struct QMEM<T> {
    pub qdists: HashMap<String, QDist<T>>,
}

#[derive(Debug, Clone)]
pub struct HybridReg {
    pub states: HashMap<String, f64>,
}

impl CMEM {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
        }
    }

    pub fn load(&self, key: &str) -> Option<&i64> {
        self.cells.get(key)
    }

    pub fn store(&mut self, key: &str, val: i64) {
        self.cells.insert(key.to_string(), val);
    }
}

impl<T> QMEM<T> {
    pub fn new() -> Self {
        Self {
            qdists: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&QDist<T>> {
        self.qdists.get(key)
    }

    pub fn insert(&mut self, key: &str, val: QDist<T>) {
        self.qdists.insert(key.to_string(), val);
    }
}
