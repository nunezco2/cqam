// cqam-vm/src/executor.rs

use crate::context::ExecutionContext;

/// A basic executor that simulates instruction dispatch (placeholder logic).
pub fn execute_instruction(ctx: &mut ExecutionContext) {
    if let Some(line) = ctx.current_line() {
        println!("Executing line[{}]: {}", ctx.pc, line);
        // Future: match parsed Instruction and dispatch
    }
    ctx.advance_pc();
}

/// Run the program line-by-line.
pub fn run_program(ctx: &mut ExecutionContext) {
    while ctx.pc < ctx.program.len() {
        execute_instruction(ctx);
    }
}
