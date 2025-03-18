use std::env;
use std::fs;
use cqam_codegen::qasm::OpenQASMEmitter;
use cqam_codegen::emitter::QASMEmitter;
use cqam_core::instruction::Instruction;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 && (args[1] == "--help" || args[1] == "-h") {
        println!("cqam2qasm - CQAM to OpenQASM 3.0 emitter");
        println!("Usage: cqam2qasm <input.cqam> <output.qasm>");
        println!("  <input.cqam>   Path to CQAM instruction file");
        println!("  <output.qasm>  Path to write OpenQASM 3.0 file");
        std::process::exit(0);
    }

    if args.len() < 3 {
        eprintln!("Usage: cqam2qasm <input.cqam> <output.qasm>");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    let program_text = fs::read_to_string(input_path)
        .expect("Failed to read CQAM program file.");

    // Mock parse: Convert each line to a dummy Instruction
    let instructions: Vec<Instruction> = program_text
        .lines()
        .filter_map(|line| parse_mock_instruction(line.trim()))
        .collect();

    let emitter = OpenQASMEmitter;
    let qasm_output = emitter.emit_program(&instructions);

    fs::write(output_path, qasm_output).expect("Failed to write QASM file.");
}

fn parse_mock_instruction(line: &str) -> Option<Instruction> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "QPREP" => Some(Instruction::QPrep {
            dst: parts.get(1)?.to_string(),
            dist_src: "".into()
        }),
        "QKERNEL" => Some(Instruction::QKernel {
            dst: parts.get(1)?.to_string(),
            src: parts.get(2)?.to_string(),
            kernel: parts.get(3)?.to_string(),
            ctx: None
        }),
        "QMEAS" => Some(Instruction::QMeas {
            dst: parts.get(1)?.to_string(),
            src: parts.get(2)?.to_string()
        }),
        "CLADD" => Some(Instruction::ClAdd {
            dst: parts.get(1)?.to_string(),
            lhs: parts.get(2)?.to_string(),
            rhs: parts.get(3)?.to_string()
        }),
        "CLSUB" => Some(Instruction::ClSub {
            dst: parts.get(1)?.to_string(),
            lhs: parts.get(2)?.to_string(),
            rhs: parts.get(3)?.to_string()
        }),
        "CLLOAD" => Some(Instruction::ClLoad {
            dst: parts.get(1)?.to_string(),
            src: parts.get(2)?.to_string()
        }),
        "CLSTORE" => Some(Instruction::ClStore {
            addr: parts.get(1)?.to_string(),
            src: parts.get(2)?.to_string()
        }),
        "CLJUMP" => Some(Instruction::ClJump {
            label: parts.get(1)?.to_string()
        }),
        "CLIF" => Some(Instruction::ClIf {
            pred: parts.get(1)?.to_string(),
            label: parts.get(2)?.to_string()
        }),
        "HYBFORK" => Some(Instruction::HybFork),
        "HYBMERGE" => Some(Instruction::HybMerge),
        "HYBCONDEXEC" => Some(Instruction::HybCondExec {
            flag: parts.get(1)?.to_string(),
            then_label: parts.get(2)?.to_string()
        }),
        "HYBREDUCE" => Some(Instruction::HybReduce {
            src: parts.get(1)?.to_string(),
            dst: parts.get(2)?.to_string(),
            function: parts.get(3)?.to_string()
        }),
        _ => Some(Instruction::Label(line.to_string()))
    }
}

