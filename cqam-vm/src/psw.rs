//! Program Status Word (PSW) for the CQAM virtual machine.
//!
//! Holds all condition flags (arithmetic, quantum state, hybrid control),
//! trap flags, and provides methods for updating flags from instruction results
//! and for querying pending traps in priority order.

/// Program Status Word: holds all condition, quantum, hybrid, and trap flags.
///
/// Condition flags are updated by arithmetic, comparison, and quantum
/// instructions. Trap flags are set by runtime faults and checked by the ISR
/// dispatch loop after each instruction. The flag IDs used by `HCEXEC` and
/// `get_flag` are defined in [`cqam_core::instruction::flag_id`].
#[derive(Debug, Default, Clone)]
pub struct ProgramStateWord {
    // --- Classical condition flags ---

    /// Zero flag: set when the last arithmetic result was zero.
    pub zf: bool,
    /// Negative flag: set when the last arithmetic result was negative.
    pub nf: bool,
    /// Overflow flag: set on signed integer overflow (not yet fully implemented).
    pub of: bool,
    /// Predicate flag: set by comparison instructions (IEq, ILt, FEq, etc.).
    pub pf: bool,

    // --- Quantum state flags ---

    /// Quantum active: at least one Q register is currently occupied.
    pub qf: bool,
    /// Superposition present: the last QKERNEL produced a non-trivial superposition.
    pub sf: bool,
    /// Entanglement present: the last QKERNEL produced measurable entanglement.
    pub ef: bool,
    /// Decohered: the last QOBSERVE collapsed a quantum register.
    pub df: bool,
    /// Collapsed: a measurement outcome has been stored in an H register.
    pub cf: bool,

    // --- Hybrid execution context flags ---

    /// Hybrid mode: the VM is inside an HFORK/HMERGE block.
    pub hf: bool,
    /// Forked: at least one parallel thread has been spawned (HFORK executed).
    pub forked: bool,
    /// Merged: a HMERGE has completed since the last HFORK.
    pub merged: bool,

    // --- Trap and interrupt flags ---

    /// Arithmetic trap: set by IDIV or IMOD with a zero divisor.
    /// Dispatched as a maskable interrupt in the ISR loop.
    pub trap_arith: bool,
    /// Halt trap: set by HALT or by the max-cycles limit.
    /// Non-maskable; causes the runner loop to terminate.
    pub trap_halt: bool,
    /// Quantum error interrupt: set when fidelity drops below threshold.
    pub int_quantum_err: bool,
    /// Synchronization failure interrupt: set when HMERGE cannot join threads.
    pub int_sync_fail: bool,
}

impl ProgramStateWord {
    /// Create a new zero-initialised PSW with all flags cleared.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all flags to their default (false/cleared) state.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Update arithmetic flags from an integer result value.
    pub fn update_from_arithmetic(&mut self, value: i64) {
        self.zf = value == 0;
        self.nf = value < 0;
        self.of = false; // TODO: real overflow detection
    }

    /// Update the predicate flag from a boolean result.
    pub fn update_from_predicate(&mut self, result: bool) {
        self.pf = result;
    }

    /// Update quantum state flags from fidelity metrics.
    pub fn update_from_qmeta(
        &mut self,
        superposition: f64,
        entanglement: f64,
        threshold: (f64, f64),
    ) {
        self.qf = true;
        self.sf = superposition > 0.0;
        self.ef = entanglement > 0.0;
        self.df = false;
        self.cf = false;

        if superposition < threshold.0 || entanglement < threshold.1 {
            self.int_quantum_err = true;
        }
    }

    /// Mark a quantum register as measured/collapsed.
    pub fn mark_measured(&mut self) {
        self.df = true;
        self.cf = true;
    }

    /// Clear all maskable trap/interrupt flags.
    ///
    /// Called by RETI to acknowledge that the interrupt handler has
    /// completed servicing the trap. Clears:
    /// - trap_arith
    /// - int_quantum_err
    /// - int_sync_fail
    ///
    /// Does NOT clear trap_halt (that is an NMI-level flag).
    pub fn clear_maskable_traps(&mut self) {
        self.trap_arith = false;
        self.int_quantum_err = false;
        self.int_sync_fail = false;
    }

    /// Read a PSW flag by numeric ID.
    ///
    /// Flag IDs (matching `flag_id` constants in `instruction.rs`):
    ///   0 = ZF (zero)
    ///   1 = NF (negative)
    ///   2 = OF (overflow)
    ///   3 = PF (predicate)
    ///   4 = QF (quantum active)
    ///   5 = SF (superposition)
    ///   6 = EF (entanglement)
    ///   7 = HF (hybrid mode)
    ///
    /// Returns `false` for any unrecognized flag ID.
    pub fn get_flag(&self, flag_id: u8) -> bool {
        match flag_id {
            0 => self.zf,
            1 => self.nf,
            2 => self.of,
            3 => self.pf,
            4 => self.qf,
            5 => self.sf,
            6 => self.ef,
            7 => self.hf,
            _ => false,
        }
    }

    /// Check for pending interrupts.
    ///
    /// Returns the highest-priority pending trap, if any.
    /// Priority order: trap_halt > trap_arith > int_quantum_err > int_sync_fail.
    pub fn check_pending_traps(&self) -> Option<PendingTrap> {
        if self.trap_halt {
            Some(PendingTrap::Halt)
        } else if self.trap_arith {
            Some(PendingTrap::Arithmetic)
        } else if self.int_quantum_err {
            Some(PendingTrap::QuantumError)
        } else if self.int_sync_fail {
            Some(PendingTrap::SyncFailure)
        } else {
            None
        }
    }
}

/// Pending trap enumeration for [`ProgramStateWord::check_pending_traps`].
///
/// This is a local convenience type used to communicate trap priority to the
/// runner loop. The authoritative two-level NMI/maskable hierarchy is defined
/// in [`crate::isr`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingTrap {
    /// Halt trap (non-maskable): HALT instruction or max-cycle limit reached.
    Halt,
    /// Arithmetic trap (maskable): division by zero or overflow.
    Arithmetic,
    /// Quantum error trap (maskable): fidelity dropped below threshold.
    QuantumError,
    /// Synchronization failure trap (maskable): HMERGE thread join failed.
    SyncFailure,
}
