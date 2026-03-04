use std::env;
use std::process;
use cqam_run::loader::load_program;
use cqam_run::runner::run_program_with_config;
use cqam_run::report::print_report;
use cqam_run::simconfig::SimConfig;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args.contains(&"--help".to_string()) {
        println!("Usage: cqam-run --input <file> [--print-final-state] [--psw-report] [--resource-usage]");
        return;
    }

    let input_path = match args.iter().position(|a| a == "--input").and_then(|i| args.get(i + 1)) {
        Some(path) => path.clone(),
        None => {
            eprintln!("Error: Missing --input <path>");
            process::exit(1);
        }
    };

    let print_state = args.contains(&"--print-final-state".to_string());
    let print_psw = args.contains(&"--psw-report".to_string());
    let print_resource = args.contains(&"--resource-usage".to_string());

    let config_path = args.iter()
        .position(|a| a == "--config")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "example_config.toml".to_string());

    let config = match SimConfig::load(&config_path) {
        Ok(cfg) => {
            println!("Loaded simulator config: {:?}", cfg);
            cfg
        }
        Err(e) => {
            eprintln!("Warning: Could not load config ({}), using defaults", e);
            SimConfig::default()
        }
    };
    let program = match load_program(&input_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error loading program: {}", e);
            process::exit(1);
        }
    };

    let ctx = match run_program_with_config(program, &config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Runtime error: {}", e);
            process::exit(1);
        }
    };

    print_report(&ctx, print_state, print_psw, print_resource);
}
