//! CODE pane renderer: displays the instruction listing with PC cursor and
//! breakpoint markers.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use cqam_core::instruction::Instruction;

use crate::app::{AppState, PaneFocus};
use crate::format::instruction::format_instruction;
use crate::ui::theme;

/// Breakpoint marker: filled red circle.
const BP_MARKER: &str = "\u{25CF}";
/// No breakpoint: space placeholder.
const NO_BP_MARKER: &str = " ";

/// Render the CODE pane into the given area.
pub fn render(frame: &mut Frame, area: Rect, app: &AppState) {
    let is_focused = app.focus == PaneFocus::Code;

    let border_style = if is_focused {
        theme::style_border_focus()
    } else {
        theme::style_border_normal()
    };

    let block = Block::default()
        .title(Span::styled(" CODE ", theme::style_title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let program = &app.engine.ctx.program;
    let pc = app.engine.ctx.pc;
    let visible_height = inner.height as usize;

    if program.is_empty() || visible_height == 0 {
        return;
    }

    // Auto-scroll to keep PC visible with 5 lines look-ahead.
    let scroll = compute_scroll(app.code_scroll, pc, visible_height, program.len());

    // Build visible list items.
    let end = (scroll + visible_height).min(program.len());
    let items: Vec<ListItem> = (scroll..end)
        .map(|idx| build_code_line(app, idx, pc))
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

/// Compute the scroll offset to keep the PC visible with 5-line look-ahead.
fn compute_scroll(current_scroll: usize, pc: usize, visible_height: usize, total: usize) -> usize {
    if visible_height == 0 || total == 0 {
        return 0;
    }

    let look_ahead = 5;
    let mut scroll = current_scroll;

    // If PC is above the visible window, scroll up to show it.
    if pc < scroll {
        scroll = pc;
    }
    // If PC + look_ahead is below the visible window, scroll down.
    else if pc + look_ahead >= scroll + visible_height {
        scroll = (pc + look_ahead + 1).saturating_sub(visible_height);
    }

    // Clamp scroll to valid range.
    let max_scroll = total.saturating_sub(visible_height);
    scroll.min(max_scroll)
}

/// Build a single code line as a ListItem.
///
/// Format: `[BP] PPPP  MNEMONIC OPERANDS`
fn build_code_line(app: &AppState, idx: usize, pc: usize) -> ListItem<'static> {
    let instr = &app.engine.ctx.program[idx];
    let is_current_pc = idx == pc;

    // Check for labels -- display as dimmed inline text.
    if let Instruction::Label(name) = instr {
        let label_text = format!("      {}:", name);
        let style = theme::style_dimmed();
        return ListItem::new(Line::from(Span::styled(label_text, style)));
    }

    // Breakpoint marker.
    let (bp_marker, bp_style) = match app.engine.breakpoints.has_breakpoint_at(idx) {
        Some(bp) => {
            if !bp.enabled {
                (BP_MARKER, theme::style_disabled_bp())
            } else if bp.condition.is_some() {
                (BP_MARKER, theme::style_conditional_bp())
            } else {
                (BP_MARKER, theme::style_breakpoint())
            }
        }
        None => (NO_BP_MARKER, theme::style_dimmed()),
    };

    // Format address.
    let addr_str = format!("{:04X}", idx);

    // Format instruction mnemonic + operands.
    let instr_text = format_instruction(instr);

    // Build the line with spans.
    let line_style = if is_current_pc {
        theme::style_current_pc()
    } else if app.engine.breakpoints.has_breakpoint_at(idx).is_some() {
        theme::style_breakpoint_line()
    } else {
        theme::style_normal()
    };

    let spans = vec![
        Span::styled(format!("{} ", bp_marker), bp_style),
        Span::styled(format!("{}  ", addr_str), line_style),
        Span::styled(instr_text, line_style),
    ];

    ListItem::new(Line::from(spans))
}
