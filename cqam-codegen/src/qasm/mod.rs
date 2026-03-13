//! QASM code generator: emits valid OpenQASM 3.0 from a CQAM instruction sequence.
//!
//! The emitter follows a three-stage pipeline:
//!   1. Scan    -- walk all instructions, collect used register indices
//!   2. Declare -- emit one declaration per used register (standalone only)
//!   3. Emit    -- translate each instruction to QASM body lines

mod types;
mod emit;
mod scan;
mod declare;
mod helpers;

pub use types::{EmitMode, EmitConfig, UsedRegisters, QasmFormat};
pub use emit::emit_qasm_program;
pub use scan::scan_registers;
pub use declare::{emit_declarations, emit_kernel_stubs};
pub use helpers::{load_template, load_gate_template};
