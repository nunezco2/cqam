//! Text-format parser for the CQAM ISA.
//!
//! Parses flat-prefix assembly syntax with numeric operands into `Instruction`
//! values. All parse functions return `Result<Instruction, CqamError>` and
//! report errors with 1-based line numbers.

pub mod types;
pub mod text;
pub mod sections;
pub mod helpers;

pub use types::{ParseResult, ProgramMetadata, DataSection, SharedSection, PrivateSection, ParsedProgram};
pub use text::{parse_instruction, parse_instruction_at, parse_program};
pub use helpers::{parse_reg, parse_u8, parse_i8};
