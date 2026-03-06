//! Program loader for the CQAM runner.
//!
//! Provides [`load_program`], which reads a `.cqam` text-format source file
//! from disk and delegates parsing to [`cqam_core::parser::parse_program`].
//! The resulting `Vec<Instruction>` can be passed directly to
//! [`cqam_run::runner::run_program`] or [`cqam_run::runner::run_program_with_config`].

use std::fs;
use cqam_core::error::CqamError;
use cqam_core::parser::ParsedProgram;

/// Load a CQAM program from a text file.
///
/// Reads the file at `path`, then delegates parsing to
/// `cqam_core::parser::parse_program()`. Returns both the parsed
/// instructions and any pragma metadata.
pub fn load_program(path: &str) -> Result<ParsedProgram, CqamError> {
    let content = fs::read_to_string(path)?;
    cqam_core::parser::parse_program(&content)
}
