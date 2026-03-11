//! Application state and top-level event loop for the debugger TUI.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::command::parse::parse_command;
use crate::command::exec::execute_command;
use crate::ecall::{OutputLine, OutputSource};
use crate::engine::DebuggerEngine;
use crate::engine::StopReason;

/// Which pane currently has visual focus (for scrolling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFocus {
    Code,
    State,
    Quantum,
    Output,
    Cmd,
}

/// Execution state of the debugger.
#[derive(Debug, Clone)]
pub enum ExecState {
    Stopped,
    Running,
    Halted,
    Error(String),
}

/// Display settings for the QUANTUM pane.
#[derive(Debug, Clone)]
pub struct DisplaySettings {
    /// Maximum entries to show in the quantum display.
    pub topk: usize,
    /// Minimum probability threshold for quantum display.
    pub threshold: f64,
    /// Selected Q register index for the QUANTUM pane.
    pub selected_qreg: u8,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            topk: 16,
            threshold: 0.01,
            selected_qreg: 0,
        }
    }
}

/// Root application state for the debugger TUI.
pub struct AppState {
    /// The debugger engine (owns the VM context).
    pub engine: DebuggerEngine,
    /// Which pane has visual focus.
    pub focus: PaneFocus,
    /// CODE pane scroll offset.
    pub code_scroll: usize,
    /// STATE pane scroll offset.
    pub state_scroll: usize,
    /// QUANTUM pane scroll offset.
    pub quantum_scroll: usize,
    /// OUTPUT pane scroll offset.
    pub output_scroll: usize,
    /// Current command line text.
    pub command_line: String,
    /// Command history.
    pub command_history: Vec<String>,
    /// Current position in command history (for Up/Down navigation).
    pub history_cursor: Option<usize>,
    /// Output buffer (ECALL output + debugger diagnostics).
    pub output_buffer: Vec<OutputLine>,
    /// Display settings.
    pub display: DisplaySettings,
    /// Current execution state.
    pub execution_state: ExecState,
    /// Whether the application should quit.
    pub should_quit: bool,
    /// Number of instructions to execute per batch in Running mode.
    pub run_batch_size: usize,
}

impl AppState {
    /// Create a new AppState from a debugger engine.
    pub fn new(engine: DebuggerEngine) -> Self {
        Self {
            engine,
            focus: PaneFocus::Cmd,
            code_scroll: 0,
            state_scroll: 0,
            quantum_scroll: 0,
            output_scroll: 0,
            command_line: String::new(),
            command_history: Vec::new(),
            history_cursor: None,
            output_buffer: Vec::new(),
            display: DisplaySettings::default(),
            execution_state: ExecState::Stopped,
            should_quit: false,
            run_batch_size: 100,
        }
    }

    /// Add a debugger diagnostic message to the output buffer.
    pub fn add_diagnostic(&mut self, text: String) {
        self.output_buffer.push(OutputLine {
            cycle: self.engine.cycle_count,
            source: OutputSource::Debugger,
            text,
        });
    }

    /// Add an error message to the output buffer.
    pub fn add_error(&mut self, text: String) {
        self.output_buffer.push(OutputLine {
            cycle: self.engine.cycle_count,
            source: OutputSource::Error,
            text,
        });
    }

    /// Drain any new output from the ECALL interceptor into the output buffer.
    pub fn drain_ecall_output(&mut self) {
        let new_output: Vec<OutputLine> = self.engine.ecall_interceptor.buffer.drain(..).collect();
        self.output_buffer.extend(new_output);
    }

    /// Cycle focus to the next pane (Tab).
    pub fn cycle_focus_forward(&mut self) {
        self.focus = match self.focus {
            PaneFocus::Code => PaneFocus::State,
            PaneFocus::State => PaneFocus::Quantum,
            PaneFocus::Quantum => PaneFocus::Output,
            PaneFocus::Output => PaneFocus::Code,
            PaneFocus::Cmd => PaneFocus::Code,
        };
    }

    /// Cycle focus to the previous pane (Shift+Tab).
    pub fn cycle_focus_backward(&mut self) {
        self.focus = match self.focus {
            PaneFocus::Code => PaneFocus::Output,
            PaneFocus::State => PaneFocus::Code,
            PaneFocus::Quantum => PaneFocus::State,
            PaneFocus::Output => PaneFocus::Quantum,
            PaneFocus::Cmd => PaneFocus::Output,
        };
    }

    /// Return focus to the CMD pane.
    pub fn focus_cmd(&mut self) {
        self.focus = PaneFocus::Cmd;
    }

    /// Scroll the currently focused pane up by `n` lines.
    pub fn scroll_up(&mut self, n: usize) {
        match self.focus {
            PaneFocus::Code => self.code_scroll = self.code_scroll.saturating_sub(n),
            PaneFocus::State => self.state_scroll = self.state_scroll.saturating_sub(n),
            PaneFocus::Quantum => self.quantum_scroll = self.quantum_scroll.saturating_sub(n),
            PaneFocus::Output => self.output_scroll = self.output_scroll.saturating_sub(n),
            PaneFocus::Cmd => {
                // Navigate command history upward.
                if !self.command_history.is_empty() {
                    self.history_cursor = Some(match self.history_cursor {
                        None => self.command_history.len() - 1,
                        Some(c) => c.saturating_sub(1),
                    });
                    if let Some(c) = self.history_cursor {
                        self.command_line = self.command_history[c].clone();
                    }
                }
            }
        }
    }

    /// Scroll the currently focused pane down by `n` lines.
    pub fn scroll_down(&mut self, n: usize) {
        match self.focus {
            PaneFocus::Code => {
                let max = self.engine.ctx.program.len().saturating_sub(1);
                self.code_scroll = (self.code_scroll + n).min(max);
            }
            PaneFocus::State => self.state_scroll += n,
            PaneFocus::Quantum => self.quantum_scroll += n,
            PaneFocus::Output => {
                let max = self.output_buffer.len().saturating_sub(1);
                self.output_scroll = (self.output_scroll + n).min(max);
            }
            PaneFocus::Cmd => {
                // Navigate command history downward.
                if let Some(c) = self.history_cursor {
                    if c + 1 >= self.command_history.len() {
                        self.history_cursor = None;
                        self.command_line.clear();
                    } else {
                        self.history_cursor = Some(c + 1);
                        self.command_line = self.command_history[c + 1].clone();
                    }
                }
            }
        }
    }

    /// Execute a single step and handle the result.
    pub fn do_step(&mut self) {
        let result = self.engine.step_one();
        self.drain_ecall_output();

        if let Some(reason) = result.stopped_reason {
            self.handle_stop_reason(reason);
        }
    }

    /// Execute N steps, stopping early on breakpoint/halt/error.
    pub fn do_step_n(&mut self, n: usize) {
        for _ in 0..n {
            let result = self.engine.step_one();
            self.drain_ecall_output();

            if let Some(reason) = result.stopped_reason {
                self.handle_stop_reason(reason);
                return;
            }
        }
    }

    /// Enter "Running" mode (continue execution).
    pub fn do_continue(&mut self) {
        self.execution_state = ExecState::Running;
    }

    /// Handle a stop reason from the engine.
    pub fn handle_stop_reason(&mut self, reason: StopReason) {
        match reason {
            StopReason::Breakpoint(id) => {
                let pc = self.engine.ctx.pc;
                self.add_diagnostic(format!(
                    "** Breakpoint {} hit at PC 0x{:04X} **",
                    id, pc
                ));
                self.execution_state = ExecState::Stopped;
            }
            StopReason::Watchpoint(regs) => {
                self.add_diagnostic(format!(
                    "** Watchpoint triggered: {} changed **",
                    regs.join(", ")
                ));
                self.execution_state = ExecState::Stopped;
            }
            StopReason::Halted => {
                self.add_diagnostic("** Program halted **".to_string());
                self.execution_state = ExecState::Halted;
            }
            StopReason::Error(msg) => {
                self.add_error(format!("** Runtime error: {} **", msg));
                self.execution_state = ExecState::Error(msg);
            }
            StopReason::EndOfProgram => {
                self.add_diagnostic("** Reached end of program **".to_string());
                self.execution_state = ExecState::Halted;
            }
            StopReason::MaxCycles => {
                self.add_diagnostic(format!(
                    "** Max cycle limit ({}) reached **",
                    self.engine.max_cycles
                ));
                self.execution_state = ExecState::Halted;
            }
        }
    }

    /// Execute a batch of instructions in Running mode.
    /// Returns true if execution should continue, false if it stopped.
    fn run_batch(&mut self) -> bool {
        for _ in 0..self.run_batch_size {
            let result = self.engine.step_one();
            self.drain_ecall_output();

            if let Some(reason) = result.stopped_reason {
                self.handle_stop_reason(reason);
                return false;
            }
        }
        true
    }

    /// Submit the current command line: parse and execute.
    fn submit_command(&mut self) {
        let input = self.command_line.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Save to history.
        self.command_history.push(input.clone());
        self.history_cursor = None;
        self.command_line.clear();

        // Parse.
        let cmd = match parse_command(&input) {
            Ok(cmd) => cmd,
            Err(e) => {
                self.add_error(e);
                return;
            }
        };

        // Execute.
        if let Err(e) = execute_command(&cmd, self) {
            self.add_error(e);
        }
    }

    /// Toggle a breakpoint at the CODE pane cursor (current PC or focused line).
    fn toggle_breakpoint_at_cursor(&mut self) {
        // Use the current PC as the target address for simplicity.
        let pc = self.engine.ctx.pc;
        if let Some(bp) = self.engine.breakpoints.has_breakpoint_at(pc) {
            let id = bp.id;
            self.engine.breakpoints.remove(id);
            self.add_diagnostic(format!("Breakpoint removed at PC 0x{:04X}", pc));
        } else {
            let id = self.engine.breakpoints.add_address(pc);
            self.add_diagnostic(format!("Breakpoint {} set at PC 0x{:04X}", id, pc));
        }
    }

    /// Run the main event loop.
    ///
    /// Returns Ok(Some(message)) with a final message to print, or Ok(None) on
    /// clean exit. Returns Err on I/O errors.
    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<Option<String>> {
        loop {
            // 1. RENDER
            terminal.draw(|frame| {
                crate::ui::render(frame, self);
            })?;

            // 2. Check quit flag.
            if self.should_quit {
                return Ok(Some(format!(
                    "cqam-dbg: exited after {} cycles.",
                    self.engine.cycle_count
                )));
            }

            // 3. DISPATCH based on execution state.
            match self.execution_state.clone() {
                ExecState::Running => {
                    // In Running mode: poll for Ctrl+C with short timeout,
                    // then execute a batch of instructions.
                    if event::poll(Duration::from_millis(1))? {
                        if let Event::Key(key) = event::read()? {
                            if key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL)
                            {
                                // Ctrl+C: interrupt execution.
                                self.execution_state = ExecState::Stopped;
                                self.add_diagnostic(format!(
                                    "** Interrupted at PC 0x{:04X} **",
                                    self.engine.ctx.pc
                                ));
                                continue;
                            }
                        }
                    }
                    // Execute a batch.
                    self.run_batch();
                }
                ExecState::Stopped => {
                    // In Stopped mode: wait for keyboard events with timeout.
                    if event::poll(Duration::from_millis(50))? {
                        if let Event::Key(key) = event::read()? {
                            self.handle_key_stopped(key);
                        }
                    }
                }
                ExecState::Halted | ExecState::Error(_) => {
                    // In Halted/Error mode: only accept quit/restart/inspection.
                    if event::poll(Duration::from_millis(50))? {
                        if let Event::Key(key) = event::read()? {
                            self.handle_key_halted(key);
                        }
                    }
                }
            }
        }
    }

    /// Handle a key event when execution is stopped.
    fn handle_key_stopped(&mut self, key: KeyEvent) {
        // Check for global shortcuts first.
        if self.handle_global_key(&key) {
            return;
        }

        // If the user is typing in the command line, route character input there.
        let is_typing = !self.command_line.is_empty() || self.focus == PaneFocus::Cmd;

        match key.code {
            // Execution shortcuts (only when not typing a partial command).
            KeyCode::F(5) if self.command_line.is_empty() => {
                self.do_continue();
            }
            KeyCode::F(10) if self.command_line.is_empty() => {
                self.do_step();
            }
            KeyCode::F(11) if self.command_line.is_empty() => {
                self.do_step();
            }
            KeyCode::F(9) if self.command_line.is_empty() => {
                self.toggle_breakpoint_at_cursor();
            }
            KeyCode::Char(' ') if self.command_line.is_empty() && self.focus != PaneFocus::Cmd => {
                self.do_step();
            }

            // Navigation.
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.cycle_focus_backward();
                } else {
                    self.cycle_focus_forward();
                }
            }
            KeyCode::BackTab => {
                self.cycle_focus_backward();
            }
            KeyCode::Up => self.scroll_up(1),
            KeyCode::Down => self.scroll_down(1),
            KeyCode::PageUp => self.scroll_up(10),
            KeyCode::PageDown => self.scroll_down(10),
            KeyCode::Home => {
                match self.focus {
                    PaneFocus::Code => self.code_scroll = 0,
                    PaneFocus::State => self.state_scroll = 0,
                    PaneFocus::Quantum => self.quantum_scroll = 0,
                    PaneFocus::Output => self.output_scroll = 0,
                    PaneFocus::Cmd => {}
                }
            }
            KeyCode::End => {
                match self.focus {
                    PaneFocus::Code => {
                        self.code_scroll = self.engine.ctx.program.len().saturating_sub(1);
                    }
                    PaneFocus::Output => {
                        self.output_scroll = self.output_buffer.len().saturating_sub(1);
                    }
                    _ => {}
                }
            }

            // Focus management.
            KeyCode::Esc => {
                self.focus_cmd();
            }

            // Command input.
            KeyCode::Char(c) => {
                if is_typing || self.focus == PaneFocus::Cmd {
                    self.command_line.push(c);
                    self.focus = PaneFocus::Cmd;
                } else if c == ':' {
                    // Colon starts command input from any pane.
                    self.focus_cmd();
                } else if c == ' ' {
                    self.do_step();
                }
            }
            KeyCode::Backspace => {
                self.command_line.pop();
            }
            KeyCode::Enter => {
                self.submit_command();
            }

            _ => {}
        }
    }

    /// Handle a key event when execution is halted or in error state.
    fn handle_key_halted(&mut self, key: KeyEvent) {
        // Check for global shortcuts.
        if self.handle_global_key(&key) {
            return;
        }

        match key.code {
            // Navigation still works.
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.cycle_focus_backward();
                } else {
                    self.cycle_focus_forward();
                }
            }
            KeyCode::BackTab => self.cycle_focus_backward(),
            KeyCode::Up => self.scroll_up(1),
            KeyCode::Down => self.scroll_down(1),
            KeyCode::PageUp => self.scroll_up(10),
            KeyCode::PageDown => self.scroll_down(10),
            KeyCode::Esc => self.focus_cmd(),

            // Command input for inspection/quit/restart.
            KeyCode::Char(c) => {
                self.command_line.push(c);
                self.focus = PaneFocus::Cmd;
            }
            KeyCode::Backspace => {
                self.command_line.pop();
            }
            KeyCode::Enter => {
                self.submit_command();
            }
            _ => {}
        }
    }

    /// Handle global key shortcuts (Ctrl+C, Ctrl+Q, Ctrl+R, Ctrl+L).
    /// Returns true if the key was consumed.
    fn handle_global_key(&mut self, key: &KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('q') => {
                    self.should_quit = true;
                    return true;
                }
                KeyCode::Char('c') => {
                    // In non-running mode, Ctrl+C is quit.
                    self.should_quit = true;
                    return true;
                }
                KeyCode::Char('r') => {
                    self.do_restart();
                    return true;
                }
                KeyCode::Char('l') => {
                    // Force redraw -- nothing to do, the next render will handle it.
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Restart the program.
    fn do_restart(&mut self) {
        // Reload the program from disk.
        let path = self.engine.program_path.to_string_lossy().to_string();
        match cqam_run::loader::load_program(&path) {
            Ok(parsed) => {
                self.engine.restart(parsed.instructions);
                self.execution_state = ExecState::Stopped;
                self.code_scroll = 0;
                self.add_diagnostic("** Program restarted **".to_string());
            }
            Err(e) => {
                self.add_error(format!("Failed to reload program: {}", e));
            }
        }
    }
}
