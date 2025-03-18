use std::env;
use cqam_run::loader::load_program;
use cqam_run::runner::run_program;
use cqam_run::report::print_report;
use cqam_run::simconfig::SimConfig;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args.contains(&"--help".to_string()) {
        println!("Usage: cqam-run --input <file> [--print-final-state] [--psw-report] [--resource-usage]");
        return;
    }

    let input_path = args.iter().position(|a| a == "--input")
        .and_then(|i| args.get(i + 1))
        .expect("Missing --input <path>");

    let print_state = args.contains(&"--print-final-state".to_string());
    let print_psw = args.contains(&"--psw-report".to_string());
    let print_resource = args.contains(&"--resource-usage".to_string());

    let config_path = args.iter()
        .position(|a| a == "--config")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "example_config.toml".to_string());
    let config = SimConfig::load(&config_path);
    println!("Loaded simulator config: {:?}", config);

    let program = load_program(input_path);
    let ctx = run_program(program);

    print_report(&ctx, print_state, print_psw, print_resource);
}
