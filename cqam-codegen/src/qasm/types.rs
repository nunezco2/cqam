//! Types and traits for QASM code generation.
//!
//! Defines the configuration types (`EmitMode`, `EmitConfig`), the register
//! census struct (`UsedRegisters`), and the `QasmFormat` trait for converting
//! CQAM instructions into OpenQASM 3.0 strings.

use std::collections::BTreeSet;
use cqam_core::instruction::KernelId;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Controls how QASM output is structured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitMode {
    /// Full program: OPENQASM header, includes, declarations, body, footer.
    Standalone,
    /// Body only: no header, no includes, no declarations, no gate stubs.
    /// Suitable for embedding in a larger QASM program.
    Fragment,
}

/// Configuration for QASM emission.
#[derive(Debug, Clone)]
pub struct EmitConfig {
    /// Standalone or fragment mode.
    pub mode: EmitMode,
    /// Whether to expand kernel templates from disk.
    pub expand_templates: bool,
    /// Base directory for template file lookup.
    /// Default: "kernels/qasm_templates"
    pub template_dir: String,
}

impl Default for EmitConfig {
    fn default() -> Self {
        EmitConfig {
            mode: EmitMode::Standalone,
            expand_templates: false,
            template_dir: "kernels/qasm_templates".to_string(),
        }
    }
}

impl EmitConfig {
    /// Create a standalone config with template expansion enabled.
    pub fn standalone() -> Self {
        EmitConfig {
            mode: EmitMode::Standalone,
            expand_templates: true,
            ..Default::default()
        }
    }

    /// Create a fragment config with template expansion disabled.
    pub fn fragment() -> Self {
        EmitConfig {
            mode: EmitMode::Fragment,
            expand_templates: false,
            ..Default::default()
        }
    }
}

/// Tracks which registers are used across a program.
///
/// Populated by `scan_registers()` during the scan phase. Each field is a
/// sorted set of register indices that appear as operands (read or write)
/// in at least one instruction.
#[derive(Debug, Clone, Default)]
pub struct UsedRegisters {
    /// Integer registers R0-R15 that appear in instructions.
    pub int_regs: BTreeSet<u8>,
    /// Float registers F0-F15 that appear in instructions.
    pub float_regs: BTreeSet<u8>,
    /// Complex registers Z0-Z15 that appear in instructions.
    /// Each entry generates two float declarations (re + im).
    pub complex_regs: BTreeSet<u8>,
    /// Quantum registers Q0-Q7 that appear in instructions.
    pub quantum_regs: BTreeSet<u8>,
    /// Hybrid registers H0-H7 that appear in instructions.
    pub hybrid_regs: BTreeSet<u8>,
    /// Whether any instruction accesses CMEM (ILdm, IStr, FLdm, FStr, ZLdm, ZStr).
    pub uses_cmem: bool,
    /// Whether any instruction accesses QMEM (QLoad, QStore).
    pub uses_qmem: bool,
    /// Set of kernel IDs referenced by QKernel instructions.
    pub kernel_ids: BTreeSet<KernelId>,
    /// Label names in program order (from Label instructions).
    pub labels: Vec<String>,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Trait for converting CQAM instructions into OpenQASM 3.0 strings.
pub trait QasmFormat {
    /// Convert a single instruction to its QASM body representation.
    ///
    /// Returns a Vec of QASM lines (possibly empty for Nop). Each line is
    /// a complete QASM statement without trailing newline.
    ///
    /// Body lines do NOT include type declarations -- those are emitted
    /// separately by `emit_declarations()`.
    fn to_qasm(&self, config: &EmitConfig) -> Vec<String>;
}
