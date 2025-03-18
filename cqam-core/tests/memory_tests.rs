// cqam-core/tests/memory_tests.rs

use cqam_core::memory::{CMEM, QMEM};
use cqam_sim::qdist::QDist;

#[test]
fn test_cmem_store_and_load() {
    let mut mem = CMEM::new();
    mem.store("addr1", 123);
    assert_eq!(mem.load("addr1"), Some(&123));
}

#[test]
fn test_qmem_insert_and_get() {
    let mut qmem: QMEM<i32> = QMEM::new();
    let domain = vec![0, 1, 2];
    let probabilities = vec![0.3, 0.4, 0.3];
    let qdist = QDist::new("qX", domain.clone(), probabilities.clone());

    qmem.insert("qX", qdist.clone());

    let retrieved = qmem.get("qX").unwrap();
    assert_eq!(retrieved.label, "qX");
    assert_eq!(retrieved.domain, domain);
    assert_eq!(retrieved.probabilities, probabilities);
}
