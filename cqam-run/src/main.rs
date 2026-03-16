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
//!   --threads <n>           Default thread count for HFORK (1-256)
//!   --shots <n>             Number of shots for QPU-realistic sampling
//!   --noise <model|path>    Noise model name or .toml file with custom parameters
//!   --noise-method <m>      Noise method: density-matrix, trajectory (auto if omitted)
//!   --print-final-state     Dump all non-zero registers and memory after execution
//!   --psw                   Print the Program State Word
//!   --resources             Print cumulative resource usage counters
//!   --verbose               Print config and execution summary
//!   --version               Show version
//!   --help                  Show this help message
//! ```

use std::process;
use cqam_run::loader::load_program;
use cqam_run::runner::run_program_with_data;
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
    eprintln!("  --threads <n>         Default thread count for HFORK (1-256)");
    eprintln!("  --shots <n>           Number of shots for QPU-realistic sampling");
    eprintln!("  --noise <model|path>  Noise model name (superconducting, trapped-ion, neutral-atom,");
    eprintln!("                        photonic, spin) or path to .toml file with custom parameters");
    eprintln!("  --noise-method <m>    Noise method: density-matrix, trajectory (auto if omitted)");
    eprintln!("  --verbose             Print config and execution summary");
    eprintln!("  --version             Show version");
    eprintln!("  --help                Show this help message");
}

struct CliArgs {
    input: String,
    config_path: Option<String>,
    qubits: Option<u8>,
    max_cycles: Option<usize>,
    threads: Option<u16>,
    shots: Option<u32>,
    density_matrix: bool,
    print_state: bool,
    print_psw: bool,
    print_resources: bool,
    verbose: bool,
    noise: Option<String>,
    noise_method: Option<String>,
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
    let mut threads: Option<u16> = None;
    let mut shots: Option<u32> = None;
    let mut density_matrix = false;
    let mut print_state = false;
    let mut print_psw = false;
    let mut print_resources = false;
    let mut verbose = false;
    let mut noise: Option<String> = None;
    let mut noise_method: Option<String> = None;

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
            "--threads" => {
                i += 1;
                let n: u16 = args.get(i)
                    .ok_or("--threads requires a number")?
                    .parse()
                    .map_err(|_| "--threads must be a number 1-256")?;
                if n == 0 || n > 256 {
                    return Err("--threads must be 1-256".to_string());
                }
                threads = Some(n);
            }
            "--shots" => {
                i += 1;
                let n: u32 = args.get(i)
                    .ok_or("--shots requires a number")?
                    .parse()
                    .map_err(|_| "--shots must be a positive integer")?;
                if n == 0 {
                    return Err("--shots must be >= 1".to_string());
                }
                shots = Some(n);
            }
            "--noise" => {
                i += 1;
                noise = Some(args.get(i).ok_or("--noise requires a model name")?.clone());
            }
            "--noise-method" => {
                i += 1;
                noise_method = Some(args.get(i)
                    .ok_or("--noise-method requires 'density-matrix' or 'trajectory'")?
                    .clone());
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
        threads,
        shots,
        density_matrix,
        print_state,
        print_psw,
        print_resources,
        verbose,
        noise,
        noise_method,
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
    if let Some(n) = cli.threads {
        config.default_threads = Some(n);
    }
    if let Some(n) = cli.shots {
        config.shots = Some(n);
    }
    if let Some(ref noise_name) = cli.noise {
        config.noise_model = Some(noise_name.clone());
    }
    if let Some(ref method) = cli.noise_method {
        config.noise_method = Some(method.clone());
    }

    // D-03: warn if --noise-method given without --noise
    if config.noise_method.is_some()
        && (config.noise_model.is_none() || config.noise_model.as_deref() == Some("none"))
    {
        eprintln!("warning: --noise-method has no effect without --noise <model>");
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

    let result = match run_program_with_data(
        parsed.instructions, &config, &parsed.metadata, &parsed.data_section,
        &parsed.shared_section, &parsed.private_section,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Runtime error: {}", e);
            process::exit(1);
        }
    };

    if cli.verbose {
        let ctx = result.ctx();
        eprintln!("Execution complete (PC={}, halted={})", ctx.pc, ctx.psw.trap_halt);
    }

    print_report(&result, cli.print_state, cli.print_psw, cli.print_resources);
}
