use std::fs;
use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;

/// Load a CQAM program from a text file.
///
/// Reads the file at `path`, then delegates parsing to
/// `cqam_core::parser::parse_program()`. Returns `Err(CqamError)` on
/// I/O or parse errors.
pub fn load_program(path: &str) -> Result<Vec<Instruction>, CqamError> {
    let content = fs::read_to_string(path)?;
    cqam_core::parser::parse_program(&content)
}
