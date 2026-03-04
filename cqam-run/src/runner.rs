use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;
use crate::simconfig::SimConfig;

/// Run a complete CQAM program to termination with simulator configuration.
///
/// The executor (`execute_instruction`) is the sole authority on PC advancement.
/// This loop must NOT call `ctx.advance_pc()` independently.
///
/// The `config` parameter controls:
/// - `max_cycles`: maximum number of instructions to execute before halting
///   (prevents infinite loops). Default: 1000.
/// - `enable_interrupts`: whether maskable interrupts are checked each cycle.
///   Default: true.
///
/// Returns `Ok(ExecutionContext)` on normal completion, or `Err(CqamError)`
/// on runtime error.
pub fn run_program_with_config(
    program: Vec<Instruction>,
    config: &SimConfig,
) -> Result<ExecutionContext, CqamError> {
    let mut ctx = ExecutionContext::new(program);
    let max_cycles = config.max_cycles.unwrap_or(1000);
    let enable_interrupts = config.enable_interrupts.unwrap_or(true);
    let mut cycle_count: usize = 0;

    while ctx.pc < ctx.program.len() {
        // Enforce max_cycles loop guard
        if cycle_count >= max_cycles {
            ctx.psw.trap_halt = true;
            break;
        }

        let instr = ctx.program[ctx.pc].clone();
        execute_instruction(&mut ctx, &instr)?;
        cycle_count += 1;

        if ctx.psw.trap_halt {
            break;
        }

        // Check maskable interrupts if enabled
        if enable_interrupts
            && (ctx.psw.trap_arith || ctx.psw.int_quantum_err || ctx.psw.int_sync_fail)
        {
            // Maskable traps halt execution when interrupts are enabled
            ctx.psw.trap_halt = true;
            break;
        }
    }

    Ok(ctx)
}

/// Run a complete CQAM program to termination with default configuration.
///
/// Convenience wrapper that uses `SimConfig::default()`.
pub fn run_program(program: Vec<Instruction>) -> Result<ExecutionContext, CqamError> {
    let config = SimConfig::default();
    run_program_with_config(program, &config)
}
