//! Debugger engine: owns the VM execution context and provides instrumented
//! single-step execution with breakpoint, watchpoint, and snapshot support.

pub mod breakpoint;
pub mod condition;
pub mod snapshot;
pub mod watchpoint;

use std::path::PathBuf;

use cqam_core::instruction::Instruction;
use cqam_core::parser::{DataSection, ProgramMetadata};
use cqam_vm::context::ExecutionContext;
use cqam_vm::fork::ForkManager;
use cqam_vm::isr::{self, MaskableTrap, Trap};
use crate::ecall::EcallInterceptor;
use crate::engine::breakpoint::BreakpointTable;
use crate::engine::snapshot::RegisterSnapshot;
use crate::engine::watchpoint::WatchpointTable;
use cqam_run::simconfig::SimConfig;

/// Result of a single step in the debugger engine.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Why execution stopped after this step (if it did).
    pub stopped_reason: Option<StopReason>,
    /// The instruction that was executed (if any).
    #[allow(dead_code)]
    pub instruction_executed: Option<Instruction>,
}

/// Reason execution paused.
#[derive(Debug, Clone)]
pub enum StopReason {
    /// A breakpoint fired. Contains the breakpoint ID.
    Breakpoint(usize),
    /// A watchpoint triggered. Contains the register name(s).
    Watchpoint(Vec<String>),
    /// The program halted (HALT instruction or trap_halt).
    Halted,
    /// An error occurred.
    Error(String),
    /// PC went out of bounds.
    EndOfProgram,
    /// Max cycles exceeded.
    MaxCycles,
}

/// The debugger engine: wraps the VM with debugging instrumentation.
pub struct DebuggerEngine {
    /// The VM execution context.
    pub ctx: ExecutionContext,
    /// Fork manager for HFORK/HMERGE.
    pub fork_mgr: ForkManager,
    /// Breakpoint table.
    pub breakpoints: BreakpointTable,
    /// Watchpoint table.
    pub watchpoints: WatchpointTable,
    /// Previous register snapshot for change detection.
    pub prev_snapshot: RegisterSnapshot,
    /// Instruction cycle count.
    pub cycle_count: usize,
    /// Maximum allowed cycles.
    pub max_cycles: usize,
    /// Whether ISR dispatch is enabled.
    pub enable_interrupts: bool,
    /// ECALL output interceptor.
    pub ecall_interceptor: EcallInterceptor,
    /// Path to the loaded program file.
    pub program_path: PathBuf,
    /// Simulator configuration.
    pub sim_config: SimConfig,
}

impl DebuggerEngine {
    /// Create a new debugger engine from a program and configuration.
    pub fn new(
        program: Vec<Instruction>,
        program_path: PathBuf,
        sim_config: SimConfig,
    ) -> Self {
        Self::new_with_metadata(
            program,
            program_path,
            sim_config,
            &ProgramMetadata::default(),
            None,
        )
    }

    /// Create a new debugger engine with full program metadata and data section.
    ///
    /// Applies pragma qubits from metadata (if no CLI override in sim_config),
    /// and pre-loads the `.data` section into CMEM.
    pub fn new_with_metadata(
        program: Vec<Instruction>,
        program_path: PathBuf,
        sim_config: SimConfig,
        metadata: &ProgramMetadata,
        data: Option<&DataSection>,
    ) -> Self {
        let mut ctx = ExecutionContext::new(program);
        let max_cycles = sim_config.max_cycles.unwrap_or(100_000);
        let enable_interrupts = sim_config.enable_interrupts.unwrap_or(true);

        // Pre-load .data section into CMEM.
        if let Some(ds) = data {
            if !ds.cells.is_empty() {
                ctx.cmem.load_data(&ds.cells);
            }
        }

        // Wire fidelity_threshold from SimConfig.
        if let Some(threshold) = sim_config.fidelity_threshold {
            ctx.config.min_purity = threshold;
        }
        // Apply qubit count: CLI override > pragma > default.
        if let Some(qubits) = sim_config.default_qubits {
            ctx.config.default_qubits = qubits;
        } else if let Some(pragma_qubits) = metadata.qubits {
            ctx.config.default_qubits = pragma_qubits;
        }
        // Wire density-matrix backend flag.
        ctx.config.force_density_matrix = sim_config.force_density_matrix;

        let prev_snapshot = RegisterSnapshot::capture(&ctx);

        Self {
            ctx,
            fork_mgr: ForkManager::new(),
            breakpoints: BreakpointTable::new(),
            watchpoints: WatchpointTable::new(),
            prev_snapshot,
            cycle_count: 0,
            max_cycles,
            enable_interrupts,
            ecall_interceptor: EcallInterceptor::new(),
            program_path,
            sim_config,
        }
    }

    /// Execute a single instruction with full debugger instrumentation.
    pub fn step_one(&mut self) -> StepResult {
        // 1. Check PC bounds.
        if self.ctx.pc >= self.ctx.program.len() {
            return StepResult {
                stopped_reason: Some(StopReason::EndOfProgram),
                instruction_executed: None,
            };
        }

        // 2. Check max cycles.
        if self.cycle_count >= self.max_cycles {
            return StepResult {
                stopped_reason: Some(StopReason::MaxCycles),
                instruction_executed: None,
            };
        }

        // 3. Snapshot registers for change detection.
        self.prev_snapshot = RegisterSnapshot::capture(&self.ctx);

        // 4. Clone instruction at current PC.
        let instr = self.ctx.program[self.ctx.pc].clone();

        // 5. Check breakpoints (address, class, conditional).
        let bp_hits = self.breakpoints.check(self.ctx.pc, &instr);
        for &bp_id in &bp_hits {
            let should_fire = if let Some(bp) = self.breakpoints.get(bp_id) {
                match &bp.condition {
                    Some(cond) => cond.evaluate(&self.ctx),
                    None => true,
                }
            } else {
                false
            };
            if should_fire {
                self.breakpoints.record_hit(bp_id);
                return StepResult {
                    stopped_reason: Some(StopReason::Breakpoint(bp_id)),
                    instruction_executed: None,
                };
            }
        }

        // 6. Handle ECALL interception.
        if let Instruction::Ecall { .. } = &instr {
            self.ecall_interceptor
                .handle_ecall(&mut self.ctx, self.cycle_count);
            self.cycle_count += 1;

            // Check watchpoints after ECALL.
            let triggered = self.watchpoints.check(&self.prev_snapshot, &self.ctx);
            if !triggered.is_empty() {
                return StepResult {
                    stopped_reason: Some(StopReason::Watchpoint(triggered)),
                    instruction_executed: Some(instr),
                };
            }

            return StepResult {
                stopped_reason: None,
                instruction_executed: Some(instr),
            };
        }

        // 7. Normal execution path: delegate to execute_instruction.
        if let Err(e) = cqam_vm::executor::execute_instruction(
            &mut self.ctx,
            &instr,
            &mut self.fork_mgr,
        ) {
            return StepResult {
                stopped_reason: Some(StopReason::Error(format!("{}", e))),
                instruction_executed: Some(instr),
            };
        }

        self.cycle_count += 1;

        // 8. Dispatch pending traps (mirrors cqam-run runner logic).
        self.dispatch_pending_traps();

        // 9. Check for halt.
        if self.ctx.psw.trap_halt {
            return StepResult {
                stopped_reason: Some(StopReason::Halted),
                instruction_executed: Some(instr),
            };
        }

        // 10. Check watchpoints.
        let triggered = self.watchpoints.check(&self.prev_snapshot, &self.ctx);
        if !triggered.is_empty() {
            return StepResult {
                stopped_reason: Some(StopReason::Watchpoint(triggered)),
                instruction_executed: Some(instr),
            };
        }

        StepResult {
            stopped_reason: None,
            instruction_executed: Some(instr),
        }
    }

    /// Dispatch pending maskable traps through the ISR table.
    ///
    /// Mirrors the trap dispatch logic in cqam-run/src/runner.rs.
    fn dispatch_pending_traps(&mut self) {
        if self.ctx.psw.trap_arith {
            let trap = Trap::Maskable(MaskableTrap::Arithmetic);
            let handler = self.ctx.isr_table.get_handler(&trap);
            self.ctx.psw.trap_arith = false;
            isr::handle_trap(trap, &mut self.ctx, handler, self.enable_interrupts);
        }

        if self.ctx.psw.int_quantum_err {
            let trap = Trap::Maskable(MaskableTrap::QuantumError);
            let handler = self.ctx.isr_table.get_handler(&trap);
            self.ctx.psw.int_quantum_err = false;
            isr::handle_trap(trap, &mut self.ctx, handler, self.enable_interrupts);
        }

        if self.ctx.psw.int_sync_fail {
            let trap = Trap::Maskable(MaskableTrap::SyncFailure);
            let handler = self.ctx.isr_table.get_handler(&trap);
            self.ctx.psw.int_sync_fail = false;
            isr::handle_trap(trap, &mut self.ctx, handler, self.enable_interrupts);
        }
    }

    /// Reset the VM to initial state with a fresh program.
    ///
    /// Keeps breakpoints and watchpoints across restarts.
    pub fn restart(&mut self, program: Vec<Instruction>) {
        let mut ctx = ExecutionContext::new(program);

        // Re-apply config.
        if let Some(threshold) = self.sim_config.fidelity_threshold {
            ctx.config.min_purity = threshold;
        }
        if let Some(qubits) = self.sim_config.default_qubits {
            ctx.config.default_qubits = qubits;
        }
        ctx.config.force_density_matrix = self.sim_config.force_density_matrix;

        self.ctx = ctx;
        self.fork_mgr = ForkManager::new();
        self.prev_snapshot = RegisterSnapshot::capture(&self.ctx);
        self.cycle_count = 0;
        self.ecall_interceptor.clear();
        // Breakpoints and watchpoints are preserved across restarts.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::instruction::Instruction;
    use cqam_core::instruction::proc_id;
    use std::path::PathBuf;

    /// Helper: create a DebuggerEngine from a program slice with default config.
    fn make_engine(program: Vec<Instruction>) -> DebuggerEngine {
        DebuggerEngine::new(program, PathBuf::from("test.cqam"), SimConfig::default())
    }

    // ---------------------------------------------------------------
    // Construction
    // ---------------------------------------------------------------

    #[test]
    fn new_engine_initial_state() {
        let engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 42 },
            Instruction::Halt,
        ]);
        assert_eq!(engine.ctx.pc, 0, "PC should start at 0");
        assert_eq!(engine.cycle_count, 0, "cycle count should start at 0");
        assert_eq!(engine.ctx.program.len(), 2, "program length should match");
        assert!(engine.breakpoints.is_empty(), "no breakpoints initially");
        assert!(engine.watchpoints.is_empty(), "no watchpoints initially");
    }

    // ---------------------------------------------------------------
    // Stepping and cycle counting
    // ---------------------------------------------------------------

    #[test]
    fn step_advances_pc_and_cycle() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 42 },
            Instruction::ILdi { dst: 1, imm: 7 },
            Instruction::Halt,
        ]);

        let r1 = engine.step_one();
        assert!(r1.stopped_reason.is_none(), "first step should not stop");
        assert!(r1.instruction_executed.is_some(), "should report executed instruction");
        assert_eq!(engine.ctx.pc, 1, "PC should advance to 1 after first step");
        assert_eq!(engine.cycle_count, 1, "cycle count should be 1");

        let r2 = engine.step_one();
        assert!(r2.stopped_reason.is_none(), "second step should not stop");
        assert_eq!(engine.ctx.pc, 2, "PC should advance to 2");
        assert_eq!(engine.cycle_count, 2, "cycle count should be 2");
    }

    #[test]
    fn step_ildi_sets_register_value() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 3, imm: 99 },
            Instruction::Halt,
        ]);
        engine.step_one();
        assert_eq!(engine.ctx.iregs.regs[3], 99, "R3 should contain the loaded immediate");
    }

    // ---------------------------------------------------------------
    // Halt detection
    // ---------------------------------------------------------------

    #[test]
    fn halt_instruction_stops_engine() {
        let mut engine = make_engine(vec![Instruction::Halt]);

        let result = engine.step_one();
        assert!(
            matches!(result.stopped_reason, Some(StopReason::Halted)),
            "stepping a Halt instruction should produce StopReason::Halted"
        );
        assert!(result.instruction_executed.is_some(), "should still report the executed instruction");
    }

    // ---------------------------------------------------------------
    // End-of-program detection
    // ---------------------------------------------------------------

    #[test]
    fn end_of_program_when_pc_past_end() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 1 },
        ]);
        // First step executes ILdi, PC advances to 1 (past end).
        let r1 = engine.step_one();
        assert!(r1.stopped_reason.is_none(), "ILdi should succeed");

        // Second step: PC is out of bounds.
        let r2 = engine.step_one();
        assert!(
            matches!(r2.stopped_reason, Some(StopReason::EndOfProgram)),
            "should report EndOfProgram when PC is past program end"
        );
        assert!(r2.instruction_executed.is_none(), "no instruction executed when out of bounds");
    }

    // ---------------------------------------------------------------
    // Breakpoint hit detection
    // ---------------------------------------------------------------

    #[test]
    fn breakpoint_fires_at_target_pc() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::ILdi { dst: 1, imm: 2 },
            Instruction::Halt,
        ]);

        let bp_id = engine.breakpoints.add_address(1);

        // Step the first instruction -- should succeed (breakpoint is at PC 1, not PC 0).
        let r1 = engine.step_one();
        assert!(r1.stopped_reason.is_none(), "should not stop at PC 0");

        // Step at PC 1 -- should hit the breakpoint.
        let r2 = engine.step_one();
        assert!(
            matches!(r2.stopped_reason, Some(StopReason::Breakpoint(id)) if id == bp_id),
            "should hit breakpoint at PC 1"
        );
        // Breakpoint fires BEFORE executing, so no instruction_executed.
        assert!(r2.instruction_executed.is_none(), "breakpoint stops before execution");
        // PC should NOT have advanced -- the instruction at PC 1 was not executed.
        assert_eq!(engine.ctx.pc, 1, "PC should remain at 1 (breakpoint pre-empts execution)");
    }

    #[test]
    fn disabled_breakpoint_does_not_fire() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::ILdi { dst: 1, imm: 2 },
            Instruction::Halt,
        ]);

        let bp_id = engine.breakpoints.add_address(1);
        engine.breakpoints.disable(bp_id);

        // Step past PC 0 and into PC 1.
        engine.step_one();
        let r2 = engine.step_one();
        assert!(
            r2.stopped_reason.is_none(),
            "disabled breakpoint should not fire"
        );
    }

    #[test]
    fn breakpoint_hit_count_increments() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 0 },
            Instruction::Halt,
        ]);

        let bp_id = engine.breakpoints.add_address(0);

        // First hit.
        let r = engine.step_one();
        assert!(matches!(r.stopped_reason, Some(StopReason::Breakpoint(_))));
        assert_eq!(engine.breakpoints.get(bp_id).unwrap().hit_count, 1);
    }

    // ---------------------------------------------------------------
    // Watchpoint triggering
    // ---------------------------------------------------------------

    #[test]
    fn watchpoint_triggers_on_register_change() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 42 },
            Instruction::Halt,
        ]);

        let wp = watchpoint::Watchpoint::parse("R0").unwrap();
        engine.watchpoints.add(wp);

        let result = engine.step_one();
        assert!(
            matches!(result.stopped_reason, Some(StopReason::Watchpoint(ref regs)) if regs.contains(&"R0".to_string())),
            "watchpoint should trigger when R0 changes from 0 to 42"
        );
    }

    #[test]
    fn watchpoint_does_not_trigger_when_value_unchanged() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 0 }, // R0 stays 0
            Instruction::Halt,
        ]);

        let wp = watchpoint::Watchpoint::parse("R0").unwrap();
        engine.watchpoints.add(wp);

        let result = engine.step_one();
        // ILdi sets R0 to 0, but R0 was already 0, so no change.
        assert!(
            result.stopped_reason.is_none(),
            "watchpoint should not trigger when value does not change"
        );
    }

    // ---------------------------------------------------------------
    // Max cycles limit
    // ---------------------------------------------------------------

    #[test]
    fn max_cycles_stops_execution() {
        let mut config = SimConfig::default();
        config.max_cycles = Some(2);
        let mut engine = DebuggerEngine::new(
            vec![
                Instruction::ILdi { dst: 0, imm: 1 },
                Instruction::ILdi { dst: 1, imm: 2 },
                Instruction::ILdi { dst: 2, imm: 3 },
                Instruction::Halt,
            ],
            PathBuf::from("test.cqam"),
            config,
        );

        // Step 1: ok (cycle 0 -> 1).
        let r1 = engine.step_one();
        assert!(r1.stopped_reason.is_none());

        // Step 2: ok (cycle 1 -> 2).
        let r2 = engine.step_one();
        assert!(r2.stopped_reason.is_none());

        // Step 3: cycle_count == max_cycles (2), should stop.
        let r3 = engine.step_one();
        assert!(
            matches!(r3.stopped_reason, Some(StopReason::MaxCycles)),
            "should report MaxCycles when cycle limit is reached"
        );
        assert!(r3.instruction_executed.is_none(), "no instruction executed when max cycles hit");
    }

    // ---------------------------------------------------------------
    // ECALL interception
    // ---------------------------------------------------------------

    #[test]
    fn ecall_print_int_captured_in_interceptor() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 77 },
            Instruction::Ecall { proc_id: proc_id::PRINT_INT },
            Instruction::Halt,
        ]);

        // Step ILdi -- sets R0 = 77.
        engine.step_one();
        assert!(engine.ecall_interceptor.buffer.is_empty(), "no ECALL output yet");

        // Step ECALL PRINT_INT -- should capture "77" in interceptor.
        let result = engine.step_one();
        assert!(result.stopped_reason.is_none(), "ECALL should not stop execution");
        assert_eq!(engine.ecall_interceptor.buffer.len(), 1, "should have one captured line");
        assert_eq!(engine.ecall_interceptor.buffer[0].text, "77");
        assert_eq!(engine.ecall_interceptor.buffer[0].source, crate::ecall::OutputSource::Ecall);
    }

    #[test]
    fn ecall_advances_pc_and_cycle() {
        let mut engine = make_engine(vec![
            Instruction::Ecall { proc_id: proc_id::PRINT_INT },
            Instruction::Halt,
        ]);

        engine.step_one();
        assert_eq!(engine.ctx.pc, 1, "ECALL should advance PC");
        assert_eq!(engine.cycle_count, 1, "ECALL should increment cycle count");
    }

    // ---------------------------------------------------------------
    // Restart preserves breakpoints/watchpoints
    // ---------------------------------------------------------------

    #[test]
    fn restart_preserves_breakpoints_and_watchpoints() {
        let mut engine = make_engine(vec![
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::Halt,
        ]);

        engine.breakpoints.add_address(1);
        let wp = watchpoint::Watchpoint::parse("R0").unwrap();
        engine.watchpoints.add(wp);

        // Step to advance state.
        engine.step_one();
        assert_eq!(engine.cycle_count, 1);

        // Restart.
        engine.restart(vec![
            Instruction::ILdi { dst: 0, imm: 99 },
            Instruction::Halt,
        ]);

        assert_eq!(engine.ctx.pc, 0, "PC should reset to 0");
        assert_eq!(engine.cycle_count, 0, "cycle count should reset to 0");
        assert_eq!(engine.breakpoints.len(), 1, "breakpoints should be preserved");
        assert_eq!(engine.watchpoints.len(), 1, "watchpoints should be preserved");
    }
}
