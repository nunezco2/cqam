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

    // If shots mode is enabled, run with per-section shot sampling
    if let Some(shots) = config.shots {
        let base_seed = config.rng_seed.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
        });
        run_with_shots(&mut ctx, &mut backend, &mut fork_mgr, max_cycles, enable_interrupts, shots, base_seed)?;
        return Ok(RunResult::Shots(ctx));
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

/// Check if instructions [start..=end] contain a mid-circuit measurement
/// feedback pattern (QMEAS followed by a quantum gate).
fn section_is_adaptive(program: &[Instruction], start: usize, end: usize) -> bool {
    let mut seen_meas = false;
    for instr in &program[start..=end] {
        match instr {
            Instruction::QMeas { .. } => {
                seen_meas = true;
            }
            Instruction::QKernel { .. }
            | Instruction::QKernelF { .. }
            | Instruction::QKernelZ { .. }
            | Instruction::QHadM { .. }
            | Instruction::QFlip { .. }
            | Instruction::QPhase { .. }
            | Instruction::QCnot { .. }
            | Instruction::QRot { .. }
            | Instruction::QCz { .. }
            | Instruction::QSwap { .. }
            | Instruction::QCustom { .. }
            if seen_meas => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Run a program in shot mode with per-quantum-section shot loops.
///
/// Classical code (including ECALL) executes exactly once. Each quantum
/// section (QPREP→QOBSERVE while QF is on) is independently sampled
/// N times:
///   - Fast path (non-adaptive): resample exact Dist in H registers N times
///   - Slow path (adaptive, QMEAS→gate feedback): replay section N-1 more
///     times from a snapshot, accumulate histogram
fn run_with_shots(
    ctx: &mut ExecutionContext,
    backend: &mut SimulationBackend,
    fork_mgr: &mut ForkManager,
    max_cycles: usize,
    enable_interrupts: bool,
    shots: u32,
    base_seed: u64,
) -> Result<(), CqamError> {
    use std::collections::BTreeMap;
    use cqam_core::shot::ShotHistogram;

    let mut cycle_count: usize = 0;
    let mut in_quantum_section = false;
    let mut section_start_pc: usize = 0;
    let mut snapshot_ctx: Option<ExecutionContext> = None;
    let mut section_index: u64 = 0;

    while ctx.pc < ctx.program.len() {
        if cycle_count >= max_cycles {
            ctx.psw.trap_halt = true;
            break;
        }

        let pc_before = ctx.pc;
        let instr = ctx.program[ctx.pc].clone();
        let qf_before = ctx.psw.qf;

        // Section entry: QF is false, about to execute quantum-entry instruction
        if !in_quantum_section && !qf_before && is_quantum_entry(&instr) {
            section_start_pc = pc_before;
            snapshot_ctx = Some(ctx.clone());
            in_quantum_section = true;
        }

        // Execute the instruction
        execute_instruction(ctx, &instr, fork_mgr, backend)?;
        cycle_count += 1;
        dispatch_pending_traps(ctx, enable_interrupts);

        if ctx.psw.trap_halt { break; }

        // Section exit: was in quantum section, QF just went false
        if in_quantum_section && qf_before && !ctx.psw.qf {
            let section_end_pc = pc_before;
            let post_first_run = ctx.clone();
            let adaptive = section_is_adaptive(&ctx.program, section_start_pc, section_end_pc);

            if adaptive {
                // Slow path: replay section N-1 more times
                let mut accumulators: [BTreeMap<u32, u32>; 8] = Default::default();

                // Collect first shot outcomes
                for i in 0..8u8 {
                    if let Ok(val) = ctx.hregs.get(i) {
                        let outcome = match val {
                            HybridValue::Dist(entries) => {
                                Some(crate::shot::sample_from_dist_seeded(
                                    entries,
                                    base_seed.wrapping_add(section_index * 1_000_000),
                                ))
                            }
                            HybridValue::Int(k) => Some(*k as u32),
                            _ => None,
                        };
                        if let Some(o) = outcome {
                            *accumulators[i as usize].entry(o).or_insert(0) += 1;
                        }
                    }
                }

                let snap = snapshot_ctx.as_ref().unwrap();
                for shot_idx in 1..shots {
                    *ctx = snap.clone();
                    let shot_seed = base_seed
                        .wrapping_add(section_index * 1_000_000)
                        .wrapping_add(shot_idx as u64);
                    backend.set_rng_seed(shot_seed);
                    let mut replay_fork_mgr = ForkManager::new();

                    // Replay section (skip ECALL to avoid duplicating I/O side effects)
                    while ctx.pc <= section_end_pc && ctx.pc < ctx.program.len() {
                        let replay_instr = ctx.program[ctx.pc].clone();
                        if matches!(replay_instr, Instruction::Ecall { .. }) {
                            // Skip ECALL but still advance PC past it
                            ctx.pc += 1;
                            continue;
                        }
                        execute_instruction(ctx, &replay_instr, &mut replay_fork_mgr, backend)?;
                        dispatch_pending_traps(ctx, enable_interrupts);
                        if ctx.psw.trap_halt { break; }
                    }

                    // Collect outcomes
                    for i in 0..8u8 {
                        if let Ok(val) = ctx.hregs.get(i) {
                            let outcome = match val {
                                HybridValue::Dist(entries) => {
                                    Some(crate::shot::sample_from_dist_seeded(
                                        entries,
                                        shot_seed.wrapping_add(i as u64 * 1000),
                                    ))
                                }
                                HybridValue::Int(k) => Some(*k as u32),
                                _ => None,
                            };
                            if let Some(o) = outcome {
                                *accumulators[i as usize].entry(o).or_insert(0) += 1;
                            }
                        }
                    }
                }

                // Restore canonical post-section state, write histograms
                *ctx = post_first_run;
                for i in 0..8u8 {
                    if !accumulators[i as usize].is_empty() {
                        let mut hist = ShotHistogram::new(shots);
                        hist.counts = accumulators[i as usize].clone();
                        let _ = ctx.hregs.set(i, HybridValue::Hist(hist));
                    }
                }
            } else {
                // Fast path: resample exact distributions
                let section_seed = base_seed.wrapping_add(section_index * 1_000_000);
                for i in 0..8u8 {
                    match ctx.hregs.get(i) {
                        Ok(HybridValue::Dist(entries)) => {
                            let hist = resample_dist(
                                entries, shots,
                                section_seed.wrapping_add(i as u64),
                            );
                            let _ = ctx.hregs.set(i, HybridValue::Hist(hist));
                        }
                        Ok(HybridValue::Int(k)) => {
                            let mut hist = ShotHistogram::new(shots);
                            hist.counts.insert(*k as u32, shots);
                            let _ = ctx.hregs.set(i, HybridValue::Hist(hist));
                        }
                        _ => {}
                    }
                }
            }

            in_quantum_section = false;
            snapshot_ctx = None;
            section_index += 1;
        }
    }

    // Program-exit flush: convert any remaining Dist/Int in H registers to Hist.
    // This handles programs that end without closing a quantum section (QF still true)
    // or programs where QOBSERVE results were never resampled.
    for i in 0..8u8 {
        match ctx.hregs.get(i) {
            Ok(HybridValue::Dist(entries)) => {
                let section_seed = base_seed.wrapping_add(section_index * 1_000_000);
                let hist = resample_dist(
                    entries, shots,
                    section_seed.wrapping_add(i as u64),
                );
                let _ = ctx.hregs.set(i, HybridValue::Hist(hist));
            }
            Ok(HybridValue::Int(k)) => {
                let mut hist = ShotHistogram::new(shots);
                hist.counts.insert(*k as u32, shots);
                let _ = ctx.hregs.set(i, HybridValue::Hist(hist));
            }
            _ => {}
        }
    }
    Ok(())
}
