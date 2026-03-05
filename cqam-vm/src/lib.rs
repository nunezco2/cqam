//! CQAM virtual machine: execution context, instruction dispatch, and I/O.
//!
//! Provides the `ExecutionContext` that holds all machine state, the
//! `execute_instruction` dispatcher, quantum operation handlers (`qop`),
//! hybrid fork/merge handlers (`hybrid`, `fork`), the program status word
//! (`psw`), interrupt service routine table (`isr`), and resource tracking.

pub mod context;
pub mod executor;
pub mod fork;
pub mod resource;
pub mod qop;
pub mod psw;
pub mod simconfig;
pub mod isr;
pub mod hybrid;
