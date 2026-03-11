//! QUANTUM pane renderer: displays the quantum state of the selected Q register.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::app::{AppState, PaneFocus};
use crate::format::quantum::{
    coherence_summary, extract_top_k, format_amplitude, format_basis_ket, format_coherence,
    format_filter_summary, format_phase, format_quantum_header, format_suppressed, render_bar,
};
use crate::ui::theme;

/// Bar chart maximum width in terminal cells.
const BAR_MAX_WIDTH: usize = 20;

/// Render the QUANTUM pane into the given area.
pub fn render(frame: &mut Frame, area: Rect, app: &AppState) {
    let is_focused = app.focus == PaneFocus::Quantum;

    let border_style = if is_focused {
        theme::style_border_focus()
    } else {
        theme::style_border_normal()
    };

    let block = Block::default()
        .title(Span::styled(" QUANTUM ", theme::style_title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height as usize;
    if visible_height == 0 {
        return;
    }

    let qreg_idx = app.display.selected_qreg;
    let qreg = &app.engine.ctx.qregs[qreg_idx as usize];

    let mut lines: Vec<ListItem> = Vec::new();

    match qreg {
        None => {
            // No quantum register allocated.
            lines.push(ListItem::new(Line::from(Span::styled(
                format!("Q{}: not allocated", qreg_idx),
                theme::style_dimmed(),
            ))));
        }
        Some(qr) => {
            // Extract top-K results.
            let result = extract_top_k(qr, app.display.topk, app.display.threshold);

            // Header line: "Q0: Pure, 3 qubits, dim=8, purity=1.000"
            let header_text = format_quantum_header(&result, qreg_idx);
            let header_style = theme::style_quantum_type(result.is_pure);
            lines.push(ListItem::new(Line::from(Span::styled(
                header_text,
                header_style,
            ))));

            // Filter summary line.
            let summary_text =
                format_filter_summary(&result, app.display.topk, app.display.threshold);
            lines.push(ListItem::new(Line::from(Span::styled(
                summary_text,
                theme::style_dimmed(),
            ))));

            // Blank line.
            lines.push(ListItem::new(Line::from("")));

            // Table header.
            let table_header = Line::from(vec![
                Span::styled(
                    format!(
                        "{:<width$}",
                        "Basis",
                        width = result.num_qubits as usize + 3
                    ),
                    theme::style_dimmed(),
                ),
                Span::styled("  Prob     ", theme::style_dimmed()),
                Span::styled("Amplitude         ", theme::style_dimmed()),
                Span::styled("Phase     ", theme::style_dimmed()),
                Span::styled("Bar", theme::style_dimmed()),
            ]);
            lines.push(ListItem::new(table_header));

            // Table entries.
            for entry in &result.entries {
                let basis = format_basis_ket(entry.basis_index, result.num_qubits);
                let prob_str = format!("{:.4}", entry.probability);
                let amp_str = if result.is_pure {
                    format_amplitude(entry.amplitude)
                } else {
                    "--".to_string()
                };
                let phase_str = format_phase(entry.phase);
                let bar_str = render_bar(entry.probability, BAR_MAX_WIDTH);

                let prob_style = theme::style_prob(entry.probability);
                let bar_color = theme::prob_color(entry.probability);

                let basis_width = result.num_qubits as usize + 3;
                let entry_line = Line::from(vec![
                    Span::styled(
                        format!("{:<width$}", basis, width = basis_width),
                        theme::style_normal(),
                    ),
                    Span::styled(format!("  {:<9}", prob_str), prob_style),
                    Span::styled(format!("{:<18}", amp_str), theme::style_normal()),
                    Span::styled(format!("{:<10}", phase_str), theme::style_normal()),
                    Span::styled(
                        bar_str,
                        ratatui::style::Style::default().fg(bar_color),
                    ),
                ]);
                lines.push(ListItem::new(entry_line));
            }

            // Suppressed count.
            let suppressed = format_suppressed(result.suppressed_count);
            if !suppressed.is_empty() {
                lines.push(ListItem::new(Line::from(Span::styled(
                    suppressed,
                    theme::style_dimmed(),
                ))));
            }

            // Coherence summary for mixed states.
            if !result.is_pure {
                if let Some(coherence) = coherence_summary(qr) {
                    lines.push(ListItem::new(Line::from("")));
                    let coherence_text = format_coherence(&coherence);
                    lines.push(ListItem::new(Line::from(Span::styled(
                        coherence_text,
                        theme::style_quantum_type(false),
                    ))));
                }
            }
        }
    }

    // Apply scroll offset.
    let scroll = app
        .quantum_scroll
        .min(lines.len().saturating_sub(visible_height));
    let visible_items: Vec<ListItem> = lines
        .into_iter()
        .skip(scroll)
        .take(visible_height)
        .collect();

    let list = List::new(visible_items);
    frame.render_widget(list, inner);
}
