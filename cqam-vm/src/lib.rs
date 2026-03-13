//! CQAM virtual machine: execution engine, context, PSW, ISR, and resources.
//!
//! `cqam-vm` is the execution layer. It takes a `Vec<Instruction>` produced by
//! `cqam-core`'s parser, wraps it in an [`ExecutionContext`](context::ExecutionContext),
//! and dispatches one instruction per cycle via
//! [`execute_instruction`](executor::execute_instruction).
//!
//! # Key types
//!
//! | Module | Key type | Purpose |
//! |--------|----------|---------|
//! | [`context`] | [`ExecutionContext`](context::ExecutionContext) | Complete VM state |
//! | [`executor`] | [`execute_instruction`](executor::execute_instruction) | Instruction dispatch |
//! | [`psw`] | [`ProgramStateWord`](psw::ProgramStateWord) | Condition and trap flags |
//! | [`isr`] | [`IsrTable`](isr::IsrTable) | Interrupt service routine vector table |
//! | [`resource`] | [`ResourceTracker`](resource::ResourceTracker) | Cumulative resource usage |
//! | [`qop`] | `execute_qop` | Quantum instruction handler |
//! | [`hybrid`] | `execute_hybrid` | Hybrid fork/merge handler |
//! | [`fork`] | [`ForkManager`](fork::ForkManager) | Thread pool for HFORK/HMERGE |
//! | `cqam_core::config` | [`VmConfig`](cqam_core::config::VmConfig) | Unified VM configuration |
//!
//! # Execution model
//!
//! The VM is a register machine with five register files (R, F, Z, Q, H),
//! classical memory (CMEM: 64K x i64), quantum memory (QMEM: 256 x `QRegHandle`),
//! a call stack, a PSW, and an ISR table. The quantum register file (Q0-Q7)
//! holds `Option<QRegHandle>` handles into the [`QuantumBackend`](cqam_core::quantum_backend::QuantumBackend),
//! operated on by QPREP, QKERNEL, and QOBSERVE.
//! HFORK spawns parallel execution threads; HMERGE joins them.
//!
//! # PC ownership contract
//!
//! [`execute_instruction`](executor::execute_instruction) is the sole
//! authority on PC advancement. Callers must not call
//! `ctx.advance_pc()` between iterations.
//!
//! # Usage
//!
//! Typically used via `cqam-run`. For direct use:
//!
//! ```ignore
//! use cqam_core::parser::parse_program;
//! use cqam_vm::context::ExecutionContext;
//! use cqam_vm::executor::execute_instruction;
//! use cqam_vm::fork::ForkManager;
//!
//! let program = parse_program("ILDI R0, 1\nHALT\n").unwrap().instructions;
//! let mut ctx = ExecutionContext::new(program);
//! let mut fork_mgr = ForkManager::new();
//!
//! while ctx.pc < ctx.program.len() {
//!     let instr = ctx.program[ctx.pc].clone();
//!     execute_instruction(&mut ctx, &instr, &mut fork_mgr).unwrap();
//!     if ctx.psw.trap_halt { break; }
//! }
//! ```

pub mod context;
pub mod executor;
pub mod fork;
pub mod resource;
pub mod qop;
pub mod psw;
pub mod isr;
pub mod hybrid;
pub mod thread_pool;
