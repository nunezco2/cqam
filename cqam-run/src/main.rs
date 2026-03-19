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
use cqam_run::simconfig::{BackendChoice, SimConfig};

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
    eprintln!("  --backend <choice>    Backend: simulation (default), mock, ibm");
    eprintln!("  --qpu-shots <n>       Shot budget for QPU backends (default: 8192)");
    eprintln!("  --qpu-confidence <f>  Bayesian confidence level 0.0-1.0 (default: 0.95)");
    eprintln!("  --qpu-device <name>   QPU device name (provider-specific)");
    eprintln!("  --ibm-token <TOKEN>               IBM Quantum API token");
    eprintln!("  --ibm-optimization-level <N>      Qiskit transpiler optimization level (0-3) [default: 1]");
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
    backend: Option<String>,
    qpu_shots: Option<u32>,
    qpu_confidence: Option<f64>,
    qpu_device: Option<String>,
    ibm_token: Option<String>,
    ibm_optimization_level: Option<u8>,
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
    let mut backend: Option<String> = None;
    let mut qpu_shots: Option<u32> = None;
    let mut qpu_confidence: Option<f64> = None;
    let mut qpu_device: Option<String> = None;
    let mut ibm_token: Option<String> = None;
    let mut ibm_optimization_level: Option<u8> = None;

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
            "--backend" => {
                i += 1;
                let val = args.get(i).ok_or("--backend requires a value")?;
                match val.as_str() {
                    "simulation" | "mock" | "ibm" => backend = Some(val.clone()),
                    other => return Err(format!(
                        "unknown backend: '{}'. Valid: simulation, mock, ibm", other
                    )),
                }
            }
            "--qpu-shots" => {
                i += 1;
                let n: u32 = args.get(i)
                    .ok_or("--qpu-shots requires a number")?
                    .parse()
                    .map_err(|_| "--qpu-shots must be a positive integer")?;
                if n == 0 { return Err("--qpu-shots must be >= 1".to_string()); }
                qpu_shots = Some(n);
            }
            "--qpu-confidence" => {
                i += 1;
                let f: f64 = args.get(i)
                    .ok_or("--qpu-confidence requires a number")?
                    .parse()
                    .map_err(|_| "--qpu-confidence must be a float 0.0-1.0")?;
                if !(0.0..=1.0).contains(&f) {
                    return Err("--qpu-confidence must be between 0.0 and 1.0".to_string());
                }
                qpu_confidence = Some(f);
            }
            "--qpu-device" => {
                i += 1;
                qpu_device = Some(args.get(i).ok_or("--qpu-device requires a name")?.clone());
            }
            "--ibm-token" => {
                i += 1;
                ibm_token = Some(args.get(i).ok_or("--ibm-token requires a value")?.clone());
            }
            "--ibm-optimization-level" => {
                i += 1;
                let n: u8 = args.get(i)
                    .ok_or("--ibm-optimization-level requires a number 0-3")?
                    .parse()
                    .map_err(|_| "--ibm-optimization-level must be 0-3")?;
                if n > 3 {
                    eprintln!("warning: --ibm-optimization-level {} clamped to 3", n);
                }
                ibm_optimization_level = Some(n.min(3));
            }
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
        backend,
        qpu_shots,
        qpu_confidence,
        qpu_device,
        ibm_token,
        ibm_optimization_level,
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

    // Construct BackendChoice from CLI args and store in config
    let backend_choice = match cli.backend.as_deref() {
        None | Some("simulation") => BackendChoice::Simulation,
        Some("mock") => BackendChoice::Qpu {
            provider: "mock".to_string(),
            device: cli.qpu_device.clone(),
            shot_budget: cli.qpu_shots.unwrap_or(8192),
            confidence: cli.qpu_confidence.unwrap_or(0.95),
        },
        Some("ibm") => BackendChoice::Qpu {
            provider: "ibm".to_string(),
            device: cli.qpu_device.clone(),
            shot_budget: cli.qpu_shots.unwrap_or(4096),
            confidence: cli.qpu_confidence.unwrap_or(0.95),
        },
        Some(other) => {
            eprintln!("Error: unknown backend '{}'", other);
            process::exit(1);
        }
    };
    config.backend = Some(backend_choice);

    // Wire IBM-specific CLI args into SimConfig
    if let Some(ref token) = cli.ibm_token {
        config.ibm_token = Some(token.clone());
    }
    if let Some(level) = cli.ibm_optimization_level {
        config.ibm_optimization_level = Some(level);
    }

    // Incompatible-flag validation for QPU backends
    if !matches!(config.backend_choice(), BackendChoice::Simulation) {
        if config.noise_model.is_some() && config.noise_model.as_deref() != Some("none") {
            eprintln!("Error: --noise is not compatible with QPU backends. \
                       Noise injection is simulation-only.");
            process::exit(1);
        }
        if config.noise_method.is_some() {
            eprintln!("Error: --noise-method is not compatible with QPU backends. \
                       Noise injection is simulation-only.");
            process::exit(1);
        }
        if config.force_density_matrix {
            eprintln!("warning: --density-matrix has no effect with QPU backends \
                       (QPU mode has no density-matrix representation).");
        }
        if config.shots.is_some() {
            eprintln!("warning: --shots with QPU backend causes redundant shot sampling. \
                       QPU backends already produce shot-based histograms internally. \
                       Consider using --qpu-shots instead.");
        }
    }

    // D-03: warn if --noise-method given without --noise
    if matches!(config.backend_choice(), BackendChoice::Simulation)
        && config.noise_method.is_some()
        && (config.noise_model.is_none() || config.noise_model.as_deref() == Some("none"))
    {
        eprintln!("warning: --noise-method has no effect without --noise <model>");
    }

    if cli.verbose {
        // Redact ibm_token to prevent accidental token leakage in logs.
        let saved_token = config.ibm_token.take();
        if saved_token.is_some() {
            config.ibm_token = Some("***".to_string());
        }
        eprintln!("Config: {:?}", config);
        config.ibm_token = saved_token;
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
