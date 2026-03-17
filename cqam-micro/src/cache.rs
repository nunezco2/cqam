//! Circuit template caching with LRU eviction.
//!
//! Stores compiled native circuits keyed by structure hash, enabling
//! fast reuse when the same circuit structure is compiled multiple times
//! with different parameter values.

use cqam_core::native_ir::{self, Circuit};

/// A cached compiled circuit template.
pub struct CachedCircuit {
    /// The compiled native circuit with parameter slots.
    pub template: Circuit,
    /// Indices of ops that contain parameterized Rz gates.
    pub param_slots: Vec<usize>,
}

/// LRU circuit template cache using a simple Vec-based approach.
pub struct CircuitCache {
    /// Entries ordered by recency: most-recently-used at the end.
    entries: Vec<(u64, CachedCircuit)>,
    /// Maximum number of entries.
    capacity: usize,
    /// Cache statistics.
    hits: u64,
    misses: u64,
}

impl CircuitCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            hits: 0,
            misses: 0,
        }
    }

    /// Look up a circuit template by structure key.
    /// On hit, moves the entry to the end (most recent).
    pub fn lookup(&mut self, key: u64) -> Option<&CachedCircuit> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| *k == key) {
            self.hits += 1;
            // Move to end (most recent)
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            Some(&self.entries.last().unwrap().1)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert a compiled circuit template. Evicts LRU entry if at capacity.
    pub fn insert(&mut self, key: u64, circuit: CachedCircuit) {
        // Remove existing entry with same key if present
        if let Some(pos) = self.entries.iter().position(|(k, _)| *k == key) {
            self.entries.remove(pos);
        }
        if self.entries.len() >= self.capacity {
            self.entries.remove(0); // evict LRU (oldest = front)
        }
        self.entries.push((key, circuit));
    }

    /// Cache hit rate as a fraction in [0.0, 1.0].
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    /// Number of cache hits.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Number of cache misses.
    pub fn misses(&self) -> u64 {
        self.misses
    }
}

/// Rebind parameters in a cached template using new parameter values.
pub fn rebind(cached: &CachedCircuit, new_params: &[f64]) -> Circuit {
    let mut circuit = cached.template.clone();
    let count = cached.param_slots.len().min(new_params.len());
    for (i, &new_val) in new_params.iter().enumerate().take(count) {
        let op_idx = cached.param_slots[i];
        if op_idx < circuit.ops.len() {
            if let native_ir::Op::Gate1q(ref mut g) = circuit.ops[op_idx] {
                if let native_ir::NativeGate1::Rz(ref mut angle) = g.gate {
                    *angle = new_val;
                }
            }
        }
    }
    circuit
}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::native_ir::{Circuit, Op, NativeGate1, ApplyGate1q, PhysicalQubit};

    fn make_template() -> CachedCircuit {
        let mut c = Circuit::new(2);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Rz(1.0),
        }));
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Sx,
        }));
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(1),
            gate: NativeGate1::Rz(2.0),
        }));
        CachedCircuit {
            template: c,
            param_slots: vec![0, 2], // indices of Rz ops
        }
    }

    #[test]
    fn test_cache_insert_and_lookup() {
        let mut cache = CircuitCache::new(10);
        cache.insert(42, make_template());
        assert!(cache.lookup(42).is_some());
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = CircuitCache::new(10);
        assert!(cache.lookup(99).is_none());
    }

    #[test]
    fn test_cache_lru_eviction() {
        let mut cache = CircuitCache::new(2);
        cache.insert(1, make_template());
        cache.insert(2, make_template());
        cache.insert(3, make_template()); // evicts key 1
        assert!(cache.lookup(1).is_none());
        assert!(cache.lookup(2).is_some());
        assert!(cache.lookup(3).is_some());
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut cache = CircuitCache::new(10);
        cache.insert(1, make_template());
        let _ = cache.lookup(1); // hit
        let _ = cache.lookup(2); // miss
        assert!((cache.hit_rate() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_cache_same_key_updates() {
        let mut cache = CircuitCache::new(10);
        cache.insert(1, make_template());
        cache.insert(1, make_template());
        // Should only have one entry
        assert!(cache.lookup(1).is_some());
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn test_cache_lookup_promotes_to_mru() {
        let mut cache = CircuitCache::new(3);
        cache.insert(1, make_template());
        cache.insert(2, make_template());
        cache.insert(3, make_template());
        // Access key 1, promoting it
        let _ = cache.lookup(1);
        // Insert key 4, should evict key 2 (now LRU)
        cache.insert(4, make_template());
        assert!(cache.lookup(1).is_some());
        assert!(cache.lookup(2).is_none());
    }

    #[test]
    fn test_rebind_params() {
        let cached = make_template();
        let rebound = rebind(&cached, &[10.0, 20.0]);
        // Check that Rz angles were updated
        if let Op::Gate1q(g) = &rebound.ops[0] {
            if let NativeGate1::Rz(angle) = g.gate {
                assert!((angle - 10.0).abs() < 1e-10);
            }
        }
        if let Op::Gate1q(g) = &rebound.ops[2] {
            if let NativeGate1::Rz(angle) = g.gate {
                assert!((angle - 20.0).abs() < 1e-10);
            }
        }
    }
}
