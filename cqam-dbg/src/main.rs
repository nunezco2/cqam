//! `cqam-dbg` -- TUI debugger for CQAM programs.
//!
//! Provides an interactive terminal UI for stepping through CQAM programs,
//! inspecting registers, quantum state, and setting breakpoints/watchpoints.

mod app;
mod command;
mod ecall;
mod engine;
mod format;
mod ui;

use std::io;
use std::path::PathBuf;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use cqam_run::loader::load_program;
use cqam_run::simconfig::SimConfig;

use crate::app::AppState;
use crate::engine::DebuggerEngine;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    eprintln!("cqam-dbg {}", VERSION);
    eprintln!("Interactive TUI debugger for CQAM programs.\n");
    eprintln!("Usage: cqam-dbg <file.cqam|file.cqb> [OPTIONS]\n");
    eprintln!("Options:");
    eprintln!("  --config <path>       TOML simulator config file");
    eprintln!("  --qubits <n>          Default qubits per quantum register (1-16)");
    eprintln!("  --max-cycles <n>      Maximum instruction cycles");
    eprintln!("  --density-matrix      Force density-matrix backend");
    eprintln!("  --version             Show version");
    eprintln!("  --help                Show this help message");
}

struct CliArgs {
    input: String,
    config_path: Option<String>,
    qubits: Option<u8>,
    max_cycles: Option<usize>,
    density_matrix: bool,
}

fn parse_args() -> Result<CliArgs, String> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) {
        print_help();
        std::process::exit(0);
    }

    if args.contains(&"--version".to_string()) {
        eprintln!("cqam-dbg {}", VERSION);
        std::process::exit(0);
    }

    let mut input: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut qubits: Option<u8> = None;
    let mut max_cycles: Option<usize> = None;
    let mut density_matrix = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = Some(
                    args.get(i)
                        .ok_or("--config requires a path")?
                        .clone(),
                );
            }
            "--qubits" => {
                i += 1;
                let n: u8 = args
                    .get(i)
                    .ok_or("--qubits requires a number")?
                    .parse()
                    .map_err(|_| "--qubits must be a number 1-16")?;
                if n == 0 || n > 16 {
                    return Err("--qubits must be 1-16".to_string());
                }
                qubits = Some(n);
            }
            "--max-cycles" => {
                i += 1;
                let n: usize = args
                    .get(i)
                    .ok_or("--max-cycles requires a number")?
                    .parse()
                    .map_err(|_| "--max-cycles must be a positive integer")?;
                max_cycles = Some(n);
            }
            "--density-matrix" => density_matrix = true,
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            _ => {
                if input.is_some() {
                    return Err(format!("Unexpected argument: {}", args[i]));
                }
                input = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let input = input.ok_or(
        "Missing input file. Usage: cqam-dbg <file.cqam|file.cqb> [OPTIONS]",
    )?;

    Ok(CliArgs {
        input,
        config_path,
        qubits,
        max_cycles,
        density_matrix,
    })
}

/// Set up the terminal for TUI mode: raw mode + alternate screen.
fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Restore the terminal to its original state.
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn main() {
    let cli = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("Error: {}", msg);
            std::process::exit(1);
        }
    };

    // Load config.
    let mut config = if let Some(ref path) = cli.config_path {
        match SimConfig::load(path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Error: Could not load config '{}': {}", path, e);
                std::process::exit(1);
            }
        }
    } else {
        SimConfig::default()
    };

    // CLI overrides.
    if let Some(n) = cli.qubits {
        config.default_qubits = Some(n);
    }
    if let Some(n) = cli.max_cycles {
        config.max_cycles = Some(n);
    }
    if cli.density_matrix {
        config.force_density_matrix = true;
    }

    // Load the program.
    let parsed = match load_program(&cli.input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let num_instructions = parsed.instructions.len();

    // Create the debugger engine with full metadata and data section.
    let engine = DebuggerEngine::new_with_metadata(
        parsed.instructions,
        PathBuf::from(&cli.input),
        config,
        &parsed.metadata,
        Some(&parsed.data_section),
    );

    let mut app = AppState::new(engine);

    // Add a welcome message to the output buffer.
    app.add_diagnostic(format!(
        "cqam-dbg {} -- loaded {} instructions from '{}'",
        VERSION, num_instructions, cli.input,
    ));
    app.add_diagnostic("Type 'help' for commands. Press F10 or Space to step, F5 to continue.".to_string());

    // Initialize terminal.
    let mut terminal = match setup_terminal() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: Could not initialize terminal: {}", e);
            std::process::exit(1);
        }
    };

    // Run the event loop.
    let result = app.run(&mut terminal);

    // Restore terminal.
    if let Err(e) = restore_terminal(&mut terminal) {
        eprintln!("Error restoring terminal: {}", e);
    }

    // Print any final messages.
    match result {
        Ok(msg) => {
            if let Some(msg) = msg {
                eprintln!("{}", msg);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
