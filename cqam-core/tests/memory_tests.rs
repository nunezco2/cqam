// cqam-core/tests/memory_tests.rs

use cqam_core::memory::{CMEM, QMEM};

#[test]
fn test_cmem_store_and_load() {
    let mut mem = CMEM::new();
    mem.store("addr1", 123);
    assert_eq!(mem.load("addr1"), Some(&123));
}

#[test]
fn test_qmem_allocation() {
    let mut qmem = QMEM::new();
    qmem.allocate("qX", 4);
    let qd = qmem.get("qX").unwrap();
    assert_eq!(qd.label, "qX");
    assert_eq!(qd.domain_size, 4);
}
