use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;
use crate::loader::parse_line;

pub fn run_program(program: Vec<String>) -> ExecutionContext {
    let mut ctx = ExecutionContext::new(program.clone());

    while let Some(line) = ctx.current_line() {
        if let Some(instr) = parse_line(line) {
            execute_instruction(&mut ctx, instr);
        } else {
            eprintln!("Warning: Failed to parse instruction: {}", line);
        }

        if ctx.psw.trap_halt {
            break;
        }

        ctx.advance_pc();
    }

    ctx
}
