//! Program Status Word (PSW) for the CQAM virtual machine.
//!
//! Holds all condition flags (arithmetic, quantum state, hybrid control),
//! trap flags, and provides methods for updating flags from instruction results
//! and for querying pending traps in priority order.

/// Program Status Word: holds all condition, quantum, hybrid, and trap flags.
///
/// Condition flags are updated by arithmetic, comparison, and quantum
/// instructions. Trap flags are set by runtime faults and checked by the ISR
/// dispatch loop after each instruction. The flag IDs used by `JMPF` and
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
    /// Superposition intent: the last quantum operation intends to
    /// create or maintain superposition.
    pub sf: bool,
    /// Entanglement intent: the last quantum operation intends to
    /// create or maintain entanglement.
    pub ef: bool,
    /// Interference intent: the last quantum operation intends to
    /// use interference to prune computational paths.
    pub inf: bool,
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
        self.of = false;
    }

    /// Update arithmetic flags from an integer result with overflow info.
    pub fn update_from_arithmetic_with_overflow(&mut self, value: i64, overflowed: bool) {
        self.zf = value == 0;
        self.nf = value < 0;
        self.of = overflowed;
    }

    /// Update arithmetic flags from a float result value.
    pub fn update_from_float_arithmetic(&mut self, value: f64) {
        self.zf = value == 0.0;
        self.nf = value < 0.0;
        self.of = value.is_infinite();
    }

    /// Update arithmetic flags from a complex result value.
    pub fn update_from_complex_arithmetic(&mut self, re: f64, im: f64) {
        self.zf = re == 0.0 && im == 0.0;
        self.nf = false; // complex numbers have no total ordering
        self.of = re.is_infinite() || im.is_infinite();
    }

    /// Update the predicate flag from a boolean result.
    pub fn update_from_predicate(&mut self, result: bool) {
        self.pf = result;
    }

    /// Update quantum state flags from purity metric.
    ///
    /// Sets qf=true (callers always have a live register).
    /// SF, EF, and IF are NOT set here -- they are intent-based flags set
    /// directly by each quantum operation handler.
    /// QOBSERVE manages qf separately via register occupancy scan.
    /// Raises int_quantum_err when purity drops below the threshold.
    pub fn update_from_qmeta(&mut self, purity: f64, threshold: f64) {
        self.qf = true;

        // DF is sticky: only set (pure->mixed transition), never cleared here.
        if purity < 1.0 - 1e-10 {
            self.df = true;
        }

        // CF is transient: any new quantum kernel supersedes stale measurement signal.
        self.cf = false;

        if threshold > 0.0 && purity < threshold {
            self.int_quantum_err = true;
        }
    }

    /// Mark that decoherence has occurred (sticky flag).
    pub fn mark_decohered(&mut self) {
        self.df = true;
    }

    /// Mark that a measurement result has been collapsed into a hybrid register.
    pub fn mark_collapsed(&mut self) {
        self.cf = true;
    }

    /// Clear the collapsed flag (e.g., after HREDUCE consumes the result).
    pub fn clear_collapsed(&mut self) {
        self.cf = false;
    }

    /// Clear the decoherence flag (e.g., after QPREP re-initialises the register).
    pub fn clear_decoherence(&mut self) {
        self.df = false;
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
            8 => self.df,
            9 => self.cf,
            10 => self.forked,
            11 => self.merged,
            12 => self.inf,
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
