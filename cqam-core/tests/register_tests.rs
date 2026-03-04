// cqam-core/tests/register_tests.rs
//
// Phase 2: Test the separate register file structs.

use cqam_core::register::*;

// =============================================================================
// IntRegFile
// =============================================================================

#[test]
fn test_int_reg_file_new_is_zeroed() {
    let regs = IntRegFile::new();
    for i in 0..16u8 {
        assert_eq!(regs.get(i), 0);
    }
}

#[test]
fn test_int_reg_file_set_and_get() {
    let mut regs = IntRegFile::new();
    regs.set(3, 42);
    assert_eq!(regs.get(3), 42);
    assert_eq!(regs.get(0), 0); // other registers unchanged
}

#[test]
fn test_int_reg_file_negative_values() {
    let mut regs = IntRegFile::new();
    regs.set(15, -999);
    assert_eq!(regs.get(15), -999);
}

#[test]
fn test_int_reg_file_overwrite() {
    let mut regs = IntRegFile::new();
    regs.set(5, 100);
    assert_eq!(regs.get(5), 100);
    regs.set(5, 200);
    assert_eq!(regs.get(5), 200);
}

#[test]
fn test_int_reg_file_max_values() {
    let mut regs = IntRegFile::new();
    regs.set(0, i64::MAX);
    regs.set(1, i64::MIN);
    assert_eq!(regs.get(0), i64::MAX);
    assert_eq!(regs.get(1), i64::MIN);
}

#[test]
fn test_int_reg_file_default() {
    let regs = IntRegFile::default();
    for i in 0..16u8 {
        assert_eq!(regs.get(i), 0);
    }
}

#[test]
#[should_panic]
fn test_int_reg_file_out_of_bounds_get_panics() {
    let regs = IntRegFile::new();
    regs.get(16); // should panic: only R0-R15
}

#[test]
#[should_panic]
fn test_int_reg_file_out_of_bounds_set_panics() {
    let mut regs = IntRegFile::new();
    regs.set(16, 0); // should panic
}

// =============================================================================
// FloatRegFile
// =============================================================================

#[test]
fn test_float_reg_file_new_is_zeroed() {
    let regs = FloatRegFile::new();
    for i in 0..16u8 {
        assert_eq!(regs.get(i), 0.0);
    }
}

#[test]
fn test_float_reg_file_set_and_get() {
    let mut regs = FloatRegFile::new();
    regs.set(7, 3.125);
    assert!((regs.get(7) - 3.125).abs() < 1e-10);
}

#[test]
fn test_float_reg_file_negative_values() {
    let mut regs = FloatRegFile::new();
    regs.set(0, -2.625);
    assert!((regs.get(0) - (-2.625)).abs() < 1e-10);
}

#[test]
fn test_float_reg_file_overwrite() {
    let mut regs = FloatRegFile::new();
    regs.set(3, 1.0);
    regs.set(3, 2.0);
    assert_eq!(regs.get(3), 2.0);
}

#[test]
fn test_float_reg_file_default() {
    let regs = FloatRegFile::default();
    for i in 0..16u8 {
        assert_eq!(regs.get(i), 0.0);
    }
}

#[test]
#[should_panic]
fn test_float_reg_file_out_of_bounds_panics() {
    let regs = FloatRegFile::new();
    regs.get(16);
}

// =============================================================================
// ComplexRegFile
// =============================================================================

#[test]
fn test_complex_reg_file_new_is_zeroed() {
    let regs = ComplexRegFile::new();
    for i in 0..16u8 {
        assert_eq!(regs.get(i), (0.0, 0.0));
    }
}

#[test]
fn test_complex_reg_file_set_and_get() {
    let mut regs = ComplexRegFile::new();
    regs.set(0, (1.5, -2.5));
    assert_eq!(regs.get(0), (1.5, -2.5));
}

#[test]
fn test_complex_reg_file_overwrite() {
    let mut regs = ComplexRegFile::new();
    regs.set(5, (1.0, 2.0));
    regs.set(5, (3.0, 4.0));
    assert_eq!(regs.get(5), (3.0, 4.0));
}

#[test]
fn test_complex_reg_file_default() {
    let regs = ComplexRegFile::default();
    for i in 0..16u8 {
        assert_eq!(regs.get(i), (0.0, 0.0));
    }
}

#[test]
#[should_panic]
fn test_complex_reg_file_out_of_bounds_panics() {
    let regs = ComplexRegFile::new();
    regs.get(16);
}

// =============================================================================
// HybridValue and HybridRegFile
// =============================================================================

#[test]
fn test_hybrid_value_default_is_empty() {
    let val = HybridValue::default();
    assert!(matches!(val, HybridValue::Empty));
}

#[test]
fn test_hybrid_value_int() {
    let val = HybridValue::Int(42);
    assert!(matches!(val, HybridValue::Int(42)));
}

#[test]
fn test_hybrid_value_float() {
    let val = HybridValue::Float(3.125);
    if let HybridValue::Float(f) = val {
        assert!((f - 3.125).abs() < 1e-10);
    } else {
        panic!("Expected HybridValue::Float");
    }
}

#[test]
fn test_hybrid_value_complex() {
    let val = HybridValue::Complex(1.0, -1.0);
    assert!(matches!(val, HybridValue::Complex(re, im) if re == 1.0 && im == -1.0));
}

#[test]
fn test_hybrid_value_dist() {
    let val = HybridValue::Dist(vec![(0u16, 0.5), (1, 0.5)]);
    if let HybridValue::Dist(ref d) = val {
        assert_eq!(d.len(), 2);
    } else {
        panic!("Expected HybridValue::Dist");
    }
}

#[test]
fn test_hybrid_reg_file_new_all_empty() {
    let regs = HybridRegFile::new();
    for i in 0..8u8 {
        assert!(matches!(regs.get(i), HybridValue::Empty));
    }
}

#[test]
fn test_hybrid_reg_file_set_int() {
    let mut regs = HybridRegFile::new();
    regs.set(0, HybridValue::Int(42));
    assert!(matches!(regs.get(0), HybridValue::Int(42)));
}

#[test]
fn test_hybrid_reg_file_set_dist() {
    let mut regs = HybridRegFile::new();
    let dist = vec![(0u16, 0.5), (1, 0.3), (2, 0.2)];
    regs.set(3, HybridValue::Dist(dist.clone()));
    if let HybridValue::Dist(d) = regs.get(3) {
        assert_eq!(d.len(), 3);
        assert_eq!(d[0], (0u16, 0.5));
    } else {
        panic!("Expected HybridValue::Dist");
    }
}

#[test]
fn test_hybrid_reg_file_overwrite() {
    let mut regs = HybridRegFile::new();
    regs.set(0, HybridValue::Int(1));
    regs.set(0, HybridValue::Float(2.0));
    assert!(matches!(regs.get(0), HybridValue::Float(f) if (*f - 2.0).abs() < 1e-10));
}

#[test]
fn test_hybrid_reg_file_default() {
    let regs = HybridRegFile::default();
    for i in 0..8u8 {
        assert!(matches!(regs.get(i), HybridValue::Empty));
    }
}

#[test]
#[should_panic]
fn test_hybrid_reg_file_out_of_bounds_get_panics() {
    let regs = HybridRegFile::new();
    regs.get(8); // should panic: only H0-H7
}

#[test]
#[should_panic]
fn test_hybrid_reg_file_out_of_bounds_set_panics() {
    let mut regs = HybridRegFile::new();
    regs.set(8, HybridValue::Empty); // should panic
}

// =============================================================================
// Clone and equality tests
// =============================================================================

#[test]
fn test_int_reg_file_clone() {
    let mut regs = IntRegFile::new();
    regs.set(5, 42);
    let cloned = regs.clone();
    assert_eq!(regs, cloned);
}

#[test]
fn test_float_reg_file_clone() {
    let mut regs = FloatRegFile::new();
    regs.set(5, 3.125);
    let cloned = regs.clone();
    assert_eq!(regs, cloned);
}

#[test]
fn test_complex_reg_file_clone() {
    let mut regs = ComplexRegFile::new();
    regs.set(5, (1.0, 2.0));
    let cloned = regs.clone();
    assert_eq!(regs, cloned);
}

#[test]
fn test_hybrid_reg_file_clone() {
    let mut regs = HybridRegFile::new();
    regs.set(0, HybridValue::Int(99));
    let cloned = regs.clone();
    assert_eq!(regs, cloned);
}
