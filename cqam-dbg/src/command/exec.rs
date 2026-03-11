//! Command execution: dispatches parsed commands to the debugger engine.
//!
//! Each command variant is mapped to the appropriate engine or app-state
//! mutation. Messages and errors are reported through the output buffer.

use super::{
    Command, DeleteTarget, FocusTarget, InfoSubcommand, PrintTarget, RunTarget, UnwatchTarget,
};
use crate::app::{AppState, ExecState, PaneFocus};
use crate::engine::breakpoint::InstrClass;
use crate::engine::condition::Condition;
use crate::engine::watchpoint::Watchpoint;
use crate::format::register::{format_complex, format_float, format_hybrid, format_int};

/// Execute a parsed command, mutating the application state.
///
/// Returns Ok(()) on success, or Err(message) on failure.
pub fn execute_command(cmd: &Command, app: &mut AppState) -> Result<(), String> {
    match cmd {
        Command::Step(n) => {
            if is_terminated(&app.execution_state) {
                return Err("Program has terminated. Use 'restart' to reset.".into());
            }
            app.do_step_n(*n);
            Ok(())
        }
        Command::Next => {
            if is_terminated(&app.execution_state) {
                return Err("Program has terminated. Use 'restart' to reset.".into());
            }
            exec_next(app);
            Ok(())
        }
        Command::Continue | Command::Run => {
            if is_terminated(&app.execution_state) {
                return Err("Program has terminated. Use 'restart' to reset.".into());
            }
            app.do_continue();
            Ok(())
        }
        Command::RunTo(target) => exec_run_to(app, target),
        Command::Finish => exec_finish(app),
        Command::BreakAddr(addr, cond_str) => exec_break_addr(app, *addr, cond_str.as_deref()),
        Command::BreakLabel(label, cond_str) => {
            exec_break_label(app, label, cond_str.as_deref())
        }
        Command::BreakClass(class_name) => exec_break_class(app, class_name),
        Command::Delete(target) => exec_delete(app, target),
        Command::Enable(id) => {
            if app.engine.breakpoints.enable(*id) {
                app.add_diagnostic(format!("Breakpoint {} enabled", id));
                Ok(())
            } else {
                Err(format!("No breakpoint with ID {}", id))
            }
        }
        Command::Disable(id) => {
            if app.engine.breakpoints.disable(*id) {
                app.add_diagnostic(format!("Breakpoint {} disabled", id));
                Ok(())
            } else {
                Err(format!("No breakpoint with ID {}", id))
            }
        }
        Command::Watch(reg) => exec_watch(app, reg),
        Command::Unwatch(target) => exec_unwatch(app, target),
        Command::Print(target) => exec_print(app, target),
        Command::Info(sub) => {
            exec_info(sub, app);
            Ok(())
        }
        Command::SetThreshold(val) => {
            app.display.threshold = *val;
            app.add_diagnostic(format!("Threshold set to {}", val));
            Ok(())
        }
        Command::SetTopK(val) => {
            app.display.topk = *val;
            app.add_diagnostic(format!("TopK set to {}", val));
            Ok(())
        }
        Command::SetQReg(idx) => {
            app.display.selected_qreg = *idx;
            app.add_diagnostic(format!("Selected Q register: Q{}", idx));
            Ok(())
        }
        Command::Focus(target) => {
            app.focus = match target {
                FocusTarget::Code => PaneFocus::Code,
                FocusTarget::State => PaneFocus::State,
                FocusTarget::Quantum => PaneFocus::Quantum,
                FocusTarget::Output => PaneFocus::Output,
            };
            Ok(())
        }
        Command::Load(path) => exec_load(app, path),
        Command::Restart => exec_restart(app),
        Command::Quit => {
            app.should_quit = true;
            Ok(())
        }
        Command::Help(topic) => {
            exec_help(topic, app);
            Ok(())
        }
        Command::Empty => Ok(()),
    }
}

fn is_terminated(state: &ExecState) -> bool {
    matches!(state, ExecState::Halted | ExecState::Error(_))
}

fn exec_next(app: &mut AppState) {
    let current_depth = app.engine.ctx.call_stack.len();
    let is_call = app.engine.ctx.pc < app.engine.ctx.program.len()
        && matches!(
            app.engine.ctx.program[app.engine.ctx.pc],
            cqam_core::instruction::Instruction::Call { .. }
        );

    if !is_call {
        app.do_step();
        return;
    }

    app.do_step();
    if !matches!(app.execution_state, ExecState::Stopped) {
        return;
    }

    loop {
        if app.engine.ctx.call_stack.len() <= current_depth {
            return;
        }
        let result = app.engine.step_one();
        app.drain_ecall_output();
        if let Some(reason) = result.stopped_reason {
            app.handle_stop_reason(reason);
            return;
        }
    }
}

fn exec_run_to(app: &mut AppState, target: &RunTarget) -> Result<(), String> {
    if is_terminated(&app.execution_state) {
        return Err("Program has terminated. Use 'restart' to reset.".into());
    }

    let target_pc = match target {
        RunTarget::Addr(addr) => *addr,
        RunTarget::Label(label) => match app.engine.ctx.labels.get(label) {
            Some(&addr) => addr,
            None => return Err(format!("Label '{}' not found", label)),
        },
    };

    let current_pc = app.engine.ctx.pc;
    if target_pc < current_pc {
        return Err(format!(
            "Target PC 0x{:04X} is behind current PC 0x{:04X}; use `restart` then `run to 0x{:04X}`",
            target_pc, current_pc, target_pc
        ));
    }

    if target_pc == current_pc {
        app.add_diagnostic(format!("Already at target PC 0x{:04X}", target_pc));
        return Ok(());
    }

    loop {
        if app.engine.ctx.pc == target_pc {
            app.add_diagnostic(format!("Reached target PC 0x{:04X}", target_pc));
            app.execution_state = ExecState::Stopped;
            return Ok(());
        }
        let result = app.engine.step_one();
        app.drain_ecall_output();
        if let Some(reason) = result.stopped_reason {
            app.handle_stop_reason(reason);
            return Ok(());
        }
    }
}

fn exec_finish(app: &mut AppState) -> Result<(), String> {
    if is_terminated(&app.execution_state) {
        return Err("Program has terminated. Use 'restart' to reset.".into());
    }

    let current_depth = app.engine.ctx.call_stack.len();
    if current_depth == 0 {
        return Err("Not inside a call frame. Use 'continue' instead.".into());
    }

    let target_depth = current_depth - 1;
    loop {
        if app.engine.ctx.call_stack.len() <= target_depth {
            app.add_diagnostic(format!("Returned from call, PC=0x{:04X}", app.engine.ctx.pc));
            app.execution_state = ExecState::Stopped;
            return Ok(());
        }
        let result = app.engine.step_one();
        app.drain_ecall_output();
        if let Some(reason) = result.stopped_reason {
            app.handle_stop_reason(reason);
            return Ok(());
        }
    }
}

fn exec_break_addr(app: &mut AppState, addr: usize, cond_str: Option<&str>) -> Result<(), String> {
    if addr >= app.engine.ctx.program.len() {
        return Err(format!(
            "Address 0x{:04X} is beyond program length ({})",
            addr, app.engine.ctx.program.len()
        ));
    }

    let id = if let Some(cond_s) = cond_str {
        let cond = Condition::parse(cond_s).map_err(|e| format!("Invalid condition: {}", e))?;
        app.engine.breakpoints.add_conditional(addr, cond)
    } else {
        app.engine.breakpoints.add_address(addr)
    };

    app.add_diagnostic(format!("Breakpoint {} set at 0x{:04X}", id, addr));
    Ok(())
}

fn exec_break_label(app: &mut AppState, label: &str, cond_str: Option<&str>) -> Result<(), String> {
    let addr = match app.engine.ctx.labels.get(label) {
        Some(&a) => a,
        None => return Err(format!("Label '{}' not found", label)),
    };

    let id = if let Some(cond_s) = cond_str {
        let cond = Condition::parse(cond_s).map_err(|e| format!("Invalid condition: {}", e))?;
        app.engine.breakpoints.add_conditional(addr, cond)
    } else {
        app.engine.breakpoints.add_label(label.to_string(), addr)
    };

    app.add_diagnostic(format!("Breakpoint {} set at {} (0x{:04X})", id, label, addr));
    Ok(())
}

fn exec_break_class(app: &mut AppState, class_name: &str) -> Result<(), String> {
    let class = InstrClass::from_name(class_name).ok_or_else(|| {
        format!(
            "Unknown instruction class '{}'. Available: quantum, hybrid, branch, memory, ecall, float, complex",
            class_name
        )
    })?;
    let id = app.engine.breakpoints.add_class(class);
    app.add_diagnostic(format!("Breakpoint {} set on class '{}'", id, class_name));
    Ok(())
}

fn exec_delete(app: &mut AppState, target: &DeleteTarget) -> Result<(), String> {
    match target {
        DeleteTarget::Id(id) => {
            if app.engine.breakpoints.remove(*id) {
                app.add_diagnostic(format!("Breakpoint {} deleted", id));
                Ok(())
            } else {
                Err(format!("No breakpoint with ID {}", id))
            }
        }
        DeleteTarget::All => {
            let count = app.engine.breakpoints.len();
            app.engine.breakpoints.remove_all();
            app.add_diagnostic(format!("Deleted {} breakpoint(s)", count));
            Ok(())
        }
    }
}

fn exec_watch(app: &mut AppState, reg_name: &str) -> Result<(), String> {
    let wp = Watchpoint::parse(reg_name).ok_or_else(|| {
        format!("Invalid register '{}'. Expected R0-R15, F0-F15, or Z0-Z15", reg_name)
    })?;
    let name = wp.name();
    if app.engine.watchpoints.add(wp) {
        app.add_diagnostic(format!("Watchpoint set on {}", name));
        Ok(())
    } else {
        Err(format!("Watchpoint on {} already exists", name))
    }
}

fn exec_unwatch(app: &mut AppState, target: &UnwatchTarget) -> Result<(), String> {
    match target {
        UnwatchTarget::Register(reg_name) => {
            if app.engine.watchpoints.remove(reg_name) {
                app.add_diagnostic(format!("Watchpoint on {} removed", reg_name));
                Ok(())
            } else {
                Err(format!("No watchpoint on {}", reg_name))
            }
        }
        UnwatchTarget::All => {
            let count = app.engine.watchpoints.len();
            app.engine.watchpoints.remove_all();
            app.add_diagnostic(format!("Removed {} watchpoint(s)", count));
            Ok(())
        }
    }
}

fn exec_print(app: &mut AppState, target: &PrintTarget) -> Result<(), String> {
    let text = match target {
        PrintTarget::Register(name) => format_register_value(&app.engine.ctx, name)?,
        PrintTarget::CmemAddr(addr) => {
            let val = app.engine.ctx.cmem.load(*addr);
            format!("CMEM[0x{:04X}] = {} (0x{:016X})", addr, val, val as u64)
        }
        PrintTarget::CmemRange(lo, hi) => {
            if lo > hi {
                return Err(format!("Invalid range: 0x{:04X}..0x{:04X}", lo, hi));
            }
            let mut lines = Vec::new();
            let end = (*hi).min(lo + 64);
            for addr in *lo..=end {
                let val = app.engine.ctx.cmem.load(addr);
                if val != 0 {
                    lines.push(format!("  [{:04X}] = {}", addr, val));
                }
            }
            if lines.is_empty() {
                format!("CMEM[0x{:04X}..0x{:04X}]: all zero", lo, hi)
            } else {
                format!("CMEM[0x{:04X}..0x{:04X}]:\n{}", lo, hi, lines.join("\n"))
            }
        }
    };
    app.add_diagnostic(text);
    Ok(())
}

fn format_register_value(
    ctx: &cqam_vm::context::ExecutionContext,
    name: &str,
) -> Result<String, String> {
    let upper = name.to_uppercase();
    if let Some(rest) = upper.strip_prefix('R') {
        let idx: usize = rest.parse().map_err(|_| format!("Invalid register: {}", name))?;
        if idx >= 16 { return Err(format!("Register index out of range: {}", idx)); }
        let val = ctx.iregs.regs[idx];
        return Ok(format!("R{} = {} (0x{:016X})", idx, format_int(val), val as u64));
    }
    if let Some(rest) = upper.strip_prefix('F') {
        let idx: usize = rest.parse().map_err(|_| format!("Invalid register: {}", name))?;
        if idx >= 16 { return Err(format!("Register index out of range: {}", idx)); }
        return Ok(format!("F{} = {}", idx, format_float(ctx.fregs.regs[idx])));
    }
    if let Some(rest) = upper.strip_prefix('Z') {
        let idx: usize = rest.parse().map_err(|_| format!("Invalid register: {}", name))?;
        if idx >= 16 { return Err(format!("Register index out of range: {}", idx)); }
        let (re, im) = ctx.zregs.regs[idx];
        return Ok(format!("Z{} = {}", idx, format_complex(re, im)));
    }
    if let Some(rest) = upper.strip_prefix('H') {
        let idx: usize = rest.parse().map_err(|_| format!("Invalid register: {}", name))?;
        if idx >= 8 { return Err(format!("Register index out of range: {}", idx)); }
        return Ok(format!("H{} = {}", idx, format_hybrid(&ctx.hregs.regs[idx])));
    }
    if let Some(rest) = upper.strip_prefix('Q') {
        let idx: usize = rest.parse().map_err(|_| format!("Invalid register: {}", name))?;
        if idx >= 8 { return Err(format!("Register index out of range: {}", idx)); }
        return Ok(match &ctx.qregs[idx] {
            Some(_) => format!("Q{}: allocated (see QUANTUM pane)", idx),
            None => format!("Q{}: not allocated", idx),
        });
    }
    Err(format!("Unknown register name: {}", name))
}

fn exec_info(sub: &InfoSubcommand, app: &mut AppState) {
    match sub {
        InfoSubcommand::Breakpoints => {
            if app.engine.breakpoints.is_empty() {
                app.add_diagnostic("No breakpoints set.".to_string());
            } else {
                app.add_diagnostic(format!("{} breakpoint(s):", app.engine.breakpoints.len()));
                let descs: Vec<String> = app.engine.breakpoints.iter().map(|bp| bp.describe()).collect();
                for desc in descs {
                    app.add_diagnostic(format!("  {}", desc));
                }
            }
        }
        InfoSubcommand::Watchpoints => {
            let wp_msg = if app.engine.watchpoints.is_empty() {
                "No watchpoints set.".to_string()
            } else {
                let names: Vec<String> = app.engine.watchpoints.iter().map(|wp| wp.name()).collect();
                format!("Watchpoints: {}", names.join(", "))
            };
            app.add_diagnostic(wp_msg);
        }
        InfoSubcommand::Registers(file) => {
            info_registers(file, app);
        }
        InfoSubcommand::Quantum(idx) => {
            let qi = idx.unwrap_or(app.display.selected_qreg) as usize;
            if qi >= 8 {
                app.add_diagnostic(format!("Q register index {} out of range", qi));
                return;
            }
            match &app.engine.ctx.qregs[qi] {
                None => app.add_diagnostic(format!("Q{}: not allocated", qi)),
                Some(_) => app.add_diagnostic(format!("Q{}: allocated (see QUANTUM pane)", qi)),
            }
        }
        InfoSubcommand::Psw => {
            let psw_line = {
                let psw = &app.engine.ctx.psw;
                format!(
                    "PSW: ZF={} NF={} OF={} PF={} QF={} SF={} EF={} HF={}",
                    psw.zf as u8, psw.nf as u8, psw.of as u8, psw.pf as u8,
                    psw.qf as u8, psw.sf as u8, psw.ef as u8, psw.hf as u8,
                )
            };
            app.add_diagnostic(psw_line);
            let trap_line = {
                let psw = &app.engine.ctx.psw;
                format!(
                    "Traps: halt={} arith={} qerr={} sync={}",
                    psw.trap_halt as u8, psw.trap_arith as u8,
                    psw.int_quantum_err as u8, psw.int_sync_fail as u8,
                )
            };
            app.add_diagnostic(trap_line);
        }
        InfoSubcommand::Resources => {
            let res_line = {
                let rt = &app.engine.ctx.resource_tracker;
                format!(
                    "Resources: T={} S={} Sup={:.1} Ent={:.1} Int={:.1}",
                    rt.total_time, rt.total_space,
                    rt.total_superposition, rt.total_entanglement, rt.total_interference,
                )
            };
            app.add_diagnostic(res_line);
            let cyc = app.engine.cycle_count;
            app.add_diagnostic(format!("Cycles: {}", cyc));
        }
        InfoSubcommand::Stack => {
            let stack_lines: Vec<String> = {
                let stack = &app.engine.ctx.call_stack;
                if stack.is_empty() {
                    vec!["Call stack: empty".to_string()]
                } else {
                    let mut lines = vec![format!("Call stack (depth {}):", stack.len())];
                    for (i, addr) in stack.iter().enumerate() {
                        lines.push(format!("  [{}] 0x{:04X}", i, addr));
                    }
                    lines
                }
            };
            for line in stack_lines {
                app.add_diagnostic(line);
            }
        }
        InfoSubcommand::Labels => {
            let label_lines: Vec<String> = {
                let labels = &app.engine.ctx.labels;
                if labels.is_empty() {
                    vec!["No labels in program.".to_string()]
                } else {
                    let mut sorted: Vec<(&String, &usize)> = labels.iter().collect();
                    sorted.sort_by_key(|(_, addr)| **addr);
                    let mut lines = vec![format!("{} label(s):", sorted.len())];
                    for (name, addr) in &sorted {
                        lines.push(format!("  0x{:04X}  {}", addr, name));
                    }
                    lines
                }
            };
            for line in label_lines {
                app.add_diagnostic(line);
            }
        }
        InfoSubcommand::Program => {
            let prog_line = {
                let path = app.engine.program_path.display().to_string();
                let count = app.engine.ctx.program.len();
                let pc = app.engine.ctx.pc;
                let nlabels = app.engine.ctx.labels.len();
                let cycles = app.engine.cycle_count;
                format!(
                    "Program: '{}', {} instructions, {} labels, PC=0x{:04X}, Cycles={}",
                    path, count, nlabels, pc, cycles,
                )
            };
            app.add_diagnostic(prog_line);
        }
    }
}

fn info_registers(file: &Option<String>, app: &mut AppState) {
    let show_nonzero = file.is_none();
    let filter = file.as_ref().map(|s| s.to_uppercase()).unwrap_or_default();

    // Collect all output lines first to avoid borrow conflicts.
    let mut lines: Vec<String> = Vec::new();
    {
        let ctx = &app.engine.ctx;
        if show_nonzero || filter == "R" {
            let mut parts = Vec::new();
            for i in 0..16 {
                let v = ctx.iregs.regs[i];
                if !show_nonzero || v != 0 {
                    parts.push(format!("R{}={}", i, format_int(v)));
                }
            }
            if parts.is_empty() && show_nonzero {
                lines.push("R-file: (all zero)".to_string());
            } else if !parts.is_empty() {
                lines.push(format!("R-file: {}", parts.join("  ")));
            }
        }
        if show_nonzero || filter == "F" {
            let mut parts = Vec::new();
            for i in 0..16 {
                let v = ctx.fregs.regs[i];
                if !show_nonzero || v != 0.0 {
                    parts.push(format!("F{}={}", i, format_float(v)));
                }
            }
            if parts.is_empty() && show_nonzero {
                lines.push("F-file: (all zero)".to_string());
            } else if !parts.is_empty() {
                lines.push(format!("F-file: {}", parts.join("  ")));
            }
        }
        if show_nonzero || filter == "Z" {
            let mut parts = Vec::new();
            for i in 0..16 {
                let (re, im) = ctx.zregs.regs[i];
                if !show_nonzero || re != 0.0 || im != 0.0 {
                    parts.push(format!("Z{}={}", i, format_complex(re, im)));
                }
            }
            if parts.is_empty() && show_nonzero {
                lines.push("Z-file: (all zero)".to_string());
            } else if !parts.is_empty() {
                lines.push(format!("Z-file: {}", parts.join("  ")));
            }
        }
        if show_nonzero || filter == "H" {
            let mut parts = Vec::new();
            for i in 0..8 {
                parts.push(format!("H{}={}", i, format_hybrid(&ctx.hregs.regs[i])));
            }
            lines.push(format!("H-file: {}", parts.join("  ")));
        }
        if show_nonzero || filter == "Q" {
            let mut parts = Vec::new();
            for i in 0..8 {
                let desc = match &ctx.qregs[i] {
                    Some(_) => format!("Q{}=active", i),
                    None => format!("Q{}=---", i),
                };
                parts.push(desc);
            }
            lines.push(format!("Q-file: {}", parts.join("  ")));
        }
    }
    for line in lines {
        app.add_diagnostic(line);
    }
}

/// Copy the SimConfig fields (SimConfig does not derive Clone).
fn copy_sim_config(c: &cqam_run::simconfig::SimConfig) -> cqam_run::simconfig::SimConfig {
    cqam_run::simconfig::SimConfig {
        fidelity_threshold: c.fidelity_threshold,
        max_cycles: c.max_cycles,
        enable_interrupts: c.enable_interrupts,
        default_qubits: c.default_qubits,
        force_density_matrix: c.force_density_matrix,
    }
}

fn exec_load(app: &mut AppState, path: &str) -> Result<(), String> {
    match cqam_run::loader::load_program(path) {
        Ok(parsed) => {
            let count = parsed.instructions.len();
            let breakpoints = app.engine.breakpoints.clone();
            let watchpoints = app.engine.watchpoints.clone();
            let sim_config = copy_sim_config(&app.engine.sim_config);
            app.engine = crate::engine::DebuggerEngine::new(
                parsed.instructions,
                std::path::PathBuf::from(path),
                sim_config,
            );
            app.engine.breakpoints = breakpoints;
            app.engine.watchpoints = watchpoints;
            app.execution_state = ExecState::Stopped;
            app.code_scroll = 0;
            app.add_diagnostic(format!("Loaded {} instructions from '{}'", count, path));
            Ok(())
        }
        Err(e) => Err(format!("Failed to load '{}': {}", path, e)),
    }
}

fn exec_restart(app: &mut AppState) -> Result<(), String> {
    let path = app.engine.program_path.to_string_lossy().to_string();
    match cqam_run::loader::load_program(&path) {
        Ok(parsed) => {
            app.engine.restart(parsed.instructions);
            app.execution_state = ExecState::Stopped;
            app.code_scroll = 0;
            app.add_diagnostic("** Program restarted **".to_string());
            Ok(())
        }
        Err(e) => Err(format!("Failed to reload program: {}", e)),
    }
}

fn exec_help(topic: &Option<String>, app: &mut AppState) {
    match topic.as_deref() {
        None => {
            let lines = [
                "Commands:",
                "  step [N]          Step N instructions (default 1)",
                "  next              Step over CALL (execute until return)",
                "  continue          Run until breakpoint or halt",
                "  run [to ADDR|LBL] Synonym for continue / run to target",
                "  finish            Run until current call returns",
                "  break ADDR [if C] Set breakpoint at address",
                "  break LABEL       Set breakpoint at label",
                "  break class NAME  Break on instruction class",
                "  delete N|all      Delete breakpoint(s)",
                "  enable/disable N  Enable/disable breakpoint",
                "  watch REG         Watch register for changes",
                "  unwatch REG|all   Remove watchpoint(s)",
                "  print REG|CMEM[]  Print register or memory value",
                "  info SUB          Show info (breakpoints, registers, psw, ...)",
                "  set KEY VAL       Set display option",
                "  focus PANE        Focus a pane (code/state/quantum/output)",
                "  load FILE         Load a new program",
                "  restart           Restart program",
                "  quit              Exit debugger",
                "  help [CMD]        Show help",
                "",
                "Keys: F5=continue F10=step F9=breakpoint Tab=focus Ctrl+C=quit",
                "Commands are case-insensitive. Unambiguous prefixes accepted.",
            ];
            for line in &lines {
                app.add_diagnostic(line.to_string());
            }
        }
        Some(cmd) => {
            let help_text = match cmd.to_lowercase().as_str() {
                "step" | "s" => "step [N]  Execute N instructions (default 1). Aliases: s",
                "next" | "n" => "next  Step over CALL. Aliases: n",
                "continue" | "c" => "continue  Run until breakpoint/halt. Aliases: c",
                "run" => "run  Synonym for continue.\nrun to ADDR|LABEL  Run to target.",
                "finish" | "fin" => "finish  Run until current call returns. Aliases: fin",
                "break" | "b" => "break ADDR [if COND]  Set breakpoint.\nbreak LABEL [if COND]  Breakpoint at label.\nbreak class NAME  Break on class. Aliases: b",
                "delete" | "del" => "delete N  Delete breakpoint.\ndelete all  Delete all. Aliases: del",
                "enable" => "enable N  Enable breakpoint N.",
                "disable" => "disable N  Disable breakpoint N.",
                "watch" => "watch REG  Watch register (R0-R15, F0-F15, Z0-Z15).",
                "unwatch" => "unwatch REG  Remove watchpoint.\nunwatch all  Remove all.",
                "print" | "p" => "print REG  Print register.\nprint CMEM[ADDR]  Print memory.\nprint CMEM[LO..HI]  Print range. Aliases: p",
                "info" | "i" => "info breakpoints|watchpoints|registers [R|F|Z|H|Q]|quantum [QN]|psw|resources|stack|labels|program",
                "set" => "set threshold FLOAT|topk N|qreg QN",
                "focus" => "focus code|state|quantum|output",
                "load" => "load FILE  Load .cqam or .cqb file.",
                "restart" => "restart  Reset VM, reload program.",
                "quit" | "q" | "exit" => "quit  Exit debugger. Aliases: q, exit",
                "help" | "h" => "help [COMMAND]  Show help. Aliases: h",
                _ => {
                    app.add_diagnostic(format!("No help for '{}'", cmd));
                    return;
                }
            };
            for line in help_text.lines() {
                app.add_diagnostic(line.to_string());
            }
        }
    }
}
