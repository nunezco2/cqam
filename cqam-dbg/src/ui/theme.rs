//! Color constants and style constructors for the TUI debugger.
//!
//! All colors are specified as ANSI 256 values. The scheme is designed for dark
//! terminals (black or near-black backgrounds).

use ratatui::style::{Color, Modifier, Style};

// =============================================================================
// 2.1 Base palette
// =============================================================================

/// Normal text foreground: Gray (250).
pub const FG_NORMAL: Color = Color::Indexed(250);

/// Dimmed text foreground: DarkGray (244). Labels, zero-value registers.
pub const FG_DIMMED: Color = Color::Indexed(244);

/// Active pane border: Cyan (51).
pub const BORDER_FOCUS: Color = Color::Indexed(51);

/// Inactive pane border: DarkGray (240).
pub const BORDER_NORMAL: Color = Color::Indexed(240);

/// Pane title foreground: Bold White (255).
pub const FG_TITLE: Color = Color::Indexed(255);


/// Command prompt foreground: Green (46).
pub const FG_PROMPT: Color = Color::Indexed(46);

// =============================================================================
// 2.2 Debugging semantics
// =============================================================================

/// Current PC line background: DarkBlue (24).
pub const BG_CURRENT_PC: Color = Color::Indexed(24);

/// Breakpoint marker foreground: Red (196) bold.
pub const FG_BREAKPOINT: Color = Color::Indexed(196);

/// Breakpoint line background (no PC): DarkRed (52).
pub const BG_BREAKPOINT_LINE: Color = Color::Indexed(52);

/// Conditional breakpoint marker: Yellow (220) bold.
pub const FG_CONDITIONAL_BP: Color = Color::Indexed(220);

/// Disabled breakpoint marker: DarkGray (244).
pub const FG_DISABLED_BP: Color = Color::Indexed(244);

// =============================================================================
// 2.3 Register change tracking
// =============================================================================

/// Value unchanged since last step: Gray (250).
pub const FG_REG_UNCHANGED: Color = Color::Indexed(250);

/// Value changed since last step: Bold Yellow (226).
pub const FG_REG_CHANGED: Color = Color::Indexed(226);

/// Value became zero: DarkGray (244).
pub const FG_REG_ZERO: Color = Color::Indexed(244);

/// Register uninitialized / empty: DarkGray (244).
pub const FG_REG_EMPTY: Color = Color::Indexed(244);

// =============================================================================
// 2.4 PSW flag coloring
// =============================================================================

/// Flag is SET (1): Bold Green (46).
pub const FG_FLAG_SET: Color = Color::Indexed(46);

/// Flag is CLR (0): DarkGray (244).
pub const FG_FLAG_CLR: Color = Color::Indexed(244);

/// Trap flag SET: Bold Red (196).
pub const FG_TRAP_SET: Color = Color::Indexed(196);

/// Trap halt SET background: Red (52) for flashing danger.
pub const BG_TRAP_HALT: Color = Color::Indexed(52);

// =============================================================================
// 2.4b PSW flag group backgrounds
// =============================================================================

/// Classical flags (ZF, NF, OF, PF) background: dark blue (17).
pub const BG_FLAG_CLASSICAL: Color = Color::Indexed(17);

/// Quantum flags (QF, SF, EF) background: dark green (22).
pub const BG_FLAG_QUANTUM: Color = Color::Indexed(22);

/// Hybrid/measurement flags (HF, DF, CF, FK, MG) background: dark magenta (53).
pub const BG_FLAG_HYBRID: Color = Color::Indexed(53);

/// Trap flags background: dark red (52).
pub const BG_FLAG_TRAP: Color = Color::Indexed(52);

// =============================================================================
// 2.5 Quantum state coloring -- five-stop heat map
// =============================================================================

/// Probability 0.000--0.001: DarkGray (240).
pub const FG_PROB_ZERO: Color = Color::Indexed(240);

/// Probability 0.001--0.100: Blue (33).
pub const FG_PROB_LOW: Color = Color::Indexed(33);

/// Probability 0.100--0.300: Cyan (45).
pub const FG_PROB_MED: Color = Color::Indexed(45);

/// Probability 0.300--0.600: Yellow (226).
pub const FG_PROB_HIGH: Color = Color::Indexed(226);

/// Probability 0.600--1.000: Bold Red (196).
pub const FG_PROB_MAX: Color = Color::Indexed(196);

/// Pure state header: Green (46).
pub const FG_PURE: Color = Color::Indexed(46);

/// Mixed state header: Magenta (201).
pub const FG_MIXED: Color = Color::Indexed(201);

// =============================================================================
// 2.6 Execution state colors
// =============================================================================

/// STOPPED: White on DarkBlue (17).
pub const FG_STATE_STOPPED: Color = Color::Indexed(255);
pub const BG_STATE_STOPPED: Color = Color::Indexed(17);

/// RUNNING: Black on Green (46).
pub const FG_STATE_RUNNING: Color = Color::Indexed(0);
pub const BG_STATE_RUNNING: Color = Color::Indexed(46);

/// HALTED: White on DarkGray (240).
pub const FG_STATE_HALTED: Color = Color::Indexed(255);
pub const BG_STATE_HALTED: Color = Color::Indexed(240);

/// ERROR: Bold White on Red (196).
pub const FG_STATE_ERROR: Color = Color::Indexed(255);
pub const BG_STATE_ERROR: Color = Color::Indexed(196);

// =============================================================================
// Style constructors
// =============================================================================

/// Normal text style.
pub fn style_normal() -> Style {
    Style::default().fg(FG_NORMAL)
}

/// Dimmed text style.
pub fn style_dimmed() -> Style {
    Style::default().fg(FG_DIMMED)
}

/// Pane title style: bold white.
pub fn style_title() -> Style {
    Style::default().fg(FG_TITLE).add_modifier(Modifier::BOLD)
}

/// Focused pane border style.
pub fn style_border_focus() -> Style {
    Style::default().fg(BORDER_FOCUS)
}

/// Unfocused pane border style.
pub fn style_border_normal() -> Style {
    Style::default().fg(BORDER_NORMAL)
}

/// Current PC line style: bold white on dark blue.
pub fn style_current_pc() -> Style {
    Style::default()
        .fg(FG_TITLE)
        .bg(BG_CURRENT_PC)
        .add_modifier(Modifier::BOLD)
}

/// Breakpoint marker style: bold red.
pub fn style_breakpoint() -> Style {
    Style::default()
        .fg(FG_BREAKPOINT)
        .add_modifier(Modifier::BOLD)
}

/// Conditional breakpoint marker style: bold yellow.
pub fn style_conditional_bp() -> Style {
    Style::default()
        .fg(FG_CONDITIONAL_BP)
        .add_modifier(Modifier::BOLD)
}

/// Disabled breakpoint marker style: dark gray.
pub fn style_disabled_bp() -> Style {
    Style::default().fg(FG_DISABLED_BP)
}

/// Breakpoint line background style (line has a breakpoint but PC is elsewhere).
pub fn style_breakpoint_line() -> Style {
    Style::default().bg(BG_BREAKPOINT_LINE)
}

/// Register value that changed since last step: bold yellow.
pub fn style_reg_changed() -> Style {
    Style::default()
        .fg(FG_REG_CHANGED)
        .add_modifier(Modifier::BOLD)
}

/// Register value unchanged: gray.
pub fn style_reg_unchanged() -> Style {
    Style::default().fg(FG_REG_UNCHANGED)
}

/// Register value that became zero: dark gray.
pub fn style_reg_zero() -> Style {
    Style::default().fg(FG_REG_ZERO)
}

/// Register uninitialized: dark gray.
pub fn style_reg_empty() -> Style {
    Style::default().fg(FG_REG_EMPTY)
}

/// PSW flag SET style: bold green.
pub fn style_flag_set() -> Style {
    Style::default()
        .fg(FG_FLAG_SET)
        .add_modifier(Modifier::BOLD)
}

/// PSW flag CLR style: dark gray.
pub fn style_flag_clr() -> Style {
    Style::default().fg(FG_FLAG_CLR)
}

/// PSW flag SET with group background.
pub fn style_flag_set_bg(bg: Color) -> Style {
    Style::default()
        .fg(FG_FLAG_SET)
        .bg(bg)
        .add_modifier(Modifier::BOLD)
}

/// PSW flag CLR with group background.
pub fn style_flag_clr_bg(bg: Color) -> Style {
    Style::default().fg(FG_FLAG_CLR).bg(bg)
}

/// Flag changed with group background.
pub fn style_flag_changed_bg(bg: Color) -> Style {
    Style::default()
        .fg(FG_REG_CHANGED)
        .bg(bg)
        .add_modifier(Modifier::BOLD)
}

/// Trap flag SET with group background.
pub fn style_trap_set_bg(bg: Color) -> Style {
    Style::default()
        .fg(FG_TRAP_SET)
        .bg(bg)
        .add_modifier(Modifier::BOLD)
}

/// Trap flag CLR with group background.
pub fn style_trap_clr_bg(bg: Color) -> Style {
    Style::default().fg(FG_FLAG_CLR).bg(bg)
}

/// Separator span with group background (for spacing between flags).
pub fn style_sep_bg(bg: Color) -> Style {
    Style::default().bg(bg)
}

/// Trap flag SET style: bold red.
pub fn style_trap_set() -> Style {
    Style::default()
        .fg(FG_TRAP_SET)
        .add_modifier(Modifier::BOLD)
}

/// Trap halt SET style: bold white on red background.
pub fn style_trap_halt() -> Style {
    Style::default()
        .fg(FG_STATE_ERROR)
        .bg(BG_TRAP_HALT)
        .add_modifier(Modifier::BOLD)
}

/// Return the heat-map color for a given probability value.
pub fn prob_color(p: f64) -> Color {
    if p < 0.001 {
        FG_PROB_ZERO
    } else if p < 0.100 {
        FG_PROB_LOW
    } else if p < 0.300 {
        FG_PROB_MED
    } else if p < 0.600 {
        FG_PROB_HIGH
    } else {
        FG_PROB_MAX
    }
}

/// Style for a probability value using the heat-map palette.
pub fn style_prob(p: f64) -> Style {
    let color = prob_color(p);
    let mut style = Style::default().fg(color);
    if p >= 0.600 {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

/// Style for a quantum state type label (Pure or Mixed).
pub fn style_quantum_type(is_pure: bool) -> Style {
    if is_pure {
        Style::default()
            .fg(FG_PURE)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(FG_MIXED)
            .add_modifier(Modifier::BOLD)
    }
}

/// Command prompt style: green.
pub fn style_prompt() -> Style {
    Style::default().fg(FG_PROMPT)
}

/// Status bar style for a given execution state.
pub fn style_status_bar(state: &str) -> Style {
    match state {
        "RUNNING" => Style::default()
            .fg(FG_STATE_RUNNING)
            .bg(BG_STATE_RUNNING),
        "HALTED" => Style::default()
            .fg(FG_STATE_HALTED)
            .bg(BG_STATE_HALTED),
        "ERROR" => Style::default()
            .fg(FG_STATE_ERROR)
            .bg(BG_STATE_ERROR)
            .add_modifier(Modifier::BOLD),
        _ => Style::default()
            .fg(FG_STATE_STOPPED)
            .bg(BG_STATE_STOPPED),
    }
}
