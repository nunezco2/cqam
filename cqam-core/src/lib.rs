//! Core ISA definitions for the CQAM virtual machine.
//!
//! Provides the instruction set architecture (ISA), register files, memory
//! abstractions, parser, binary opcode encoding, and the unified error type
//! shared across all CQAM crates.

pub mod error;
pub mod instruction;
pub mod quantum_state;
pub mod register;
pub mod memory;
pub mod parser;
pub mod opcode;
