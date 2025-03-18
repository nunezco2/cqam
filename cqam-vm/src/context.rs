// cqam-vm/src/context.rs

use cqam_core::memory::{CMEM, QMEM};
use cqam_core::register::RegisterBank;

/// Represents the execution state of the CQAM interpreter.
pub struct ExecutionContext {
    pub pc: usize,
    pub cmem: CMEM,
    pub qmem: QMEM,
    pub registers: RegisterBank,
    pub program: Vec<String>, // Later replaced by parsed Instruction
}

impl ExecutionContext {
    pub fn new(program: Vec<String>) -> Self {
        Self {
            pc: 0,
            cmem: CMEM::new(),
            qmem: QMEM::new(),
            registers: RegisterBank::new(),
            program,
        }
    }

    pub fn advance_pc(&mut self) {
        self.pc += 1;
    }

    pub fn reset_pc(&mut self) {
        self.pc = 0;
    }

    pub fn current_line(&self) -> Option<&String> {
        self.program.get(self.pc)
    }
}
