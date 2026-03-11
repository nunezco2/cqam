//! CMD pane renderer: status bar and command input line.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{AppState, ExecState, PaneFocus};
use crate::ui::theme;

/// Render the CMD pane into the given area.
///
/// The CMD pane has two sub-regions:
/// 1. Status bar (top line): execution state, PC, selected Q register, breakpoint counts.
/// 2. Command input (bottom line): "dbg>" prompt + current command text.
pub fn render(frame: &mut Frame, area: Rect, app: &AppState) {
    let is_focused = app.focus == PaneFocus::Cmd;

    let border_style = if is_focused {
        theme::style_border_focus()
    } else {
        theme::style_border_normal()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        // Not enough space for both status bar and input line.
        if inner.height >= 1 {
            render_command_line(frame, inner, app);
        }
        return;
    }

    // Split inner area into status bar (1 line) and command input (1 line).
    let cmd_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(1),   // command input
        ])
        .split(inner);

    render_status_bar(frame, cmd_chunks[0], app);
    render_command_line(frame, cmd_chunks[1], app);
}

/// Render the status bar line.
fn render_status_bar(frame: &mut Frame, area: Rect, app: &AppState) {
    let (state_text, state_key) = match &app.execution_state {
        ExecState::Stopped => ("STOPPED", "STOPPED"),
        ExecState::Running => ("RUNNING", "RUNNING"),
        ExecState::Halted => ("HALTED", "HALTED"),
        ExecState::Error(msg) => {
            // We need to return owned data, but we build the line below.
            // For the style key, just use "ERROR".
            let _ = msg; // used below when building spans
            ("ERROR", "ERROR")
        }
    };

    let status_style = theme::style_status_bar(state_key);

    let pc = app.engine.ctx.pc;
    let qreg = app.display.selected_qreg;
    let bp_count = app.engine.breakpoints.enabled_count();
    let wp_count = app.engine.watchpoints.len();
    let cycle = app.engine.cycle_count;

    // Build the error suffix if applicable.
    let state_display = match &app.execution_state {
        ExecState::Error(msg) => format!("ERROR: {}", msg),
        _ => state_text.to_string(),
    };

    // Build status bar content.
    let status_line = format!(
        "  {}  PC={:04X}  Q=Q{}  BP={}  WP={}  cycle={}",
        state_display, pc, qreg, bp_count, wp_count, cycle
    );

    // Pad to fill the full width.
    let padded = format!("{:<width$}", status_line, width = area.width as usize);

    let paragraph = Paragraph::new(Line::from(Span::styled(padded, status_style)));
    frame.render_widget(paragraph, area);
}

/// Render the command input line.
fn render_command_line(frame: &mut Frame, area: Rect, app: &AppState) {
    let prompt = Span::styled("dbg> ", theme::style_prompt());
    let input = Span::styled(app.command_line.clone(), theme::style_normal());

    let line = Line::from(vec![prompt, input]);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
