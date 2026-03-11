//! Top-level TUI layout and pane rendering.
//!
//! Splits the terminal into the five panes defined in the design document:
//! CODE, STATE, QUANTUM, OUTPUT, and CMD.

pub mod code_pane;
pub mod cmd_pane;
pub mod output_pane;
pub mod quantum_pane;
pub mod state_pane;
pub mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use crate::app::AppState;

/// Compute the five pane rectangles from the terminal area using Layout D.
///
/// Returns `(code, state, quantum, output, cmd)`.
fn layout_panes(area: Rect) -> (Rect, Rect, Rect, Rect, Rect) {
    // Split vertically: top 50%, middle 40%, bottom 10%.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // top row (CODE + STATE)
            Constraint::Percentage(40), // bottom row (QUANTUM + OUTPUT)
            Constraint::Percentage(10), // CMD bar
        ])
        .split(area);

    // Split top row: CODE 55%, STATE 45%.
    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55),
            Constraint::Percentage(45),
        ])
        .split(rows[0]);

    // Split bottom row: QUANTUM 55%, OUTPUT 45%.
    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55),
            Constraint::Percentage(45),
        ])
        .split(rows[1]);

    (top_cols[0], top_cols[1], bottom_cols[0], bottom_cols[1], rows[2])
}

/// Render the full debugger UI into the given frame.
pub fn render(frame: &mut Frame, app: &AppState) {
    let (code_area, state_area, quantum_area, output_area, cmd_area) =
        layout_panes(frame.area());

    code_pane::render(frame, code_area, app);
    state_pane::render(frame, state_area, app);
    quantum_pane::render(frame, quantum_area, app);
    output_pane::render(frame, output_area, app);
    cmd_pane::render(frame, cmd_area, app);
}
