use std::env;
use std::fs;
use std::io::{self, Read};
use cqam_core::instruction::Instruction;
use cqam_core::parser::parse_instruction;
use cqam2qasm::qasm::emit_qasm_program;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.contains(&"--help".to_string()) || args.len() < 2 {
        println!("Usage: cqam2qasm <input.cqam> [output.qasm] [--emit]");
        println!("  If no input file is provided, reads from stdin.");
        return;
    }

    let input_path = &args[1];
    let use_stdout = args.iter().any(|arg| arg == "--emit");

    // Load input source: file or stdin
    let input_content = if input_path == "-" {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer).expect("Failed to read from stdin");
        buffer
    } else {
        fs::read_to_string(input_path).expect("Failed to read input file")
    };

    let instructions: Vec<Instruction> = input_content
        .lines()
        .map(|line| parse_instruction(line))
        .collect();

    let output = emit_qasm_program(&instructions);

    if use_stdout || args.get(2).is_none() {
        println!("{}", output);
    } else {
        let output_path = &args[2];
        fs::write(output_path, output).expect("Failed to write output file");
        println!("QASM file written to: {}", output_path);
    }
}
