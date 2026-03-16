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
use cqam_core::parser::{DataSection, ProgramMetadata, SharedSection, PrivateSection};
use cqam_core::quantum_backend::QuantumBackend;
use cqam_core::register::HybridValue;
use cqam_sim::backend::SimulationBackend;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;
use cqam_vm::fork::ForkManager;
use cqam_vm::isr::{self, Trap, MaskableTrap};
use crate::simconfig::SimConfig;
use crate::shot::{RunResult, resample_dist};

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
/// Returns `Ok(RunResult)` on normal completion, or `Err(CqamError)`
/// on runtime error.
pub fn run_program_with_config(
    program: Vec<Instruction>,
    config: &SimConfig,
) -> Result<RunResult, CqamError> {
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
    shared: &SharedSection,
    private: &PrivateSection,
) -> Result<RunResult, CqamError> {
    run_program_with_config_metadata_and_data(program, config, metadata, Some(data), shared, private)
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
) -> Result<RunResult, CqamError> {
    run_program_with_config_metadata_and_data(program, config, metadata, None, &SharedSection::default(), &PrivateSection::default())
}

fn run_program_with_config_metadata_and_data(
    program: Vec<Instruction>,
    config: &SimConfig,
    metadata: &ProgramMetadata,
    data: Option<&DataSection>,
    shared: &SharedSection,
    private: &PrivateSection,
) -> Result<RunResult, CqamError> {
    let mut ctx = ExecutionContext::new(program);

    // Pre-load .data section into CMEM
    if let Some(ds) = data {
        if !ds.cells.is_empty() {
            ctx.cmem.load_data(&ds.cells);
        }
    }

    // Pre-load .shared section into CMEM at shared.base
    if !shared.cells.is_empty() {
        let base = shared.base as usize;
        for (i, &val) in shared.cells.iter().enumerate() {
            ctx.cmem.store(
                (base + i) as u16,
                val,
            );
        }
        let end = shared.base + shared.cells.len() as u16;
        ctx.shared_region = Some((shared.base, end));
    }

    // Create the simulation backend
    let mut backend = SimulationBackend::new();

    let mut fork_mgr = ForkManager::new();
    let max_cycles = config.max_cycles.unwrap_or(1000);
    let enable_interrupts = config.enable_interrupts.unwrap_or(true);
    let mut cycle_count: usize = 0;

    // Build unified VmConfig from SimConfig + metadata (CLI > pragma > default)
    let vm_config = config.to_vm_config(metadata);
    ctx.thread_count = vm_config.default_threads;
    ctx.config = vm_config;

    // Wire density-matrix backend flag
    backend.set_force_density_matrix(config.force_density_matrix);

    // Set up noise model if configured
    if let Some(ref noise_name) = config.noise_model {
        if noise_name != "none" {
            let noise = crate::simconfig::build_noise_model(noise_name)?;
            let num_qubits = ctx.config.default_qubits;
            let method = config.resolve_noise_method(num_qubits)
                .unwrap_or(cqam_sim::noise::NoiseMethod::DensityMatrix);
            backend.set_noise_model(Some(noise), method);
        }
    }

    // Set RNG seed on backend if configured
    if let Some(seed) = config.rng_seed {
        backend.set_rng_seed(seed);
    }

    // Store private section size
    let _ = private;

    // If shots mode is enabled, run with shot sampling
    if let Some(shots) = config.shots {
        return run_with_shots(
            &mut ctx,
            &mut backend,
            &mut fork_mgr,
            max_cycles,
            enable_interrupts,
            shots,
            config.rng_seed.unwrap_or(42),
        );
    }

    while ctx.pc < ctx.program.len() {
        // Enforce max_cycles loop guard
        if cycle_count >= max_cycles {
            ctx.psw.trap_halt = true;
            break;
        }

        let instr = ctx.program[ctx.pc].clone();
        execute_instruction(&mut ctx, &instr, &mut fork_mgr, &mut backend)?;
        cycle_count += 1;

        // ISR dispatch: check for pending traps and route through ISR table
        dispatch_pending_traps(&mut ctx, enable_interrupts);

        if ctx.psw.trap_halt {
            break;
        }
    }

    Ok(RunResult::Exact(ctx))
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
/// let result = run_program(parsed.instructions).unwrap();
///
/// // R2 should contain 7 + 3 = 10
/// assert_eq!(result.ctx().iregs.get(2).unwrap(), 10);
/// ```
pub fn run_program(program: Vec<Instruction>) -> Result<RunResult, CqamError> {
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

// =============================================================================
// Shot-mode execution
// =============================================================================

/// Check if an instruction is a quantum entry point (prepares quantum state).
fn is_quantum_entry(instr: &Instruction) -> bool {
    matches!(
        instr,
        Instruction::QPrep { .. }
            | Instruction::QPrepR { .. }
            | Instruction::QPrepN { .. }
            | Instruction::QEncode { .. }
            | Instruction::QMixed { .. }
            | Instruction::QLoad { .. }
    )
}

/// Check if an instruction is a terminal observation (collapses quantum state).
fn is_terminal_observation(instr: &Instruction) -> bool {
    matches!(
        instr,
        Instruction::QObserve { .. } | Instruction::QSample { .. }
    )
}

/// Check if a program section contains mid-circuit measurements.
///
/// A mid-circuit measurement is a QObserve/QSample that is followed by
/// another quantum entry point (QPREP/etc.) before the section ends.
fn section_has_mid_circuit_measurement(program: &[Instruction], section_start: usize) -> bool {
    let mut seen_observation = false;
    for instr in program.iter().skip(section_start) {
        if matches!(instr, Instruction::Halt) {
            break;
        }
        if is_terminal_observation(instr) {
            seen_observation = true;
        } else if seen_observation && is_quantum_entry(instr) {
            return true;
        }
    }
    false
}

/// Run a program in shot mode: execute the program once to get exact
/// distributions, then resample all Dist values in H registers into
/// ShotHistograms.
///
/// This is the "fast path" — it runs the program once and resamples
/// the resulting distributions. For programs with mid-circuit measurements
/// that require re-execution, the slow path would re-run the program N times,
/// but the fast path is sufficient for most use cases.
fn run_with_shots(
    ctx: &mut ExecutionContext,
    backend: &mut SimulationBackend,
    fork_mgr: &mut ForkManager,
    max_cycles: usize,
    enable_interrupts: bool,
    shots: u32,
    base_seed: u64,
) -> Result<RunResult, CqamError> {
    let has_mid_circuit = section_has_mid_circuit_measurement(&ctx.program, 0);

    if has_mid_circuit {
        // Slow path: re-run program N times and accumulate results
        run_shots_slow(ctx, backend, fork_mgr, max_cycles, enable_interrupts, shots, base_seed)
    } else {
        // Fast path: run once, resample distributions
        run_shots_fast(ctx, backend, fork_mgr, max_cycles, enable_interrupts, shots, base_seed)
    }
}

/// Fast-path shot mode: run program once exactly, then resample all
/// Dist values in H registers into ShotHistograms.
fn run_shots_fast(
    ctx: &mut ExecutionContext,
    backend: &mut SimulationBackend,
    fork_mgr: &mut ForkManager,
    max_cycles: usize,
    enable_interrupts: bool,
    shots: u32,
    base_seed: u64,
) -> Result<RunResult, CqamError> {
    let mut cycle_count: usize = 0;

    // Run the program to completion (exact simulation)
    while ctx.pc < ctx.program.len() {
        if cycle_count >= max_cycles {
            ctx.psw.trap_halt = true;
            break;
        }

        let instr = ctx.program[ctx.pc].clone();
        execute_instruction(ctx, &instr, fork_mgr, backend)?;
        cycle_count += 1;

        dispatch_pending_traps(ctx, enable_interrupts);

        if ctx.psw.trap_halt {
            break;
        }
    }

    // Resample all Dist values in H registers into ShotHistograms
    execute_section_fast(ctx, shots, base_seed);

    Ok(RunResult::Shots(ctx.clone()))
}

/// Resample all Dist values in H registers into ShotHistograms.
fn execute_section_fast(ctx: &mut ExecutionContext, shots: u32, base_seed: u64) {
    for i in 0..8u8 {
        if let Ok(HybridValue::Dist(entries)) = ctx.hregs.get(i) {
            let hist = resample_dist(entries, shots, base_seed.wrapping_add(i as u64));
            let _ = ctx.hregs.set(i, HybridValue::Hist(hist));
        }
    }
}

/// Slow-path shot mode: re-run the program N times and accumulate
/// measurement results into histograms.
///
/// This handles programs with mid-circuit measurements where the
/// quantum state depends on previous measurement outcomes.
fn run_shots_slow(
    ctx: &mut ExecutionContext,
    backend: &mut SimulationBackend,
    fork_mgr: &mut ForkManager,
    max_cycles: usize,
    enable_interrupts: bool,
    shots: u32,
    base_seed: u64,
) -> Result<RunResult, CqamError> {
    use std::collections::BTreeMap;
    use cqam_core::shot::ShotHistogram;

    // We need to snapshot the initial state and re-run for each shot
    let initial_ctx = ctx.clone();

    // Track histograms per H register across all shots
    let mut accumulators: [BTreeMap<u32, u32>; 8] = Default::default();

    for shot_idx in 0..shots {
        // Reset to initial state
        *ctx = initial_ctx.clone();
        // Create a fresh backend for each shot with a unique seed
        let shot_seed = base_seed.wrapping_add(shot_idx as u64);
        backend.set_rng_seed(shot_seed);

        let mut cycle_count: usize = 0;
        *fork_mgr = ForkManager::new();

        while ctx.pc < ctx.program.len() {
            if cycle_count >= max_cycles {
                ctx.psw.trap_halt = true;
                break;
            }

            let instr = ctx.program[ctx.pc].clone();
            execute_instruction(ctx, &instr, fork_mgr, backend)?;
            cycle_count += 1;

            dispatch_pending_traps(ctx, enable_interrupts);

            if ctx.psw.trap_halt {
                break;
            }
        }

        // Accumulate measurement results from H registers
        for i in 0..8u8 {
            if let Ok(val) = ctx.hregs.get(i) {
                match val {
                    HybridValue::Dist(entries) => {
                        // For Dist, find the mode (most probable outcome) as a single sample
                        // In the slow path, each run produces a distribution — we
                        // sample once from it for this shot
                        let outcome = crate::shot::sample_from_dist_seeded(
                            entries,
                            base_seed.wrapping_add(shot_idx as u64).wrapping_add(i as u64 * 1000),
                        );
                        *accumulators[i as usize].entry(outcome).or_insert(0) += 1;
                    }
                    HybridValue::Int(k) => {
                        *accumulators[i as usize].entry(*k as u32).or_insert(0) += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    // Reset ctx to the last run's state, then replace H registers with histograms
    for i in 0..8u8 {
        if !accumulators[i as usize].is_empty() {
            let mut hist = ShotHistogram::new(shots);
            hist.counts = accumulators[i as usize].clone();
            let _ = ctx.hregs.set(i, HybridValue::Hist(hist));
        }
    }

    Ok(RunResult::Shots(ctx.clone()))
}
