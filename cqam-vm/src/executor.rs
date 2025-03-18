// cqam-vm/src/executor.rs

use crate::context::ExecutionContext;
use cqam_core::instruction::Instruction;
use cqam_core::register::{CValue};

/// Parse a line of CQAM assembly into an Instruction (temporary stub parser)
pub fn parse_instruction(line: &str) -> Instruction {
    if line.trim().starts_with("CL:LOAD") {
        // Example: CL:LOAD R1, 42
        let tokens: Vec<&str> = line.trim().split_whitespace().collect();
        if tokens.len() >= 3 {
            let dst = tokens[1].trim_end_matches(',').to_string();
            let src = tokens[2].to_string();
            return Instruction::ClLoad { dst, src };
        }
    }

    Instruction::NoOp // Default fallback
}

/// Execute a single instruction (with optional input override)
pub fn execute_instruction(ctx: &mut ExecutionContext, instr: Instruction) {
    match instr {
        Instruction::ClLoad { dst, src } => {
            let value = parse_cvalue(&src);
            ctx.registers.store_c(&dst, value);
        }
        Instruction::ClAdd { dst, lhs, rhs } => {
            let lv = ctx.registers.load_c(&lhs).cloned();
            let rv = ctx.registers.load_c(&rhs).cloned();
            let result = match (lv, rv) {
                (Some(CValue::Int(a)), Some(CValue::Int(b))) => CValue::Int(a + b),
                (Some(CValue::Float(a)), Some(CValue::Float(b))) => CValue::Float(a + b),
                _ => panic!("Type mismatch or missing operands in ClAdd"),
            };
            ctx.registers.store_c(&dst, result);
        }
        Instruction::ClSub { dst, lhs, rhs } => {
            let lv = ctx.registers.load_c(&lhs).cloned();
            let rv = ctx.registers.load_c(&rhs).cloned();
            let result = match (lv, rv) {
                (Some(CValue::Int(a)), Some(CValue::Int(b))) => CValue::Int(a - b),
                (Some(CValue::Float(a)), Some(CValue::Float(b))) => CValue::Float(a - b),
                _ => panic!("Type mismatch or missing operands in ClSub"),
            };
            ctx.registers.store_c(&dst, result);
        }
        Instruction::ClStore { addr, src } => {
            if let Some(CValue::Int(val)) = ctx.registers.load_c(&src) {
                ctx.cmem.store(&addr, *val);
            } else {
                panic!("ClStore expects an integer source value");
            }
        }
        Instruction::ClJump { label } => {
            ctx.pc = resolve_label(&label, &ctx.program);
            return;
        }
        Instruction::ClIf { pred, label } => {
            if let Some(CValue::Bool(true)) = ctx.registers.load_c(&pred) {
                ctx.pc = resolve_label(&label, &ctx.program);
                return;
            }
        }
        _ => {
            println!("Unhandled or NoOp: {:?}", instr);
        }
    }

    ctx.advance_pc();
}

/// Run a full program by parsing and executing each line
pub fn run_program(ctx: &mut ExecutionContext) {
    while let Some(line) = ctx.current_line() {
        let instr = parse_instruction(line);
        execute_instruction(ctx, instr);
    }
}

/// Helper: parse literal value string into CValue
fn parse_cvalue(src: &str) -> CValue {
    if let Ok(i) = src.parse::<i64>() {
        CValue::Int(i)
    } else if let Ok(f) = src.parse::<f64>() {
        CValue::Float(f)
    } else if src == "true" || src == "false" {
        CValue::Bool(src == "true")
    } else {
        CValue::Str(src.to_string())
    }
}

/// Helper: find index of a label line
fn resolve_label(label: &str, program: &[String]) -> usize {
    program.iter().position(|line| line.trim() == format!("LABEL: {}", label)).unwrap_or(0)
}
