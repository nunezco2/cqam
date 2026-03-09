//! Program execution loop for the CQAM runner.
//!
//! Provides two public entry points:
//!
//! - [`run_program`] — execute with default [`SimConfig`] (max 1000 cycles,
//!   interrupts enabled, fidelity threshold 0.95).
//! - [`run_program_with_config`] — execute with a caller-supplied
//!   [`SimConfig`] for full control over cycle limits, interrupt masking, and
//!   quantum fidelity thresholds.
//!
//! The runner owns the instruction fetch-execute loop and ISR dispatch.
//! [`execute_instruction`](cqam_vm::executor::execute_instruction) is the
//! sole authority on PC advancement; this module must not advance the PC
//! independently.

use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;
use cqam_core::parser::{DataSection, ProgramMetadata};
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;
use cqam_vm::fork::ForkManager;
use cqam_vm::isr::{self, Trap, MaskableTrap};
use crate::simconfig::SimConfig;

/// Run a complete CQAM program to termination with simulator configuration.
///
/// The executor (`execute_instruction`) is the sole authority on PC advancement.
/// This loop must NOT call `ctx.advance_pc()` independently.
///
/// Creates a ForkManager internally for HFORK/HMERGE support.
///
/// After each instruction, pending traps are dispatched through the ISR table.
/// If a handler is registered (via SETIV), execution jumps to the handler.
/// If no handler is registered, the default behavior (halt) applies.
///
/// Returns `Ok(ExecutionContext)` on normal completion, or `Err(CqamError)`
/// on runtime error.
pub fn run_program_with_config(
    program: Vec<Instruction>,
    config: &SimConfig,
) -> Result<ExecutionContext, CqamError> {
    run_program_with_config_and_metadata(program, config, &ProgramMetadata::default())
}

/// Run a complete CQAM program with configuration, metadata, and data section.
///
/// Pre-loads the `.data` section into CMEM before execution begins.
pub fn run_program_with_data(
    program: Vec<Instruction>,
    config: &SimConfig,
    metadata: &ProgramMetadata,
    data: &DataSection,
) -> Result<ExecutionContext, CqamError> {
    run_program_with_config_metadata_and_data(program, config, metadata, Some(data))
}

/// Run a complete CQAM program with configuration and program metadata.
///
/// Precedence for qubit count:
///   1. CLI --qubits flag (stored in SimConfig::default_qubits if set)
///   2. `#! qubits N` pragma (from ProgramMetadata)
///   3. Default (2 qubits)
pub fn run_program_with_config_and_metadata(
    program: Vec<Instruction>,
    config: &SimConfig,
    metadata: &ProgramMetadata,
) -> Result<ExecutionContext, CqamError> {
    run_program_with_config_metadata_and_data(program, config, metadata, None)
}

fn run_program_with_config_metadata_and_data(
    program: Vec<Instruction>,
    config: &SimConfig,
    metadata: &ProgramMetadata,
    data: Option<&DataSection>,
) -> Result<ExecutionContext, CqamError> {
    let mut ctx = ExecutionContext::new(program);

    // Pre-load .data section into CMEM
    if let Some(ds) = data {
        if !ds.cells.is_empty() {
            ctx.cmem.load_data(&ds.cells);
        }
    }
    let mut fork_mgr = ForkManager::new();
    let max_cycles = config.max_cycles.unwrap_or(1000);
    let enable_interrupts = config.enable_interrupts.unwrap_or(true);
    let mut cycle_count: usize = 0;

    // Wire fidelity_threshold from SimConfig to QuantumFidelityThreshold
    if let Some(threshold) = config.fidelity_threshold {
        ctx.config.min_purity = threshold;
    }

    // Apply qubit count with precedence: CLI > pragma > default
    if let Some(qubits) = config.default_qubits {
        ctx.config.default_qubits = qubits;
    } else if let Some(pragma_qubits) = metadata.qubits {
        ctx.config.default_qubits = pragma_qubits;
    }

    // Wire density-matrix backend flag
    ctx.config.force_density_matrix = config.force_density_matrix;

    while ctx.pc < ctx.program.len() {
        // Enforce max_cycles loop guard
        if cycle_count >= max_cycles {
            ctx.psw.trap_halt = true;
            break;
        }

        let instr = ctx.program[ctx.pc].clone();
        execute_instruction(&mut ctx, &instr, &mut fork_mgr)?;
        cycle_count += 1;

        // ISR dispatch: check for pending traps and route through ISR table
        dispatch_pending_traps(&mut ctx, enable_interrupts);

        if ctx.psw.trap_halt {
            break;
        }
    }

    Ok(ctx)
}

/// Run a complete CQAM program to termination with default configuration.
///
/// Convenience wrapper that uses [`SimConfig::default`] (max 1000 cycles,
/// interrupts enabled, fidelity threshold 0.95).
///
/// # Errors
///
/// Returns `Err(CqamError)` on runtime errors such as division by zero,
/// unresolved labels, or register out-of-bounds access.
///
/// # Examples
///
/// ```
/// use cqam_core::parser::parse_program;
/// use cqam_run::runner::run_program;
///
/// let source = "ILDI R0, 7\nILDI R1, 3\nIADD R2, R0, R1\nHALT\n";
/// let parsed = parse_program(source).unwrap();
/// let ctx = run_program(parsed.instructions).unwrap();
///
/// // R2 should contain 7 + 3 = 10
/// assert_eq!(ctx.iregs.get(2).unwrap(), 10);
/// ```
pub fn run_program(program: Vec<Instruction>) -> Result<ExecutionContext, CqamError> {
    let config = SimConfig::default();
    run_program_with_config(program, &config)
}

/// Dispatch pending maskable traps through the ISR table.
///
/// Checks each maskable trap flag in priority order. For each pending trap:
/// 1. Look up the handler address in ctx.isr_table.
/// 2. Clear the pending flag (it has been acknowledged).
/// 3. Call isr::handle_trap() with the looked-up handler address.
///
/// If handle_trap dispatches to a handler, ctx.pc is redirected to the handler
/// (with the original PC saved on the call stack for RETI).
/// If no handler is registered, handle_trap applies default behavior
/// (setting trap_halt).
fn dispatch_pending_traps(ctx: &mut ExecutionContext, enable_interrupts: bool) {
    if ctx.psw.trap_arith {
        let trap = Trap::Maskable(MaskableTrap::Arithmetic);
        let handler = ctx.isr_table.get_handler(&trap);
        ctx.psw.trap_arith = false; // Acknowledge the trap
        isr::handle_trap(trap, ctx, handler, enable_interrupts);
    }

    if ctx.psw.int_quantum_err {
        let trap = Trap::Maskable(MaskableTrap::QuantumError);
        let handler = ctx.isr_table.get_handler(&trap);
        ctx.psw.int_quantum_err = false;
        isr::handle_trap(trap, ctx, handler, enable_interrupts);
    }

    if ctx.psw.int_sync_fail {
        let trap = Trap::Maskable(MaskableTrap::SyncFailure);
        let handler = ctx.isr_table.get_handler(&trap);
        ctx.psw.int_sync_fail = false;
        isr::handle_trap(trap, ctx, handler, enable_interrupts);
    }
}
