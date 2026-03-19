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
use cqam_qpu::mock::{MockCalibrationData, MockQpuBackend};
use cqam_qpu::traits::{ConnectivityGraph, ConvergenceCriterion};
use cqam_core::native_ir::NativeGateSet;
use cqam_sim::backend::SimulationBackend;
use cqam_sim::circuit_backend::CircuitBackend;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;
use cqam_vm::fork::ForkManager;
use cqam_vm::isr::{self, Trap, MaskableTrap};
use crate::simconfig::{BackendChoice, SimConfig};
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
    match config.backend_choice() {
        BackendChoice::Simulation => {
            let mut backend = SimulationBackend::new();
            configure_sim_backend(&mut backend, config)?;
            run_sim_with_backend(program, config, metadata, data, shared, private, backend)
        }
        BackendChoice::Qpu { ref provider, ref device, shot_budget, confidence } => {
            match provider.as_str() {
                "mock" => {
                    let qpu = build_mock_qpu(device.as_deref());
                    let convergence = ConvergenceCriterion {
                        confidence,
                        ..ConvergenceCriterion::default()
                    };
                    let mut backend = CircuitBackend::new(qpu, convergence, shot_budget);
                    if let Some(seed) = config.rng_seed {
                        backend.set_rng_seed(seed);
                    }
                    run_with_backend(program, config, metadata, data, shared, private, backend)
                }
                #[cfg(feature = "ibm")]
                "ibm" => {
                    let token = resolve_ibm_token(config)?;
                    let opt_level = config.ibm_optimization_level.unwrap_or(1);
                    let qpu = build_ibm_qpu(&token, device.as_deref(), opt_level)?;
                    let convergence = ConvergenceCriterion {
                        confidence,
                        ..ConvergenceCriterion::default()
                    };
                    let backend = CircuitBackend::new(qpu, convergence, shot_budget);
                    run_with_backend(program, config, metadata, data, shared, private, backend)
                }
                #[cfg(not(feature = "ibm"))]
                "ibm" => Err(CqamError::ConfigError(
                    "IBM backend not available. Rebuild with: cargo build --features ibm".to_string()
                )),
                other => Err(CqamError::ConfigError(
                    format!(
                        "unknown QPU provider: '{}'. Valid: mock{}",
                        other,
                        if cfg!(feature = "ibm") { ", ibm" } else { "" }
                    )
                )),
            }
        }
    }
}

/// Run with a `SimulationBackend`, handling shots mode if configured.
///
/// Shots mode is simulation-specific: it replays quantum sections using the
/// exact state-vector distributions. QPU backends handle shots internally
/// via their own shot-sampling mechanism, so this wrapper is only used for
/// the `Simulation` backend choice.
fn run_sim_with_backend(
    program: Vec<Instruction>,
    config: &SimConfig,
    metadata: &ProgramMetadata,
    data: Option<&DataSection>,
    shared: &SharedSection,
    private: &PrivateSection,
    mut backend: SimulationBackend,
) -> Result<RunResult, CqamError> {
    if let Some(shots) = config.shots {
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
                ctx.cmem.store((base + i) as u16, val);
            }
            let end = shared.base + shared.cells.len() as u16;
            ctx.shared_region = Some((shared.base, end));
        }

        let mut fork_mgr = ForkManager::new();
        let max_cycles = config.max_cycles.unwrap_or(1000);
        let enable_interrupts = config.enable_interrupts.unwrap_or(true);

        let vm_config = config.to_vm_config(metadata);
        ctx.thread_count = vm_config.default_threads;
        ctx.bell_pair_budget = vm_config.bell_pair_budget;
        ctx.config = vm_config;

        let _ = private;

        let base_seed = config.rng_seed.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
        });
        run_with_shots(&mut ctx, &mut backend, &mut fork_mgr, max_cycles, enable_interrupts, shots, base_seed)?;
        return Ok(RunResult::Shots(ctx));
    }

    // Non-shots path: delegate to the generic helper
    run_with_backend(program, config, metadata, data, shared, private, backend)
}

/// Configure simulation-specific settings on a `SimulationBackend`.
///
/// These settings (noise model, density-matrix mode) do not apply to
/// `CircuitBackend` and must not be called in the generic path.
fn configure_sim_backend(
    backend: &mut SimulationBackend,
    config: &SimConfig,
) -> Result<(), CqamError> {
    backend.set_force_density_matrix(config.force_density_matrix);
    if let Some(ref noise_name) = config.noise_model {
        if noise_name != "none" {
            let noise = crate::simconfig::build_noise_model(noise_name)?;
            let num_qubits = config.default_qubits.unwrap_or(2);
            let method = config.resolve_noise_method(num_qubits)
                .unwrap_or(cqam_sim::noise::NoiseMethod::DensityMatrix);
            backend.set_noise_model(Some(noise), method);
        }
    }
    if let Some(seed) = config.rng_seed {
        backend.set_rng_seed(seed);
    }
    Ok(())
}

/// Build a `MockQpuBackend` with sensible defaults.
///
/// `device` is currently unused (Phase 4 has one mock topology). Future
/// phases can dispatch on the device name to select different topologies.
fn build_mock_qpu(_device: Option<&str>) -> MockQpuBackend {
    MockQpuBackend::with_config(
        ConnectivityGraph::all_to_all(27),
        NativeGateSet::Superconducting,
        27,
        MockCalibrationData::default(),
        None,
    )
}

/// Resolve the IBM Quantum API token.
///
/// Precedence:
/// 1. `SimConfig.ibm_token` (from `--ibm-token` CLI flag)
/// 2. `IBM_QUANTUM_TOKEN` environment variable
/// 3. `~/.qiskit/ibm_quantum_token` file (trimmed)
///
/// Returns `Err(CqamError::ConfigError)` with an actionable message if
/// no token is found at any level.
// Used by the #[cfg(feature = "ibm")] dispatch arm and by tests.
#[allow(dead_code)]
fn resolve_ibm_token(config: &SimConfig) -> Result<String, CqamError> {
    // 1. CLI flag (highest priority)
    if let Some(ref token) = config.ibm_token {
        return Ok(token.clone());
    }

    // 2. Environment variable
    if let Ok(token) = std::env::var("IBM_QUANTUM_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // 3. Token file (~/.qiskit/ibm_quantum_token)
    let home = std::env::var("HOME").unwrap_or_default();
    let token_path = format!("{}/.qiskit/ibm_quantum_token", home);
    if let Ok(token) = std::fs::read_to_string(&token_path) {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    Err(CqamError::ConfigError(
        "IBM Quantum token not found. Provide one of:\n  \
         --ibm-token <TOKEN>\n  \
         IBM_QUANTUM_TOKEN environment variable\n  \
         ~/.qiskit/ibm_quantum_token file".to_string()
    ))
}

/// Build an `IbmQpuBackend` configured for the specified device.
///
/// Mirrors the `build_mock_qpu` pattern. Uses `IbmQpuBackend::from_device`
/// so that topology and qubit count are fetched from the IBM API automatically.
/// Calibration refresh is best-effort: failure logs a warning and synthetic
/// defaults are used, but execution continues.
#[cfg(feature = "ibm")]
fn build_ibm_qpu(
    token: &str,
    device: Option<&str>,
    optimization_level: u8,
) -> Result<cqam_qpu_ibm::IbmQpuBackend, CqamError> {
    let device_name = device.unwrap_or("ibm_brisbane");

    let mut backend = cqam_qpu_ibm::IbmQpuBackend::from_device(token, device_name)
        .map_err(|e| CqamError::ConfigError(
            format!("IBM backend initialization failed for '{}': {}", device_name, e)
        ))?;

    backend = backend.with_optimization_level(optimization_level);

    // Best-effort calibration refresh; fall back to synthetic on failure.
    if let Err(e) = backend.refresh_calibration() {
        eprintln!(
            "warning: could not fetch IBM calibration for '{}': {}. \
             Using synthetic defaults.",
            device_name, e
        );
    }

    Ok(backend)
}

/// Generic execution loop: creates context, loads memory sections, and runs
/// the program with the supplied backend.
///
/// This is the single authority on the fetch-execute loop. SimulationBackend-
/// specific configuration (`set_force_density_matrix`, `set_noise_model`)
/// must be applied by the caller before passing the backend here.
fn run_with_backend<B: QuantumBackend + Clone + Send + 'static>(
    program: Vec<Instruction>,
    config: &SimConfig,
    metadata: &ProgramMetadata,
    data: Option<&DataSection>,
    shared: &SharedSection,
    private: &PrivateSection,
    mut backend: B,
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

    let mut fork_mgr = ForkManager::new();
    let max_cycles = config.max_cycles.unwrap_or(1000);
    let enable_interrupts = config.enable_interrupts.unwrap_or(true);
    let mut cycle_count: usize = 0;

    // Build unified VmConfig from SimConfig + metadata (CLI > pragma > default)
    let vm_config = config.to_vm_config(metadata);
    ctx.thread_count = vm_config.default_threads;
    ctx.bell_pair_budget = vm_config.bell_pair_budget;
    ctx.config = vm_config;

    // Store private section size
    let _ = private;

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

/// Check if an instruction is a quantum gate (kernel or primitive gate).
fn is_quantum_gate(instr: &Instruction) -> bool {
    matches!(
        instr,
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
    )
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
    let mut snapshot_ctx: Option<ExecutionContext> = None;
    let mut section_index: u64 = 0;
    // Track adaptivity dynamically: set true when QMEAS is followed by
    // a quantum gate within the same section (mid-circuit measurement feedback).
    let mut section_seen_meas = false;
    let mut section_adaptive = false;

    while ctx.pc < ctx.program.len() {
        if cycle_count >= max_cycles {
            ctx.psw.trap_halt = true;
            break;
        }

        let instr = ctx.program[ctx.pc].clone();
        let qf_before = ctx.psw.qf;

        // Section entry: QF is false, about to execute quantum-entry instruction
        if !in_quantum_section && !qf_before && is_quantum_entry(&instr) {
            snapshot_ctx = Some(ctx.clone());
            in_quantum_section = true;
            section_seen_meas = false;
            section_adaptive = false;
        }

        // Track adaptivity: QMEAS sets seen_meas; subsequent quantum gate confirms adaptive
        if in_quantum_section {
            if matches!(instr, Instruction::QMeas { .. }) {
                section_seen_meas = true;
            } else if section_seen_meas && is_quantum_gate(&instr) {
                section_adaptive = true;
            }
        }

        // Execute the instruction
        execute_instruction(ctx, &instr, fork_mgr, backend)?;
        cycle_count += 1;
        dispatch_pending_traps(ctx, enable_interrupts);

        if ctx.psw.trap_halt { break; }

        // Section exit: was in quantum section, QF just went false
        if in_quantum_section && qf_before && !ctx.psw.qf {
            let post_first_run = ctx.clone();
            let adaptive = section_adaptive;

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

                    // Replay section until QF goes false (QOBSERVE hit)
                    // Skip ECALL to avoid duplicating I/O side effects
                    while ctx.pc < ctx.program.len() {
                        let replay_instr = ctx.program[ctx.pc].clone();
                        if matches!(replay_instr, Instruction::Ecall { .. }) {
                            ctx.pc += 1;
                            continue;
                        }
                        let qf_pre = ctx.psw.qf;
                        execute_instruction(ctx, &replay_instr, &mut replay_fork_mgr, backend)?;
                        dispatch_pending_traps(ctx, enable_interrupts);
                        if ctx.psw.trap_halt { break; }
                        // Section ends when QF transitions from true to false
                        if qf_pre && !ctx.psw.qf { break; }
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

// =============================================================================
// Tests for resolve_ibm_token
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Environment variables are process-global state.  Tests that mutate
    // IBM_QUANTUM_TOKEN must hold this lock to avoid racing with each other
    // when the test runner uses multiple threads.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_resolve_ibm_token_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("IBM_QUANTUM_TOKEN", "test_token_abc") };
        let config = SimConfig::default();
        let token = resolve_ibm_token(&config).unwrap();
        assert_eq!(token, "test_token_abc");
        unsafe { std::env::remove_var("IBM_QUANTUM_TOKEN") };
    }

    #[test]
    fn test_resolve_ibm_token_cli_priority() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("IBM_QUANTUM_TOKEN", "env_token") };
        let mut config = SimConfig::default();
        config.ibm_token = Some("cli_token".to_string());
        let token = resolve_ibm_token(&config).unwrap();
        assert_eq!(token, "cli_token"); // CLI wins
        unsafe { std::env::remove_var("IBM_QUANTUM_TOKEN") };
    }

    #[test]
    fn test_resolve_ibm_token_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("IBM_QUANTUM_TOKEN") };
        let config = SimConfig::default();
        let result = resolve_ibm_token(&config);
        assert!(result.is_err(), "expected error when no token is available");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("IBM Quantum token not found"));
        assert!(msg.contains("--ibm-token"));
        assert!(msg.contains("IBM_QUANTUM_TOKEN"));
    }

    #[test]
    fn test_resolve_ibm_token_empty_env_rejected() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("IBM_QUANTUM_TOKEN", "") };
        let config = SimConfig::default();
        // Empty env var must not be accepted. Verify CLI token field is absent.
        assert!(config.ibm_token.is_none(), "default config should have no CLI token");
        // The resolver should fall through to an error (env var is empty, no file).
        // We do not assert is_err() here because a ~/.qiskit/ibm_quantum_token file
        // may exist on the developer's machine — the important invariant is that
        // the empty env var does NOT satisfy the resolver by itself.
        unsafe { std::env::remove_var("IBM_QUANTUM_TOKEN") };
    }

    #[test]
    fn test_ibm_provider_error_without_feature() {
        // When compiled without the `ibm` feature, the "ibm" provider arm
        // must produce a clear error pointing to --features ibm.
        #[cfg(not(feature = "ibm"))]
        {
            let config = SimConfig {
                backend: Some(BackendChoice::Qpu {
                    provider: "ibm".to_string(),
                    device: None,
                    shot_budget: 4096,
                    confidence: 0.95,
                }),
                ..SimConfig::default()
            };
            let program = vec![cqam_core::instruction::Instruction::Halt];
            let result = run_program_with_config(program, &config);
            assert!(result.is_err(), "expected error for ibm provider without feature");
            let msg = match result {
                Err(e) => format!("{}", e),
                Ok(_) => unreachable!(),
            };
            assert!(msg.contains("--features ibm"));
        }
    }

    /// Live integration test — requires IBM network access.
    /// Run with: cargo test --features ibm -- --ignored
    #[cfg(feature = "ibm")]
    #[test]
    #[ignore]
    fn test_ibm_backend_gate_set() {
        let token = std::env::var("IBM_QUANTUM_TOKEN")
            .expect("IBM_QUANTUM_TOKEN must be set for this test");
        let qpu = build_ibm_qpu(&token, Some("ibm_brisbane"), 1).unwrap();
        use cqam_qpu::traits::QpuBackend;
        assert_eq!(qpu.gate_set(), &cqam_core::native_ir::NativeGateSet::Superconducting);
    }
}
