//! Type definitions used across the parser module.
//!
//! Contains `ParseResult`, `ProgramMetadata`, `DataSection`, `SharedSection`,
//! `PrivateSection`, and `ParsedProgram`.

use std::collections::HashMap;

use crate::error::CqamError;
use crate::instruction::Instruction;

/// Convenience type alias for parser results.
pub type ParseResult = Result<Instruction, CqamError>;

/// Metadata extracted from `#!` pragma directives in a CQAM source file.
///
/// Pragmas are processed during parsing but do not generate instructions.
/// They provide configuration hints that the loader/runner can apply before
/// execution.
#[derive(Debug, Default, Clone)]
pub struct ProgramMetadata {
    /// Number of qubits requested by the program via `#! qubits N`.
    ///
    /// `None` means no pragma was found; use the default or CLI value.
    pub qubits: Option<u8>,
    /// Number of threads requested by the program via `#! threads N`.
    pub threads: Option<u16>,
}

/// Pre-loaded data from a `.data` section.
///
/// Each cell maps to one CMEM slot (one i64 per cell). Labels record the
/// starting address and length so that code can reference them with `@label`
/// and `@label.len`.
#[derive(Debug, Clone, Default)]
pub struct DataSection {
    /// Flat vector of i64 values to be loaded into CMEM[0..cells.len()].
    pub cells: Vec<i64>,

    /// label → (base_address, length_in_cells).
    pub labels: HashMap<String, (u16, u16)>,
}

/// Pre-loaded data from a `.shared` section.
#[derive(Debug, Clone, Default)]
pub struct SharedSection {
    /// Base address in CMEM (from `.org`).
    pub base: u16,
    /// Flat vector of i64 values for shared memory initialization.
    pub cells: Vec<i64>,
    /// label -> (base_address, length_in_cells).
    pub labels: HashMap<String, (u16, u16)>,
}

/// Configuration from a `.private` section.
#[derive(Debug, Clone, Default)]
pub struct PrivateSection {
    /// Per-thread private memory size in cells. 0 means no private section.
    pub size: u16,
}

/// Result of parsing a complete CQAM program.
///
/// Contains both the instruction stream and any pragma metadata.
#[derive(Debug)]
pub struct ParsedProgram {
    /// The instruction stream (labels, ops, no Nops).
    pub instructions: Vec<Instruction>,

    /// Metadata from `#!` pragma directives.
    pub metadata: ProgramMetadata,

    /// Pre-loaded data from the `.data` section (empty if none).
    pub data_section: DataSection,
    /// Pre-loaded data from `.shared` section (empty if none).
    pub shared_section: SharedSection,
    /// Per-thread private memory configuration (zero size if none).
    pub private_section: PrivateSection,
}
