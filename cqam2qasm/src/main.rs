use std::env;
use std::fs;

use cqam_codegen::qasm::emit_qasm_program;
use cqam_core::parser::parse_instruction;

fn print_help() {
    println!("Usage: cqam2qasm <input_file.cqam> [--out output.qasm] [--doc] [--version]");
    println!("  --out <file>     Specify output file path");
    println!("  --doc            Print CQAM instruction reference");
    println!("  --version        Show tool version");
}

fn print_version() {
    println!("cqam2qasm version 0.1.0");
}

fn print_doc_reference() {
    println!("CQAM Instruction Reference:\n");
    println!("  CL:LOAD dst, src");
    println!("  CL:ADD dst, lhs, rhs");
    println!("  CL:SUB dst, lhs, rhs");
    println!("  CL:STORE addr, src");
    println!("  CL:JMP label");
    println!("  CL:IF pred, label");
    println!("  HYB:FORK, MERGE, COND_EXEC, REDUCE");
    println!("  QPREP dst, dist");
    println!("  QKERNEL dst, src, kernel");
    println!("  QMEAS dst, src");
    println!("  QOBSERVE dst, src");
    println!("  HALT");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) {
        print_help();
        return;
    }

    if args.contains(&"--version".to_string()) {
        print_version();
        return;
    }

    if args.contains(&"--doc".to_string()) {
        print_doc_reference();
        return;
    }

    let input_path = args[1].clone();
    let output_path = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1));

    let input = fs::read_to_string(&input_path).expect("Failed to read input file");
    let mut program = vec![];
    for line in input.lines() {
        program.push(parse_instruction(line));
    }

    let output = emit_qasm_program(&program);

    if let Some(out_path) = output_path {
        fs::write(out_path, output).expect("Failed to write output file");
    } else {
        println!("{}", output);
    }
}
