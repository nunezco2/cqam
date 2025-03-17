// cqam-core/tests/register_tests.rs

use cqam_core::register::{RegisterBank, CValue};

#[test]
fn test_register_bank_store_and_load() {
    let mut rb = RegisterBank::new();
    rb.store_c("R1", CValue::Int(42));
    assert_eq!(rb.load_c("R1"), Some(&CValue::Int(42)));
}
