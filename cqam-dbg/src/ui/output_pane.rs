//! OUTPUT pane renderer: displays ECALL output and debugger diagnostics.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::app::{AppState, PaneFocus};
use crate::ecall::OutputSource;
use crate::ui::theme;

/// Render the OUTPUT pane into the given area.
pub fn render(frame: &mut Frame, area: Rect, app: &AppState) {
    let is_focused = app.focus == PaneFocus::Output;

    let border_style = if is_focused {
        theme::style_border_focus()
    } else {
        theme::style_border_normal()
    };

    let block = Block::default()
        .title(Span::styled(" OUTPUT ", theme::style_title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height as usize;
    if visible_height == 0 {
        return;
    }

    // Combine engine ecall buffer and app output buffer.
    let ecall_lines = &app.engine.ecall_interceptor.buffer;
    let app_lines = &app.output_buffer;

    // Merge both output sources, maintaining order.
    let total_count = ecall_lines.len() + app_lines.len();

    let items: Vec<ListItem> = if total_count == 0 {
        vec![ListItem::new(Line::from(Span::styled(
            "(no output)",
            theme::style_dimmed(),
        )))]
    } else {
        // Collect all output lines, sorted by cycle (stable order for same cycle).
        let mut all_lines: Vec<&crate::ecall::OutputLine> = Vec::with_capacity(total_count);
        all_lines.extend(ecall_lines.iter());
        all_lines.extend(app_lines.iter());
        // Already in insertion order (chronological), no need to sort.

        all_lines
            .iter()
            .map(|line| {
                let cycle_prefix = format!("[cycle {:04}] ", line.cycle);
                let text_style = match line.source {
                    OutputSource::Ecall => theme::style_normal(),
                    OutputSource::Debugger => theme::style_dimmed(),
                    OutputSource::Error => {
                        ratatui::style::Style::default()
                            .fg(theme::FG_BREAKPOINT)
                            .add_modifier(ratatui::style::Modifier::BOLD)
                    }
                };

                let prefix_style = theme::style_dimmed();

                ListItem::new(Line::from(vec![
                    Span::styled(cycle_prefix, prefix_style),
                    Span::styled(line.text.clone(), text_style),
                ]))
            })
            .collect()
    };

    // Auto-scroll to show newest output at the bottom.
    let total = items.len();
    let scroll = if total > visible_height {
        // Use app's scroll offset if manually scrolled, otherwise auto-scroll to bottom.
        let auto_scroll = total.saturating_sub(visible_height);
        if app.output_scroll > 0 {
            app.output_scroll.min(auto_scroll)
        } else {
            auto_scroll
        }
    } else {
        0
    };

    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(scroll)
        .take(visible_height)
        .collect();

    let list = List::new(visible_items);
    frame.render_widget(list, inner);
}
