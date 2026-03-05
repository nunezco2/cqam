//! Program Status Word (PSW) for the CQAM virtual machine.
//!
//! Holds all condition flags (arithmetic, quantum state, hybrid control),
//! trap flags, and provides methods for updating flags from instruction results
//! and for querying pending traps in priority order.

/// Program Status Word: holds all condition and trap flags.
#[derive(Debug, Default, Clone)]
pub struct ProgramStateWord {
    // Classical condition flags
    pub zf: bool, // Zero Flag
    pub nf: bool, // Negative Flag
    pub of: bool, // Overflow Flag
    pub pf: bool, // Predicate Flag

    // Quantum state flags
    pub qf: bool, // Quantum active
    pub sf: bool, // Superposition present
    pub ef: bool, // Entanglement present
    pub df: bool, // Decohered (measured)
    pub cf: bool, // Collapsed distribution

    // Hybrid execution context flags
    pub hf: bool,       // Hybrid mode
    pub forked: bool,   // Forked control path
    pub merged: bool,   // Merge occurred

    // Trap and interrupt flags
    pub trap_arith: bool,
    pub trap_halt: bool,
    pub int_quantum_err: bool,
    pub int_sync_fail: bool,
}

impl ProgramStateWord {
    pub fn new() -> Self {
        Self::default()
    }

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

/// Pending trap enumeration for `ProgramStateWord::check_pending_traps`.
///
/// This is a local convenience type. The authoritative trap hierarchy
/// (with two-level NMI/maskable semantics) is defined in `isr.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingTrap {
    Halt,
    Arithmetic,
    QuantumError,
    SyncFailure,
}
