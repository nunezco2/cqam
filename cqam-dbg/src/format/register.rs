//! Register value formatting for the STATE pane.
//!
//! Provides formatting functions for integer, float, complex, hybrid, and
//! quantum register values, including change detection styling.
#![allow(dead_code)]

use cqam_core::register::HybridValue;
use cqam_core::quantum_backend::QRegHandle;

/// Format an integer register value.
///
/// Returns the formatted string (e.g., "42", "-7", "0").
pub fn format_int(value: i64) -> String {
    format!("{}", value)
}

/// Format a float register value.
///
/// Uses up to 4 decimal places, trimming trailing zeros.
pub fn format_float(value: f64) -> String {
    if value == 0.0 {
        "0.0".to_string()
    } else if value.fract() == 0.0 {
        format!("{:.1}", value)
    } else {
        // Use a precision that shows meaningful digits without excessive zeros.
        let s = format!("{:.4}", value);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        // Ensure at least one decimal place for floats.
        if !s.contains('.') {
            format!("{}.0", s)
        } else {
            s.to_string()
        }
    }
}

/// Format a complex register value.
///
/// Returns a string like "(0.707,0.707)" or "(0,0)".
pub fn format_complex(re: f64, im: f64) -> String {
    format!("({},{})", format_float(re), format_float(im))
}

/// Format a hybrid register value.
///
/// Returns a human-readable representation:
/// - `Empty` -> "---"
/// - `Int(n)` -> "n"
/// - `Float(f)` -> "f"
/// - `Complex(re, im)` -> "(re,im)"
/// - `Dist(entries)` -> "Dist(N)" where N is the number of entries
pub fn format_hybrid(value: &HybridValue) -> String {
    match value {
        HybridValue::Empty => "---".to_string(),
        HybridValue::Int(n) => format!("{}", n),
        HybridValue::Float(f) => format_float(*f),
        HybridValue::Complex(re, im) => format_complex(*re, *im),
        HybridValue::Dist(entries) => format!("Dist({})", entries.len()),
        HybridValue::Hist(hist) => format!("Hist({} shots, {} outcomes)", hist.total_shots, hist.num_outcomes()),
    }
}

/// Format a quantum register summary.
///
/// Returns a compact description:
/// - `None` -> "---"
/// - `Some(handle)` -> "Handle(N)"
pub fn format_qreg_summary(qreg: &Option<QRegHandle>) -> String {
    match qreg {
        None => "---".to_string(),
        Some(handle) => format!("Handle({})", handle.0),
    }
}

/// Check if an integer value is "zero" (the default/uninteresting state).
pub fn is_int_zero(value: i64) -> bool {
    value == 0
}

/// Check if a float value is "zero".
pub fn is_float_zero(value: f64) -> bool {
    value == 0.0
}

/// Check if a complex value is "zero".
pub fn is_complex_zero(re: f64, im: f64) -> bool {
    re == 0.0 && im == 0.0
}

/// Check if a hybrid value is "empty" (uninitialized).
pub fn is_hybrid_empty(value: &HybridValue) -> bool {
    matches!(value, HybridValue::Empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_int() {
        assert_eq!(format_int(42), "42");
        assert_eq!(format_int(-7), "-7");
        assert_eq!(format_int(0), "0");
    }

    #[test]
    fn test_format_float() {
        assert_eq!(format_float(0.0), "0.0");
        assert_eq!(format_float(3.14), "3.14");
        assert_eq!(format_float(1.0), "1.0");
    }

    #[test]
    fn test_format_complex() {
        let s = format_complex(0.707, 0.707);
        assert!(s.starts_with('('));
        assert!(s.ends_with(')'));
    }

    #[test]
    fn test_format_hybrid() {
        assert_eq!(format_hybrid(&HybridValue::Empty), "---");
        assert_eq!(format_hybrid(&HybridValue::Int(42)), "42");
    }

    #[test]
    fn test_format_qreg_summary_none() {
        assert_eq!(format_qreg_summary(&None), "---");
    }

}
