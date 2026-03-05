// cqam-vm/src/isr.rs
//
// Phase 2: ISR vector table with two-level interrupt model.
// Replaces the simple match-on-Trap approach.

use std::collections::HashMap;
use crate::context::ExecutionContext;

// =============================================================================
// Trap type hierarchy
// =============================================================================

/// Non-maskable traps that always fire regardless of the interrupt enable flag.
/// These represent unrecoverable or critical conditions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NmiTrap {
    /// Explicit HALT instruction encountered.
    Halt,
    /// Program counter went out of bounds (past end of program without HALT).
    IllegalPC,
}

/// Maskable traps that are gated by the `enable_interrupts` configuration.
/// These represent recoverable conditions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaskableTrap {
    /// Arithmetic fault (division by zero, overflow, etc.).
    Arithmetic,
    /// Quantum fidelity dropped below threshold.
    QuantumError,
    /// Hybrid branch synchronization failure.
    SyncFailure,
}

/// Unified trap type that can be either NMI or maskable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trap {
    /// Non-maskable interrupt.
    Nmi(NmiTrap),
    /// Maskable interrupt.
    Maskable(MaskableTrap),
}

// =============================================================================
// ISR vector table
// =============================================================================

/// Interrupt Service Routine vector table.
///
/// Maps trap types to handler addresses (instruction indices in the program).
/// When a trap fires and has a registered handler, execution jumps to the
/// handler address. The current PC is pushed onto the call stack so that
/// a RET instruction can resume execution after the handler.
///
/// If no handler is registered for a trap, the default behavior applies:
/// - NMI Halt: sets trap_halt flag
/// - NMI IllegalPC: sets trap_halt flag
/// - Maskable Arithmetic: sets trap_arith flag, continues with default value
/// - Maskable QuantumError: sets int_quantum_err flag
/// - Maskable SyncFailure: sets int_sync_fail flag
#[derive(Clone)]
pub struct IsrTable {
    /// Handlers for non-maskable interrupts.
    nmi_handlers: HashMap<NmiTrap, usize>,

    /// Handlers for maskable interrupts.
    maskable_handlers: HashMap<MaskableTrap, usize>,
}

impl IsrTable {
    /// Create a new ISR table with no handlers registered.
    pub fn new() -> Self {
        Self {
            nmi_handlers: HashMap::new(),
            maskable_handlers: HashMap::new(),
        }
    }

    /// Register a handler for a trap type.
    ///
    /// `handler_addr` is the instruction index in the program to jump to
    /// when the trap fires.
    pub fn set_handler(&mut self, trap: &Trap, handler_addr: usize) {
        match trap {
            Trap::Nmi(nmi) => {
                self.nmi_handlers.insert(nmi.clone(), handler_addr);
            }
            Trap::Maskable(maskable) => {
                self.maskable_handlers.insert(maskable.clone(), handler_addr);
            }
        }
    }

    /// Look up the handler address for a trap type.
    ///
    /// Returns `None` if no handler is registered.
    pub fn get_handler(&self, trap: &Trap) -> Option<usize> {
        match trap {
            Trap::Nmi(nmi) => self.nmi_handlers.get(nmi).copied(),
            Trap::Maskable(maskable) => self.maskable_handlers.get(maskable).copied(),
        }
    }
}

impl Default for IsrTable {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Trap handling
// =============================================================================

/// Handle a trap according to the two-level interrupt model.
///
/// - NMI traps always fire. If a handler address is provided, execution jumps
///   to the handler (with the current PC saved on the call stack for RETI).
///   If no handler is provided, the default action (usually halt) applies.
///
/// - Maskable traps check `enable_interrupts`. If interrupts are disabled,
///   the trap is silently ignored. If enabled, the handler is invoked (or
///   default action applies if no handler is provided).
///
/// # Parameters
///
/// - `trap`: The trap to handle.
/// - `ctx`: The execution context.
/// - `handler_addr`: Pre-looked-up handler address (avoids borrow conflict
///   since IsrTable lives inside ExecutionContext).
/// - `enable_interrupts`: Whether maskable interrupts are enabled.
pub fn handle_trap(
    trap: Trap,
    ctx: &mut ExecutionContext,
    handler_addr: Option<usize>,
    enable_interrupts: bool,
) {
    match &trap {
        Trap::Nmi(nmi) => {
            // NMI always fires
            if let Some(addr) = handler_addr {
                ctx.call_stack.push(ctx.pc);
                ctx.pc = addr;
            } else {
                // Default NMI behavior
                match nmi {
                    NmiTrap::Halt => {
                        ctx.psw.trap_halt = true;
                    }
                    NmiTrap::IllegalPC => {
                        log::error!("TRAP: Illegal PC at {}", ctx.pc);
                        ctx.psw.trap_halt = true;
                    }
                }
            }
        }

        Trap::Maskable(maskable) => {
            if !enable_interrupts {
                // Interrupts disabled: silently ignore
                return;
            }

            if let Some(addr) = handler_addr {
                ctx.call_stack.push(ctx.pc);
                ctx.pc = addr;
            } else {
                // Default maskable behavior
                match maskable {
                    MaskableTrap::Arithmetic => {
                        ctx.psw.trap_halt = true;
                    }
                    MaskableTrap::QuantumError => {
                        ctx.psw.trap_halt = true;
                    }
                    MaskableTrap::SyncFailure => {
                        ctx.psw.trap_halt = true;
                    }
                }
            }
        }
    }
}
