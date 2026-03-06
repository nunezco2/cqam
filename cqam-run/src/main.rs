//! `cqam-run` -- command-line runner for CQAM programs.
//!
//! Loads a `.cqam` source file, optionally loads a TOML simulator
//! configuration, executes the program on the [`cqam_vm`] execution engine,
//! and optionally prints a final-state report.
//!
//! # Usage
//!
//! ```text
//! cqam-run <file.cqam|file.cqb> [OPTIONS]
//!
//! Options:
//!   --config <path>         Path to TOML simulator config
//!   --qubits <n>            Default qubits per quantum register (overrides config)
//!   --max-cycles <n>        Maximum instruction cycles (overrides config)
//!   --density-matrix        Force density-matrix backend (no statevector)
//!   --print-final-state     Dump all non-zero registers and memory after execution
//!   --psw                   Print the Program State Word
//!   --resources             Print cumulative resource usage counters
//!   --verbose               Print config and execution summary
//!   --version               Show version
//!   --help                  Show this help message
//! ```

use std::process;
use cqam_run::loader::load_program;
use cqam_run::runner::run_program_with_config_and_metadata;
use cqam_run::report::print_report;
use cqam_run::simconfig::SimConfig;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    eprintln!("cqam-run {}", VERSION);
    eprintln!("Execute a CQAM program.\n");
    eprintln!("Usage: cqam-run <file.cqam|file.cqb> [OPTIONS]\n");
    eprintln!("Options:");
    eprintln!("  --config <path>       TOML simulator config file");
    eprintln!("  --qubits <n>          Default qubits per quantum register (1-16)");
    eprintln!("  --max-cycles <n>      Maximum instruction cycles before forced halt");
    eprintln!("  --print-final-state   Dump all non-zero registers and memory");
    eprintln!("  --psw                 Print the Program State Word");
    eprintln!("  --resources           Print cumulative resource usage counters");
    eprintln!("  --density-matrix      Force density-matrix backend (no statevector)");
    eprintln!("  --verbose             Print config and execution summary");
    eprintln!("  --version             Show version");
    eprintln!("  --help                Show this help message");
}

struct CliArgs {
    input: String,
    config_path: Option<String>,
    qubits: Option<u8>,
    max_cycles: Option<usize>,
    density_matrix: bool,
    print_state: bool,
    print_psw: bool,
    print_resources: bool,
    verbose: bool,
}

fn parse_args() -> Result<CliArgs, String> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) {
        print_help();
        std::process::exit(0);
    }

    if args.contains(&"--version".to_string()) {
        eprintln!("cqam-run {}", VERSION);
        std::process::exit(0);
    }

    let mut input: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut qubits: Option<u8> = None;
    let mut max_cycles: Option<usize> = None;
    let mut density_matrix = false;
    let mut print_state = false;
    let mut print_psw = false;
    let mut print_resources = false;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = Some(args.get(i).ok_or("--config requires a path")?.clone());
            }
            "--qubits" => {
                i += 1;
                let n: u8 = args.get(i)
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
                let n: usize = args.get(i)
                    .ok_or("--max-cycles requires a number")?
                    .parse()
                    .map_err(|_| "--max-cycles must be a positive integer")?;
                max_cycles = Some(n);
            }
            "--density-matrix" => density_matrix = true,
            "--print-final-state" => print_state = true,
            "--psw" => print_psw = true,
            "--resources" => print_resources = true,
            "--verbose" => verbose = true,
            // Backward compatibility
            "--psw-report" => print_psw = true,
            "--resource-usage" => print_resources = true,
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

    let input = input.ok_or("Missing input file. Usage: cqam-run <file.cqam|file.cqb> [OPTIONS]")?;

    Ok(CliArgs {
        input,
        config_path,
        qubits,
        max_cycles,
        density_matrix,
        print_state,
        print_psw,
        print_resources,
        verbose,
    })
}

fn main() {
    env_logger::init();

    let cli = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("Error: {}", msg);
            process::exit(1);
        }
    };

    // Load config: explicit path, or try default, or use built-in defaults
    let mut config = if let Some(ref path) = cli.config_path {
        match SimConfig::load(path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Error: Could not load config '{}': {}", path, e);
                process::exit(1);
            }
        }
    } else {
        SimConfig::default()
    };

    // CLI overrides
    if let Some(n) = cli.qubits {
        config.default_qubits = Some(n);
    }
    if let Some(n) = cli.max_cycles {
        config.max_cycles = Some(n);
    }
    if cli.density_matrix {
        config.force_density_matrix = true;
    }

    if cli.verbose {
        eprintln!("Config: {:?}", config);
    }

    let parsed = match load_program(&cli.input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    if cli.verbose {
        eprintln!("Loaded {} instructions from {}", parsed.instructions.len(), cli.input);
    }

    let ctx = match run_program_with_config_and_metadata(parsed.instructions, &config, &parsed.metadata) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Runtime error: {}", e);
            process::exit(1);
        }
    };

    if cli.verbose {
        eprintln!("Execution complete (PC={}, halted={})", ctx.pc, ctx.psw.trap_halt);
    }

    print_report(&ctx, cli.print_state, cli.print_psw, cli.print_resources);
}
