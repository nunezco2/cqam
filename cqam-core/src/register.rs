// cqam-core/src/register.rs
//
// Phase 2: Separate register files with fixed-size arrays.
// Replaces the old CValue/RegisterBank string-keyed design.

// =============================================================================
// Integer register file: R0-R15
// =============================================================================

/// 16 general-purpose 64-bit signed integer registers (R0-R15).
///
/// Used by all I-prefix instructions (IADD, ISUB, ILDI, etc.) and as
/// the comparison result target for FEq/FLt/FGt.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IntRegFile {
    pub regs: [i64; 16],
}

impl IntRegFile {
    /// Create a new zero-initialized integer register file.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read integer register R[idx].
    ///
    /// # Panics
    /// Panics if `idx >= 16`. Phase 4 converts this to `Result`.
    pub fn get(&self, idx: u8) -> i64 {
        self.regs[idx as usize]
    }

    /// Write integer register R[idx].
    ///
    /// # Panics
    /// Panics if `idx >= 16`. Phase 4 converts this to `Result`.
    pub fn set(&mut self, idx: u8, val: i64) {
        self.regs[idx as usize] = val;
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
    pub regs: [f64; 16],
}

impl Default for FloatRegFile {
    fn default() -> Self {
        Self { regs: [0.0; 16] }
    }
}

impl FloatRegFile {
    /// Create a new zero-initialized float register file.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read float register F[idx].
    ///
    /// # Panics
    /// Panics if `idx >= 16`.
    pub fn get(&self, idx: u8) -> f64 {
        self.regs[idx as usize]
    }

    /// Write float register F[idx].
    ///
    /// # Panics
    /// Panics if `idx >= 16`.
    pub fn set(&mut self, idx: u8, val: f64) {
        self.regs[idx as usize] = val;
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
    pub regs: [(f64, f64); 16],
}

impl Default for ComplexRegFile {
    fn default() -> Self {
        Self { regs: [(0.0, 0.0); 16] }
    }
}

impl ComplexRegFile {
    /// Create a new zero-initialized complex register file.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read complex register Z[idx].
    ///
    /// # Panics
    /// Panics if `idx >= 16`.
    pub fn get(&self, idx: u8) -> (f64, f64) {
        self.regs[idx as usize]
    }

    /// Write complex register Z[idx].
    ///
    /// # Panics
    /// Panics if `idx >= 16`.
    pub fn set(&mut self, idx: u8, val: (f64, f64)) {
        self.regs[idx as usize] = val;
    }
}

// =============================================================================
// Hybrid value type
// =============================================================================

/// A hybrid value that can hold classical data or a probability distribution
/// resulting from quantum measurement.
///
/// This replaces the old `HybridValue` which only supported `Prob(f64)` and
/// `Dist(Vec<(CValue, f64)>)`.
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
    /// Each entry is (basis_state: u16, probability: f64).
    /// This is the primary output of QObserve.
    Dist(Vec<(u16, f64)>),
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
    pub regs: [HybridValue; 8],
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
    /// # Panics
    /// Panics if `idx >= 8`.
    pub fn get(&self, idx: u8) -> &HybridValue {
        &self.regs[idx as usize]
    }

    /// Write hybrid register H[idx].
    ///
    /// # Panics
    /// Panics if `idx >= 8`.
    pub fn set(&mut self, idx: u8, val: HybridValue) {
        self.regs[idx as usize] = val;
    }
}
