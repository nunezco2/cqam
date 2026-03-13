//! Register files and hybrid value types for the CQAM virtual machine.
//!
//! Provides fixed-size typed register files for each of the five register
//! namespaces: integer (R), float (F), complex (Z), quantum (Q, in cqam-vm),
//! and hybrid (H). Also defines `HybridValue`, the tagged union stored in the
//! H-file.

use crate::constants::{INT_REG_COUNT, FLOAT_REG_COUNT, COMPLEX_REG_COUNT, H_REG_COUNT};
use crate::error::CqamError;

// =============================================================================
// Integer register file: R0-R15
// =============================================================================

/// 16 general-purpose 64-bit signed integer registers (R0-R15).
///
/// Used by all I-prefix instructions (IADD, ISUB, ILDI, etc.) and as
/// the comparison result target for FEq/FLt/FGt.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IntRegFile {
    pub regs: [i64; INT_REG_COUNT],
}

impl IntRegFile {
    /// Create a new zero-initialized integer register file.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read integer register R[idx].
    ///
    /// Returns `Err(CqamError::RegisterOutOfBounds)` if `idx >= 16`.
    pub fn get(&self, idx: u8) -> Result<i64, CqamError> {
        if (idx as usize) < self.regs.len() {
            Ok(self.regs[idx as usize])
        } else {
            Err(CqamError::RegisterOutOfBounds {
                file: "R".to_string(),
                index: idx,
            })
        }
    }

    /// Return the backing array as an immutable slice.
    pub fn as_slice(&self) -> &[i64] {
        &self.regs
    }

    /// Return the backing array as a mutable slice.
    pub fn as_slice_mut(&mut self) -> &mut [i64] {
        &mut self.regs
    }

    /// Write integer register R[idx].
    ///
    /// Returns `Err(CqamError::RegisterOutOfBounds)` if `idx >= 16`.
    pub fn set(&mut self, idx: u8, val: i64) -> Result<(), CqamError> {
        if (idx as usize) < self.regs.len() {
            self.regs[idx as usize] = val;
            Ok(())
        } else {
            Err(CqamError::RegisterOutOfBounds {
                file: "R".to_string(),
                index: idx,
            })
        }
    }
}

// =============================================================================
// Float register file: F0-F15
// =============================================================================

/// 16 general-purpose 64-bit floating-point registers (F0-F15).
///
/// Used by all F-prefix instructions (FADD, FSUB, FLDI, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct FloatRegFile {
    pub regs: [f64; FLOAT_REG_COUNT],
}

impl Default for FloatRegFile {
    fn default() -> Self {
        Self { regs: [0.0; FLOAT_REG_COUNT] }
    }
}

impl FloatRegFile {
    /// Create a new zero-initialized float register file.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read float register F[idx].
    ///
    /// Returns `Err(CqamError::RegisterOutOfBounds)` if `idx >= 16`.
    pub fn get(&self, idx: u8) -> Result<f64, CqamError> {
        if (idx as usize) < self.regs.len() {
            Ok(self.regs[idx as usize])
        } else {
            Err(CqamError::RegisterOutOfBounds {
                file: "F".to_string(),
                index: idx,
            })
        }
    }

    /// Return the backing array as an immutable slice.
    pub fn as_slice(&self) -> &[f64] {
        &self.regs
    }

    /// Return the backing array as a mutable slice.
    pub fn as_slice_mut(&mut self) -> &mut [f64] {
        &mut self.regs
    }

    /// Write float register F[idx].
    ///
    /// Returns `Err(CqamError::RegisterOutOfBounds)` if `idx >= 16`.
    pub fn set(&mut self, idx: u8, val: f64) -> Result<(), CqamError> {
        if (idx as usize) < self.regs.len() {
            self.regs[idx as usize] = val;
            Ok(())
        } else {
            Err(CqamError::RegisterOutOfBounds {
                file: "F".to_string(),
                index: idx,
            })
        }
    }
}

// =============================================================================
// Complex register file: Z0-Z15
// =============================================================================

/// 16 complex number registers (Z0-Z15), each a pair of f64 (real, imaginary).
///
/// Used by all Z-prefix instructions (ZADD, ZSUB, ZLDI, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexRegFile {
    pub regs: [(f64, f64); COMPLEX_REG_COUNT],
}

impl Default for ComplexRegFile {
    fn default() -> Self {
        Self { regs: [(0.0, 0.0); COMPLEX_REG_COUNT] }
    }
}

impl ComplexRegFile {
    /// Create a new zero-initialized complex register file.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read complex register Z[idx].
    ///
    /// Returns `Err(CqamError::RegisterOutOfBounds)` if `idx >= 16`.
    pub fn get(&self, idx: u8) -> Result<(f64, f64), CqamError> {
        if (idx as usize) < self.regs.len() {
            Ok(self.regs[idx as usize])
        } else {
            Err(CqamError::RegisterOutOfBounds {
                file: "Z".to_string(),
                index: idx,
            })
        }
    }

    /// Return the backing array as an immutable slice.
    pub fn as_slice(&self) -> &[(f64, f64)] {
        &self.regs
    }

    /// Return the backing array as a mutable slice.
    pub fn as_slice_mut(&mut self) -> &mut [(f64, f64)] {
        &mut self.regs
    }

    /// Write complex register Z[idx].
    ///
    /// Returns `Err(CqamError::RegisterOutOfBounds)` if `idx >= 16`.
    pub fn set(&mut self, idx: u8, val: (f64, f64)) -> Result<(), CqamError> {
        if (idx as usize) < self.regs.len() {
            self.regs[idx as usize] = val;
            Ok(())
        } else {
            Err(CqamError::RegisterOutOfBounds {
                file: "Z".to_string(),
                index: idx,
            })
        }
    }
}

// =============================================================================
// Hybrid value type
// =============================================================================

/// A hybrid value that can hold classical data or a probability distribution
/// resulting from quantum measurement.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum HybridValue {
    /// Uninitialized / empty slot.
    #[default]
    Empty,

    /// Integer value (from reduction or direct write).
    Int(i64),

    /// Float value (from reduction or direct write).
    Float(f64),

    /// Complex value (from reduction).
    Complex(f64, f64),

    /// Probability distribution over basis states.
    /// Each entry is (basis_state: u32, probability: f64).
    /// This is the primary output of QObserve.
    Dist(Vec<(u32, f64)>),
}


// =============================================================================
// Hybrid register file: H0-H7
// =============================================================================

/// 8 hybrid registers (H0-H7), each holding a HybridValue.
///
/// Used by QObserve (writes measurement results) and HReduce (reads for
/// classical reduction).
#[derive(Debug, Clone, PartialEq)]
pub struct HybridRegFile {
    pub regs: [HybridValue; H_REG_COUNT],
}

impl Default for HybridRegFile {
    fn default() -> Self {
        Self {
            regs: std::array::from_fn(|_| HybridValue::Empty),
        }
    }
}

impl HybridRegFile {
    /// Create a new hybrid register file with all slots empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read hybrid register H[idx].
    ///
    /// Returns `Err(CqamError::RegisterOutOfBounds)` if `idx >= 8`.
    pub fn get(&self, idx: u8) -> Result<&HybridValue, CqamError> {
        if (idx as usize) < self.regs.len() {
            Ok(&self.regs[idx as usize])
        } else {
            Err(CqamError::RegisterOutOfBounds {
                file: "H".to_string(),
                index: idx,
            })
        }
    }

    /// Return the backing array as an immutable slice.
    pub fn as_slice(&self) -> &[HybridValue] {
        &self.regs
    }

    /// Return the backing array as a mutable slice.
    pub fn as_slice_mut(&mut self) -> &mut [HybridValue] {
        &mut self.regs
    }

    /// Write hybrid register H[idx].
    ///
    /// Returns `Err(CqamError::RegisterOutOfBounds)` if `idx >= 8`.
    pub fn set(&mut self, idx: u8, val: HybridValue) -> Result<(), CqamError> {
        if (idx as usize) < self.regs.len() {
            self.regs[idx as usize] = val;
            Ok(())
        } else {
            Err(CqamError::RegisterOutOfBounds {
                file: "H".to_string(),
                index: idx,
            })
        }
    }
}
