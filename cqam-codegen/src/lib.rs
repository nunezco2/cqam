//! CQAM code generation: translates CQAM IR to OpenQASM 3.0.
//!
//! `cqam-codegen` implements a three-phase emission pipeline:
//!
//! 1. **Scan** — walk all instructions, collect used register indices and
//!    kernel IDs via [`qasm::scan_registers`].
//! 2. **Declare** — emit one QASM variable declaration per used register
//!    via [`qasm::emit_declarations`] (standalone mode only).
//! 3. **Emit** — translate each instruction to QASM body lines via the
//!    [`qasm::QasmFormat`] trait.
//!
//! The top-level entry point is [`qasm::emit_qasm_program`], which orchestrates
//! all three phases and returns a complete QASM string.
//!
//! # Key types
//!
//! | Module | Type / function | Purpose |
//! |--------|-----------------|---------|
//! | [`qasm`] | [`EmitMode`](qasm::EmitMode) | Standalone vs. fragment output |
//! | [`qasm`] | [`EmitConfig`](qasm::EmitConfig) | Mode + template expansion flag |
//! | [`qasm`] | [`UsedRegisters`](qasm::UsedRegisters) | Per-program register census |
//! | [`qasm`] | [`emit_qasm_program`](qasm::emit_qasm_program) | Full pipeline entry point |
//! | [`qasm`] | [`QasmFormat`](qasm::QasmFormat) | Per-instruction QASM trait |
//!
//! # Usage
//!
//! ```
//! use cqam_core::parser::parse_program;
//! use cqam_codegen::qasm::{emit_qasm_program, EmitConfig};
//!
//! let program = parse_program("ILDI R0, 5\nHALT\n").unwrap().instructions;
//! let qasm = emit_qasm_program(&program, &EmitConfig::default());
//! assert!(qasm.contains("OPENQASM 3.0"));
//! assert!(qasm.contains("R0 = 5;"));
//! ```

pub mod qasm;
