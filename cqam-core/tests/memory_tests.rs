//! Tests for `CMem` and `QMem<DensityMatrix>` covering store/load,
//! boundary addresses, take, overwrite, and emptiness checks.

use cqam_core::memory::{CMem, QMem};
use cqam_sim::density_matrix::DensityMatrix;

// --- CMem --------------------------------------------------------------------

#[test]
fn test_cmem_new_is_zeroed() {
    let mem = CMem::new();
    assert_eq!(mem.load(0), 0);
    assert_eq!(mem.load(65535), 0);
}

#[test]
fn test_cmem_store_and_load() {
    let mut mem = CMem::new();
    mem.store(100, 42);
    assert_eq!(mem.load(100), 42);
    assert_eq!(mem.load(101), 0); // adjacent cell unchanged
}

#[test]
fn test_cmem_overwrite() {
    let mut mem = CMem::new();
    mem.store(500, 10);
    mem.store(500, 20);
    assert_eq!(mem.load(500), 20);
}

#[test]
fn test_cmem_negative_values() {
    let mut mem = CMem::new();
    mem.store(0, -12345);
    assert_eq!(mem.load(0), -12345);
}

#[test]
fn test_cmem_max_address() {
    let mut mem = CMem::new();
    mem.store(65535, 999);
    assert_eq!(mem.load(65535), 999);
}

#[test]
fn test_cmem_non_zero_entries() {
    let mut mem = CMem::new();
    mem.store(0, 1);
    mem.store(1000, 2);
    mem.store(65535, 3);

    let entries: Vec<(u16, i64)> = mem.non_zero_entries().collect();
    assert_eq!(entries.len(), 3);
    assert!(entries.contains(&(0, 1)));
    assert!(entries.contains(&(1000, 2)));
    assert!(entries.contains(&(65535, 3)));
}

#[test]
fn test_cmem_is_empty_when_new() {
    let mem = CMem::new();
    assert!(mem.is_empty());
}

#[test]
fn test_cmem_is_not_empty_after_write() {
    let mut mem = CMem::new();
    mem.store(42, 1);
    assert!(!mem.is_empty());
}

#[test]
fn test_cmem_len() {
    let mem = CMem::new();
    assert_eq!(mem.len(), 65536);
}

#[test]
fn test_cmem_default() {
    let mem = CMem::default();
    assert_eq!(mem.len(), 65536);
    assert!(mem.is_empty());
}

// --- QMem --------------------------------------------------------------------

#[test]
fn test_qmem_new_all_empty() {
    let qmem: QMem<DensityMatrix> = QMem::new();
    for addr in 0..=255u8 {
        assert!(qmem.load(addr).is_none());
    }
}

#[test]
fn test_qmem_store_and_load() {
    let mut qmem: QMem<DensityMatrix> = QMem::new();
    let dm = DensityMatrix::new_zero_state(2);

    qmem.store(10, dm);

    let loaded = qmem.load(10).unwrap();
    assert_eq!(loaded.num_qubits(), 2);
    assert_eq!(loaded.dimension(), 4);
    // Zero state: rho[0][0] = 1.0
    assert!((loaded.get(0, 0).0 - 1.0).abs() < 1e-10);
}

#[test]
fn test_qmem_take_removes_slot() {
    let mut qmem: QMem<DensityMatrix> = QMem::new();
    let dm = DensityMatrix::new_uniform(2);
    qmem.store(5, dm);

    assert!(qmem.is_occupied(5));
    let taken = qmem.take(5);
    assert!(taken.is_some());
    assert!(!qmem.is_occupied(5));
    assert!(qmem.load(5).is_none());
}

#[test]
fn test_qmem_take_returns_correct_value() {
    let mut qmem: QMem<DensityMatrix> = QMem::new();
    let dm = DensityMatrix::new_bell();
    qmem.store(42, dm);

    let taken = qmem.take(42).unwrap();
    assert_eq!(taken.num_qubits(), 2);
    // Bell state: rho[0][0] = 0.5
    assert!((taken.get(0, 0).0 - 0.5).abs() < 1e-10);
}

#[test]
fn test_qmem_take_empty_slot_returns_none() {
    let mut qmem: QMem<DensityMatrix> = QMem::new();
    assert!(qmem.take(100).is_none());
}

#[test]
fn test_qmem_is_occupied() {
    let mut qmem: QMem<DensityMatrix> = QMem::new();
    assert!(!qmem.is_occupied(0));
    qmem.store(0, DensityMatrix::new_zero_state(1));
    assert!(qmem.is_occupied(0));
}

#[test]
fn test_qmem_overwrite() {
    let mut qmem: QMem<DensityMatrix> = QMem::new();
    qmem.store(0, DensityMatrix::new_zero_state(1));
    qmem.store(0, DensityMatrix::new_uniform(2));

    let loaded = qmem.load(0).unwrap();
    assert_eq!(loaded.num_qubits(), 2);
}

#[test]
fn test_qmem_max_address() {
    let mut qmem: QMem<DensityMatrix> = QMem::new();
    qmem.store(255, DensityMatrix::new_zero_state(1));
    assert!(qmem.is_occupied(255));
    let loaded = qmem.load(255).unwrap();
    assert_eq!(loaded.num_qubits(), 1);
}

#[test]
fn test_qmem_default() {
    let qmem: QMem<DensityMatrix> = QMem::default();
    assert!(!qmem.is_occupied(0));
    assert!(!qmem.is_occupied(255));
}
