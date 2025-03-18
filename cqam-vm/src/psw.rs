// cqam-vm/src/psw.rs

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

    pub fn update_from_arithmetic(&mut self, value: i64) {
        self.zf = value == 0;
        self.nf = value < 0;
        self.of = false; // Add overflow check later
    }

    pub fn update_from_predicate(&mut self, result: bool) {
        self.pf = result;
    }

    pub fn update_from_qmeta(&mut self, superposition: f64, entanglement: f64, threshold: (f64, f64)) {
        self.qf = true;
        self.sf = superposition > 0.0;
        self.ef = entanglement > 0.0;
        self.df = false;
        self.cf = false;

        if superposition < threshold.0 || entanglement < threshold.1 {
            self.int_quantum_err = true;
        }
    }

    pub fn mark_measured(&mut self) {
        self.df = true;
        self.cf = true;
    }

    pub fn check_interrupts(&self) -> Option<Trap> {
        if self.trap_arith {
            Some(Trap::Arithmetic)
        } else if self.trap_halt {
            Some(Trap::Halt)
        } else if self.int_quantum_err {
            Some(Trap::QuantumError)
        } else if self.int_sync_fail {
            Some(Trap::SyncFailure)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trap {
    Arithmetic,
    Halt,
    QuantumError,
    SyncFailure,
}
