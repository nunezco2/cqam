//! CLI tool: converts a `.cqam` source file to OpenQASM 3.0.

use std::env;
use std::fs;
use std::process;

use cqam_codegen::qasm::{EmitConfig, EmitMode, emit_qasm_program};
use cqam_core::parser::parse_program;

fn print_help() {
    eprintln!("Usage: cqam2qasm <input.cqam> [OPTIONS]");
    eprintln!();
    eprintln!("Convert a CQAM assembly file to OpenQASM 3.0.");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o <file>        Output file path (default: stdout)");
    eprintln!("  --fragment       Emit body only (no header, declarations, gate stubs)");
    eprintln!("  --expand         Enable kernel template expansion");
    eprintln!("  --no-expand      Disable kernel template expansion");
    eprintln!("  --doc            Print CQAM instruction reference");
    eprintln!("  --version        Show tool version");
    eprintln!("  --help           Show this help message");
}

fn print_version() {
    eprintln!("cqam2qasm {}", env!("CARGO_PKG_VERSION"));
}

/// Print the updated ISA reference for the flat-prefix syntax.
fn print_doc_reference() {
    eprintln!("CQAM Instruction Reference:\n");
    eprintln!("  Integer arithmetic:   IADD  ISUB  IMUL  IDIV  IMOD");
    eprintln!("  Integer bitwise:      IAND  IOR   IXOR  INOT  ISHL  ISHR");
    eprintln!("  Integer memory:       ILDI  ILDM  ISTR  ISTRX  ILDX");
    eprintln!("  Integer comparison:   IEQ   ILT   IGT");
    eprintln!();
    eprintln!("  Float arithmetic:     FADD  FSUB  FMUL  FDIV");
    eprintln!("  Float transcendental: FSIN  FCOS  FATAN2  FSQRT");
    eprintln!("  Float memory:         FLDI  FLDM  FSTR");
    eprintln!("  Float comparison:     FEQ   FLT   FGT");
    eprintln!();
    eprintln!("  Complex arithmetic:   ZADD  ZSUB  ZMUL  ZDIV");
    eprintln!("  Complex memory:       ZLDI  ZLDM  ZSTR");
    eprintln!();
    eprintln!("  Type conversion:      CVTIF  CVTFI  CVTFZ  CVTZF");
    eprintln!();
    eprintln!("  Control flow:         JMP   JIF   CALL  RET   HALT  NOP");
    eprintln!("                        LABEL (pseudo-instruction)");
    eprintln!();
    eprintln!("  Interrupts:           SETIV  RETI");
    eprintln!();
    eprintln!("  Quantum preparation:  QPREP  QPREPN  QMIXED");
    eprintln!("  Quantum gates:        QHADM  QPHASE  QFLIP  QROT");
    eprintln!("  Quantum two-qubit:    QCNOT  QCZ  QSWAP  QTENSOR");
    eprintln!("  Quantum kernels:      QKERNEL  QKERNELF  QKERNELZ  QCUSTOM");
    eprintln!("  Quantum measurement:  QSAMPLE  QOBSERVE  QMEAS");
    eprintln!("  Quantum memory:       QLOAD  QSTORE  QPTRACE  QRESET");
    eprintln!();
    eprintln!("  Hybrid:               HFORK  HMERGE  JMPF  HREDUCE");
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
    let output_path = args.iter().position(|a| a == "-o" || a == "--out").and_then(|i| args.get(i + 1));

    let input = match fs::read_to_string(&input_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading input file: {}", e);
            process::exit(1);
        }
    };

    let program = match parse_program(&input) {
        Ok(p) => p.instructions,
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
