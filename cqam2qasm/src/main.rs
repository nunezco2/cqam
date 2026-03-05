//! CLI tool: converts a `.cqam` source file to OpenQASM 3.0.

use std::env;
use std::fs;
use std::process;

use cqam_codegen::qasm::{EmitConfig, EmitMode, emit_qasm_program};
use cqam_core::parser::parse_program;

fn print_help() {
    println!("Usage: cqam2qasm <input_file.cqam> [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --out <file>     Specify output file path");
    println!("  --fragment       Emit body only (no header, declarations, gate stubs)");
    println!("  --expand         Enable kernel template expansion");
    println!("  --no-expand      Disable kernel template expansion");
    println!("  --doc            Print CQAM instruction reference");
    println!("  --version        Show tool version");
    println!("  --help           Show this help message");
}

fn print_version() {
    println!("cqam2qasm version 0.3.0");
}

/// Print the updated ISA reference for the flat-prefix syntax.
fn print_doc_reference() {
    println!("CQAM Instruction Reference:\n");
    println!("  Integer arithmetic:   IADD  ISUB  IMUL  IDIV  IMOD");
    println!("  Integer bitwise:      IAND  IOR   IXOR  INOT  ISHL  ISHR");
    println!("  Integer memory:       ILDI  ILDM  ISTR");
    println!("  Integer comparison:   IEQ   ILT   IGT");
    println!();
    println!("  Float arithmetic:     FADD  FSUB  FMUL  FDIV");
    println!("  Float memory:         FLDI  FLDM  FSTR");
    println!("  Float comparison:     FEQ   FLT   FGT");
    println!();
    println!("  Complex arithmetic:   ZADD  ZSUB  ZMUL  ZDIV");
    println!("  Complex memory:       ZLDI  ZLDM  ZSTR");
    println!();
    println!("  Type conversion:      CVTIF  CVTFI  CVTFZ  CVTZF");
    println!();
    println!("  Control flow:         JMP   JIF   CALL  RET   HALT");
    println!("                        LABEL (pseudo-instruction)");
    println!();
    println!("  Quantum:              QPREP  QKERNEL  QOBSERVE");
    println!("                        QLOAD  QSTORE");
    println!();
    println!("  Hybrid:               HFORK  HMERGE  HCEXEC  HREDUCE");
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

    let input = match fs::read_to_string(&input_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading input file: {}", e);
            process::exit(1);
        }
    };

    let program = match parse_program(&input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    // Build EmitConfig from CLI flags
    let is_fragment = args.contains(&"--fragment".to_string());
    let has_expand = args.contains(&"--expand".to_string());
    let has_no_expand = args.contains(&"--no-expand".to_string());

    let mode = if is_fragment {
        EmitMode::Fragment
    } else {
        EmitMode::Standalone
    };

    // Template expansion defaults:
    //   Standalone -> true (expand by default)
    //   Fragment   -> false (no expansion by default)
    // --expand / --no-expand override the default
    let expand_templates = has_expand || (!has_no_expand && !is_fragment);

    let config = EmitConfig {
        mode,
        expand_templates,
        template_dir: "kernels/qasm_templates".to_string(),
    };

    let output = emit_qasm_program(&program, &config);

    if let Some(out_path) = output_path {
        if let Err(e) = fs::write(out_path, output) {
            eprintln!("Error writing output file: {}", e);
            process::exit(1);
        }
    } else {
        println!("{}", output);
    }
}
