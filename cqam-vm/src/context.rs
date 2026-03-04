// cqam-vm/src/context.rs
//
// Phase 2: Separate register files, quantum register array, call stack,
// and label resolution cache.

use std::collections::HashMap;

use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;
use cqam_core::memory::{CMem, QMem};
use cqam_core::register::{IntRegFile, FloatRegFile, ComplexRegFile, HybridRegFile};
use cqam_sim::qdist::QDist;
use crate::resource::ResourceTracker;
use crate::psw::ProgramStateWord;
use crate::simconfig::QuantumFidelityThreshold;

/// The complete execution state of the CQAM virtual machine.
///
/// Contains the program counter, all register files, memory banks,
/// call stack, PSW, and the program being executed.
pub struct ExecutionContext {
    /// Program counter: index into `self.program`.
    pub pc: usize,

    /// Integer register file: R0-R15 (16 x i64).
    pub iregs: IntRegFile,

    /// Float register file: F0-F15 (16 x f64).
    pub fregs: FloatRegFile,

    /// Complex register file: Z0-Z15 (16 x (f64, f64)).
    pub zregs: ComplexRegFile,

    /// Hybrid register file: H0-H7 (8 x HybridValue).
    pub hregs: HybridRegFile,

    /// Quantum register file: Q0-Q7 (8 x Option<QDist<u16>>).
    /// Separate from QMEM. These are the "live" quantum registers
    /// that QPREP, QKERNEL, and QOBSERVE operate on.
    pub qregs: [Option<QDist<u16>>; 8],

    /// Classical memory: 65536 cells of i64.
    pub cmem: CMem,

    /// Quantum memory: 256 slots of QDist<u16>.
    pub qmem: QMem,

    /// Call stack for CALL/RET instructions.
    /// Each entry is the return address (PC value to resume at).
    pub call_stack: Vec<usize>,

    /// Program status word (condition flags, trap flags).
    pub psw: ProgramStateWord,

    /// Quantum fidelity thresholds for interrupt generation.
    pub config: QuantumFidelityThreshold,

    /// Cumulative resource usage tracker.
    pub resource_tracker: ResourceTracker,

    /// The program being executed.
    /// Remains `Vec<Instruction>` until Phase 5 switches to `Vec<u32>`.
    pub program: Vec<Instruction>,

    /// Label resolution cache: label name -> instruction index.
    /// Populated once during construction by `resolve_labels()`.
    pub labels: HashMap<String, usize>,
}

impl ExecutionContext {
    /// Create a new execution context for the given program.
    ///
    /// Initializes all register files to zero/empty, allocates memory,
    /// and resolves all labels in the program.
    pub fn new(program: Vec<Instruction>) -> Self {
        let mut ctx = Self {
            pc: 0,
            iregs: IntRegFile::new(),
            fregs: FloatRegFile::new(),
            zregs: ComplexRegFile::new(),
            hregs: HybridRegFile::new(),
            qregs: Default::default(), // [None; 8]
            cmem: CMem::new(),
            qmem: QMem::new(),
            call_stack: Vec::new(),
            psw: ProgramStateWord::new(),
            config: QuantumFidelityThreshold::default(),
            resource_tracker: ResourceTracker::new(),
            labels: HashMap::new(),
            program,
        };
        ctx.resolve_labels();
        ctx
    }

    /// Advance the program counter by one.
    pub fn advance_pc(&mut self) {
        self.pc += 1;
    }

    /// Reset the program counter to zero.
    pub fn reset_pc(&mut self) {
        self.pc = 0;
    }

    /// Get the instruction at the current PC, or None if past end-of-program.
    pub fn current_line(&self) -> Option<&Instruction> {
        self.program.get(self.pc)
    }

    /// Jump to the instruction at the given label.
    ///
    /// Sets PC to the label's resolved address.
    /// Returns `Err(CqamError::UnresolvedLabel)` if the label is not found.
    pub fn jump_to_label(&mut self, label: &str) -> Result<(), CqamError> {
        if let Some(&addr) = self.labels.get(label) {
            self.pc = addr;
            Ok(())
        } else {
            Err(CqamError::UnresolvedLabel(label.to_string()))
        }
    }

    /// Push the current PC+1 onto the call stack (for CALL instruction).
    pub fn push_call(&mut self) {
        self.call_stack.push(self.pc + 1);
    }

    /// Pop the top of the call stack (for RET instruction).
    ///
    /// Returns `None` if the call stack is empty (indicating a RET from
    /// the top-level, which should be treated as HALT).
    pub fn pop_call(&mut self) -> Option<usize> {
        self.call_stack.pop()
    }

    /// Scan the program for Label instructions and populate the label cache.
    ///
    /// Called once during construction. Subsequent label lookups are O(1)
    /// via the HashMap, replacing the previous O(n) linear scan per jump.
    fn resolve_labels(&mut self) {
        self.labels.clear();
        for (idx, instr) in self.program.iter().enumerate() {
            if let Instruction::Label(name) = instr {
                self.labels.insert(name.clone(), idx);
            }
        }
    }
}
