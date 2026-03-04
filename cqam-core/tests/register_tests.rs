// cqam-core/tests/register_tests.rs
//
// Phase 2: Test the separate register file structs.

use cqam_core::register::*;
use cqam_core::error::CqamError;

// =============================================================================
// IntRegFile
// =============================================================================

#[test]
fn test_int_reg_file_new_is_zeroed() {
    let regs = IntRegFile::new();
    for i in 0..16u8 {
        assert_eq!(regs.get(i).unwrap(), 0);
    }
}

#[test]
fn test_int_reg_file_set_and_get() {
    let mut regs = IntRegFile::new();
    regs.set(3, 42).unwrap();
    assert_eq!(regs.get(3).unwrap(), 42);
    assert_eq!(regs.get(0).unwrap(), 0); // other registers unchanged
}

#[test]
fn test_int_reg_file_negative_values() {
    let mut regs = IntRegFile::new();
    regs.set(15, -999).unwrap();
    assert_eq!(regs.get(15).unwrap(), -999);
}

#[test]
fn test_int_reg_file_overwrite() {
    let mut regs = IntRegFile::new();
    regs.set(5, 100).unwrap();
    assert_eq!(regs.get(5).unwrap(), 100);
    regs.set(5, 200).unwrap();
    assert_eq!(regs.get(5).unwrap(), 200);
}

#[test]
fn test_int_reg_file_max_values() {
    let mut regs = IntRegFile::new();
    regs.set(0, i64::MAX).unwrap();
    regs.set(1, i64::MIN).unwrap();
    assert_eq!(regs.get(0).unwrap(), i64::MAX);
    assert_eq!(regs.get(1).unwrap(), i64::MIN);
}

#[test]
fn test_int_reg_file_default() {
    let regs = IntRegFile::default();
    for i in 0..16u8 {
        assert_eq!(regs.get(i).unwrap(), 0);
    }
}

#[test]
fn test_int_reg_file_out_of_bounds_get_returns_err() {
    let regs = IntRegFile::new();
    assert!(regs.get(16).is_err());
}

#[test]
fn test_int_reg_file_out_of_bounds_set_returns_err() {
    let mut regs = IntRegFile::new();
    assert!(regs.set(16, 0).is_err());
}

// =============================================================================
// FloatRegFile
// =============================================================================

#[test]
fn test_float_reg_file_new_is_zeroed() {
    let regs = FloatRegFile::new();
    for i in 0..16u8 {
        assert_eq!(regs.get(i).unwrap(), 0.0);
    }
}

#[test]
fn test_float_reg_file_set_and_get() {
    let mut regs = FloatRegFile::new();
    regs.set(7, 3.125).unwrap();
    assert!((regs.get(7).unwrap() - 3.125).abs() < 1e-10);
}

#[test]
fn test_float_reg_file_negative_values() {
    let mut regs = FloatRegFile::new();
    regs.set(0, -2.625).unwrap();
    assert!((regs.get(0).unwrap() - (-2.625)).abs() < 1e-10);
}

#[test]
fn test_float_reg_file_overwrite() {
    let mut regs = FloatRegFile::new();
    regs.set(3, 1.0).unwrap();
    regs.set(3, 2.0).unwrap();
    assert_eq!(regs.get(3).unwrap(), 2.0);
}

#[test]
fn test_float_reg_file_default() {
    let regs = FloatRegFile::default();
    for i in 0..16u8 {
        assert_eq!(regs.get(i).unwrap(), 0.0);
    }
}

#[test]
fn test_float_reg_file_out_of_bounds_returns_err() {
    let regs = FloatRegFile::new();
    assert!(regs.get(16).is_err());
}

// =============================================================================
// ComplexRegFile
// =============================================================================

#[test]
fn test_complex_reg_file_new_is_zeroed() {
    let regs = ComplexRegFile::new();
    for i in 0..16u8 {
        assert_eq!(regs.get(i).unwrap(), (0.0, 0.0));
    }
}

#[test]
fn test_complex_reg_file_set_and_get() {
    let mut regs = ComplexRegFile::new();
    regs.set(0, (1.5, -2.5)).unwrap();
    assert_eq!(regs.get(0).unwrap(), (1.5, -2.5));
}

#[test]
fn test_complex_reg_file_overwrite() {
    let mut regs = ComplexRegFile::new();
    regs.set(5, (1.0, 2.0)).unwrap();
    regs.set(5, (3.0, 4.0)).unwrap();
    assert_eq!(regs.get(5).unwrap(), (3.0, 4.0));
}

#[test]
fn test_complex_reg_file_default() {
    let regs = ComplexRegFile::default();
    for i in 0..16u8 {
        assert_eq!(regs.get(i).unwrap(), (0.0, 0.0));
    }
}

#[test]
fn test_complex_reg_file_out_of_bounds_returns_err() {
    let regs = ComplexRegFile::new();
    assert!(regs.get(16).is_err());
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
        assert!(matches!(regs.get(i).unwrap(), HybridValue::Empty));
    }
}

#[test]
fn test_hybrid_reg_file_set_int() {
    let mut regs = HybridRegFile::new();
    regs.set(0, HybridValue::Int(42)).unwrap();
    assert!(matches!(regs.get(0).unwrap(), HybridValue::Int(42)));
}

#[test]
fn test_hybrid_reg_file_set_dist() {
    let mut regs = HybridRegFile::new();
    let dist = vec![(0u16, 0.5), (1, 0.3), (2, 0.2)];
    regs.set(3, HybridValue::Dist(dist.clone())).unwrap();
    if let HybridValue::Dist(d) = regs.get(3).unwrap() {
        assert_eq!(d.len(), 3);
        assert_eq!(d[0], (0u16, 0.5));
    } else {
        panic!("Expected HybridValue::Dist");
    }
}

#[test]
fn test_hybrid_reg_file_overwrite() {
    let mut regs = HybridRegFile::new();
    regs.set(0, HybridValue::Int(1)).unwrap();
    regs.set(0, HybridValue::Float(2.0)).unwrap();
    assert!(matches!(regs.get(0).unwrap(), HybridValue::Float(f) if (*f - 2.0).abs() < 1e-10));
}

#[test]
fn test_hybrid_reg_file_default() {
    let regs = HybridRegFile::default();
    for i in 0..8u8 {
        assert!(matches!(regs.get(i).unwrap(), HybridValue::Empty));
    }
}

#[test]
fn test_hybrid_reg_file_out_of_bounds_get_returns_err() {
    let regs = HybridRegFile::new();
    assert!(regs.get(8).is_err());
}

#[test]
fn test_hybrid_reg_file_out_of_bounds_set_returns_err() {
    let mut regs = HybridRegFile::new();
    assert!(regs.set(8, HybridValue::Empty).is_err());
}

// =============================================================================
// Clone and equality tests
// =============================================================================

#[test]
fn test_int_reg_file_clone() {
    let mut regs = IntRegFile::new();
    regs.set(5, 42).unwrap();
    let cloned = regs.clone();
    assert_eq!(regs, cloned);
}

#[test]
fn test_float_reg_file_clone() {
    let mut regs = FloatRegFile::new();
    regs.set(5, 3.125).unwrap();
    let cloned = regs.clone();
    assert_eq!(regs, cloned);
}

#[test]
fn test_complex_reg_file_clone() {
    let mut regs = ComplexRegFile::new();
    regs.set(5, (1.0, 2.0)).unwrap();
    let cloned = regs.clone();
    assert_eq!(regs, cloned);
}

#[test]
fn test_hybrid_reg_file_clone() {
    let mut regs = HybridRegFile::new();
    regs.set(0, HybridValue::Int(99)).unwrap();
    let cloned = regs.clone();
    assert_eq!(regs, cloned);
}

#[test]
fn test_register_out_of_bounds_returns_correct_error() {
    let regs = IntRegFile::new();
    let err = regs.get(16).unwrap_err();
    assert!(matches!(err, CqamError::RegisterOutOfBounds { ref file, index } if file == "R" && index == 16));
}
