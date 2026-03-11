//! Command parser and dispatcher for the debugger command language.

pub mod exec;
pub mod parse;

/// A parsed debugger command.
#[derive(Debug, Clone)]
pub enum Command {
    // Execution control
    Step(usize),
    Next,
    Continue,
    Run,
    RunTo(RunTarget),
    Finish,

    // Breakpoints
    BreakAddr(usize, Option<String>),   // address, optional condition string
    BreakLabel(String, Option<String>),  // label, optional condition string
    BreakClass(String),
    Delete(DeleteTarget),
    Enable(usize),
    Disable(usize),

    // Watchpoints
    Watch(String),
    Unwatch(UnwatchTarget),

    // Inspection
    Print(PrintTarget),
    Info(InfoSubcommand),

    // Display settings
    SetThreshold(f64),
    SetTopK(usize),
    SetQReg(u8),
    Focus(FocusTarget),

    // Program control
    Load(String),
    Restart,
    Quit,
    Help(Option<String>),

    // Internal
    Empty,
}

/// Target for `run to`.
#[derive(Debug, Clone)]
pub enum RunTarget {
    Addr(usize),
    Label(String),
}

/// Target for `delete`.
#[derive(Debug, Clone)]
pub enum DeleteTarget {
    Id(usize),
    All,
}

/// Target for `unwatch`.
#[derive(Debug, Clone)]
pub enum UnwatchTarget {
    Register(String),
    All,
}

/// Target for `print`.
#[derive(Debug, Clone)]
pub enum PrintTarget {
    Register(String),
    CmemAddr(u16),
    CmemRange(u16, u16),
}

/// Subcommand for `info`.
#[derive(Debug, Clone)]
pub enum InfoSubcommand {
    Breakpoints,
    Watchpoints,
    Registers(Option<String>),
    Quantum(Option<u8>),
    Psw,
    Resources,
    Stack,
    Labels,
    Program,
}

/// Target for `focus`.
#[derive(Debug, Clone)]
pub enum FocusTarget {
    Code,
    State,
    Quantum,
    Output,
}
