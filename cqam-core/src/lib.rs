//! Core ISA definitions for the CQAM virtual machine.
//!
//! `cqam-core` is the foundational crate depended upon by every other crate in
//! the workspace. It contains no simulation logic and no I/O; it is a pure
//! in-memory definition layer.
//!
//! # Key types
//!
//! | Module | Key type | Purpose |
//! |--------|----------|---------|
//! | [`instruction`] | [`Instruction`](instruction::Instruction) | Complete ISA enum |
//! | [`opcode`] | [`encode`](opcode::encode) / [`decode`](opcode::decode) | 32-bit binary encoding |
//! | [`parser`] | [`parse_program`](parser::parse_program) | Text-format parser |
//! | [`register`] | `IntRegFile`, `FloatRegFile`, `ComplexRegFile`, `HybridRegFile` | Register files |
//! | [`memory`] | [`CMem`](memory::CMem), [`QMem`](memory::QMem) | Classical and quantum memory |
//! | [`quantum_state`] | [`QuantumState`](quantum_state::QuantumState) | Abstraction trait for QMEM |
//! | [`error`] | [`CqamError`](error::CqamError) | Unified error type |
//!
//! # Usage patterns
//!
//! Parse a program from text:
//! ```
//! use cqam_core::parser::parse_program;
//! let parsed = parse_program("ILDI R0, 42\nHALT\n").unwrap();
//! assert_eq!(parsed.instructions.len(), 2);
//! ```
//!
//! Encode a single instruction to a 32-bit word:
//! ```
//! use std::collections::HashMap;
//! use cqam_core::instruction::Instruction;
//! use cqam_core::opcode::encode;
//! let word = encode(&Instruction::Halt, &HashMap::new()).unwrap();
//! assert_eq!(word, 0x2B000000);
//! ```

pub mod error;
pub mod instruction;
pub mod quantum_state;
pub mod register;
pub mod memory;
pub mod parser;
pub mod opcode;
