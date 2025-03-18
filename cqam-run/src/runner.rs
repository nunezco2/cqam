use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;

pub fn run_program(program: Vec<Instruction>) -> ExecutionContext {
    let mut ctx = ExecutionContext::new(program);

    while ctx.pc < ctx.program.len() {
        let instr = ctx.program[ctx.pc].clone();
        execute_instruction(&mut ctx, instr);

        if ctx.psw.trap_halt {
            break;
        }

        ctx.advance_pc();
    }

    ctx
}