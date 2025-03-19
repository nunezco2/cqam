use cqam_core::instruction::Instruction;

pub trait OpenQASMEmitter {
    fn emit_header(&self) -> String;
    fn emit_register_declarations(&self) -> String;
    fn emit_instruction(&self, instr: &Instruction) -> Option<String>;
    fn emit_program(&self, program: &[Instruction]) -> String;
}
