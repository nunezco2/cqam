//! STATE pane renderer: displays register files, PSW, call stack, and resources.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::app::{AppState, PaneFocus};
use crate::format::register::{
    format_complex, format_float, format_hybrid, format_int, format_qreg_summary,
    is_complex_zero, is_float_zero, is_hybrid_empty, is_int_zero,
};
use crate::ui::theme;

/// Render the STATE pane into the given area.
pub fn render(frame: &mut Frame, area: Rect, app: &AppState) {
    let is_focused = app.focus == PaneFocus::State;

    let border_style = if is_focused {
        theme::style_border_focus()
    } else {
        theme::style_border_normal()
    };

    let block = Block::default()
        .title(Span::styled(" STATE ", theme::style_title()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height as usize;
    if visible_height == 0 {
        return;
    }

    let mut lines: Vec<ListItem> = Vec::new();

    // PSW flags (top for visibility).
    build_psw_lines(app, &mut lines);

    // Trap flags.
    build_trap_lines(app, &mut lines);

    // Blank separator.
    lines.push(ListItem::new(Line::from("")));

    // R-file: 4 per row.
    build_ireg_lines(app, &mut lines);

    // F-file: 4 per row.
    build_freg_lines(app, &mut lines);

    // Z-file: 2 per row (wider values).
    build_zreg_lines(app, &mut lines);

    // H-file: 4 per row.
    build_hreg_lines(app, &mut lines);

    // Q-file summary: 4 per row.
    build_qreg_lines(app, &mut lines);

    // Blank separator.
    lines.push(ListItem::new(Line::from("")));

    // Call stack.
    build_stack_line(app, &mut lines);

    // Cycle count.
    build_cycle_line(app, &mut lines);

    // Resources.
    build_resource_lines(app, &mut lines);

    // Apply scroll offset.
    let scroll = app.state_scroll.min(lines.len().saturating_sub(visible_height));
    let end = (scroll + visible_height).min(lines.len());
    let visible_items: Vec<ListItem> = lines.into_iter().skip(scroll).take(end - scroll).collect();

    let list = List::new(visible_items);
    frame.render_widget(list, inner);
}

/// Build integer register display lines (4 per row).
fn build_ireg_lines(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let ctx = &app.engine.ctx;
    let snap = &app.engine.prev_snapshot;

    for chunk_start in (0..16).step_by(4) {
        let mut spans = vec![Span::styled("R-file  ", theme::style_dimmed())];
        for i in chunk_start..chunk_start + 4 {
            let val = ctx.iregs.regs[i];
            let changed = snap.ireg_changed(ctx, i);
            let style = reg_style(changed, is_int_zero(val));
            let text = format!("R{}={}", i, format_int(val));
            let suffix = if changed { "*" } else { "" };
            spans.push(Span::styled(format!("{}{}  ", text, suffix), style));
        }
        lines.push(ListItem::new(Line::from(spans)));
    }
}

/// Build float register display lines (4 per row).
fn build_freg_lines(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let ctx = &app.engine.ctx;
    let snap = &app.engine.prev_snapshot;

    for chunk_start in (0..16).step_by(4) {
        let mut spans = vec![Span::styled("F-file  ", theme::style_dimmed())];
        for i in chunk_start..chunk_start + 4 {
            let val = ctx.fregs.regs[i];
            let changed = snap.freg_changed(ctx, i);
            let style = reg_style(changed, is_float_zero(val));
            let text = format!("F{}={}", i, format_float(val));
            let suffix = if changed { "*" } else { "" };
            spans.push(Span::styled(format!("{}{}  ", text, suffix), style));
        }
        lines.push(ListItem::new(Line::from(spans)));
    }
}

/// Build complex register display lines (2 per row).
fn build_zreg_lines(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let ctx = &app.engine.ctx;
    let snap = &app.engine.prev_snapshot;

    for chunk_start in (0..16).step_by(2) {
        let mut spans = vec![Span::styled("Z-file  ", theme::style_dimmed())];
        for i in chunk_start..chunk_start + 2 {
            let (re, im) = ctx.zregs.regs[i];
            let changed = snap.zreg_changed(ctx, i);
            let style = reg_style(changed, is_complex_zero(re, im));
            let text = format!("Z{}={}", i, format_complex(re, im));
            let suffix = if changed { "*" } else { "" };
            spans.push(Span::styled(format!("{}{}  ", text, suffix), style));
        }
        lines.push(ListItem::new(Line::from(spans)));
    }
}

/// Build hybrid register display lines (4 per row).
fn build_hreg_lines(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let ctx = &app.engine.ctx;

    for chunk_start in (0..8).step_by(4) {
        let mut spans = vec![Span::styled("H-file  ", theme::style_dimmed())];
        let end = (chunk_start + 4).min(8);
        for i in chunk_start..end {
            let val = &ctx.hregs.regs[i];
            let style = if is_hybrid_empty(val) {
                theme::style_reg_empty()
            } else {
                theme::style_reg_unchanged()
            };
            let text = format!("H{}={}", i, format_hybrid(val));
            spans.push(Span::styled(format!("{}  ", text), style));
        }
        lines.push(ListItem::new(Line::from(spans)));
    }
}

/// Build quantum register summary lines (4 per row).
fn build_qreg_lines(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let ctx = &app.engine.ctx;

    for chunk_start in (0..8).step_by(4) {
        let mut spans = vec![Span::styled("Q-file  ", theme::style_dimmed())];
        let end = (chunk_start + 4).min(8);
        for i in chunk_start..end {
            let summary = format_qreg_summary(&ctx.qregs[i]);
            let style = if ctx.qregs[i].is_none() {
                theme::style_reg_empty()
            } else {
                theme::style_reg_unchanged()
            };
            let text = format!("Q{}={}", i, summary);
            spans.push(Span::styled(format!("{}  ", text), style));
        }
        lines.push(ListItem::new(Line::from(spans)));
    }
}

/// Build PSW flag display lines, grouped by resource type with colored backgrounds.
fn build_psw_lines(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let psw = &app.engine.ctx.psw;
    let snap = &app.engine.prev_snapshot;
    let ctx = &app.engine.ctx;

    // Group definitions: (label, background, flag_names, flag_values, flag_ids)
    let groups: &[(&str, ratatui::style::Color, &[&str], &[bool], &[usize])] = &[
        ("Classical", theme::BG_FLAG_CLASSICAL, &["ZF", "NF", "OF", "PF"], &[psw.zf, psw.nf, psw.of, psw.pf], &[0, 1, 2, 3]),
        ("Quantum", theme::BG_FLAG_QUANTUM, &["QF", "SF", "EF", "IF"], &[psw.qf, psw.sf, psw.ef, psw.inf], &[4, 5, 6, 12]),
        ("Hybrid", theme::BG_FLAG_HYBRID, &["HF", "DF", "CF", "FK", "MG"], &[psw.hf, psw.df, psw.cf, psw.forked, psw.merged], &[7, 8, 9, 10, 11]),
    ];

    let mut spans = vec![Span::styled("PSW  ", theme::style_dimmed())];
    for &(label, bg, names, values, flag_ids) in groups {
        spans.push(Span::styled(format!(" {} ", label), theme::style_sep_bg(bg)));
        for (j, (&name, &value)) in names.iter().zip(values.iter()).enumerate() {
            let flag_id = flag_ids[j];
            let changed = snap.psw_flag_changed(ctx, flag_id);
            let flag_str = if value { "1" } else { "0" };
            let style = if changed {
                theme::style_flag_changed_bg(bg)
            } else if value {
                theme::style_flag_set_bg(bg)
            } else {
                theme::style_flag_clr_bg(bg)
            };
            spans.push(Span::styled(format!("{}={}", name, flag_str), style));
            spans.push(Span::styled(" ", theme::style_sep_bg(bg)));
        }
        spans.push(Span::styled(" ", theme::style_normal()));
    }
    lines.push(ListItem::new(Line::from(spans)));
}

/// Build trap flag display line with colored background.
fn build_trap_lines(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let psw = &app.engine.ctx.psw;
    let bg = theme::BG_FLAG_TRAP;

    let trap_names = ["halt", "arith", "qerr", "sync"];
    let trap_values = [psw.trap_halt, psw.trap_arith, psw.int_quantum_err, psw.int_sync_fail];

    let mut spans = vec![Span::styled("Traps", theme::style_dimmed())];
    spans.push(Span::styled(" ", theme::style_sep_bg(bg)));
    for (&name, &value) in trap_names.iter().zip(trap_values.iter()) {
        let flag_str = if value { "1" } else { "0" };
        let style = if value {
            theme::style_trap_set_bg(bg)
        } else {
            theme::style_trap_clr_bg(bg)
        };
        spans.push(Span::styled(format!("{}={}", name, flag_str), style));
        spans.push(Span::styled(" ", theme::style_sep_bg(bg)));
    }
    lines.push(ListItem::new(Line::from(spans)));
}

/// Build call stack display line.
fn build_stack_line(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let stack = &app.engine.ctx.call_stack;
    let depth = stack.len();

    let mut spans = vec![Span::styled("Stack   ", theme::style_dimmed())];
    spans.push(Span::styled(format!("depth={}", depth), theme::style_normal()));

    if !stack.is_empty() {
        let addrs: Vec<String> = stack.iter().map(|a| format!("0x{:04X}", a)).collect();
        // Show up to 8 most recent return addresses.
        let display: Vec<&str> = addrs.iter().rev().take(8).map(|s| s.as_str()).collect();
        spans.push(Span::styled(
            format!("  [{}]", display.join(", ")),
            theme::style_dimmed(),
        ));
    }

    lines.push(ListItem::new(Line::from(spans)));
}

/// Build cycle count display line.
fn build_cycle_line(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let cycle = app.engine.cycle_count;
    let max = app.engine.max_cycles;

    let spans = vec![
        Span::styled("Cycle   ", theme::style_dimmed()),
        Span::styled(format!("{} / {}", cycle, max), theme::style_normal()),
    ];
    lines.push(ListItem::new(Line::from(spans)));
}

/// Build resource tracker display line.
fn build_resource_lines(app: &AppState, lines: &mut Vec<ListItem<'static>>) {
    let rt = &app.engine.ctx.resource_tracker;

    let spans = vec![
        Span::styled("Resources  ", theme::style_dimmed()),
        Span::styled(format!("T={}  ", rt.total_time), theme::style_normal()),
        Span::styled(format!("S={}  ", rt.total_space), theme::style_normal()),
        Span::styled(format!("Sup={:.1}  ", rt.total_superposition), theme::style_normal()),
        Span::styled(format!("Ent={:.1}  ", rt.total_entanglement), theme::style_normal()),
        Span::styled(format!("Int={:.1}", rt.total_interference), theme::style_normal()),
    ];
    lines.push(ListItem::new(Line::from(spans)));
}

/// Determine the style for a register value based on change status and zero-ness.
fn reg_style(changed: bool, is_zero: bool) -> ratatui::style::Style {
    if changed {
        theme::style_reg_changed()
    } else if is_zero {
        theme::style_reg_zero()
    } else {
        theme::style_reg_unchanged()
    }
}
