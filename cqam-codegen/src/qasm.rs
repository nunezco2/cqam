use cqam_core::instruction::Instruction;
use std::collections::HashSet;

/// Trait for converting CQAM instructions into OpenQASM 3.0 strings
pub trait QasmFormat {
    fn to_qasm(&self) -> Option<String>;
    fn emit_qasm_functions(&self) -> Option<String> {
        None
    }
}

impl QasmFormat for Instruction {
    fn to_qasm(&self) -> Option<String> {
        match self {
            Instruction::ClLoad { dst, src } => Some(format!("// CL:LOAD {}, {}\n    let {} = {};", dst, src, dst, src)),
            Instruction::ClAdd { dst, lhs, rhs } => Some(format!("// CL:ADD {}, {}, {}\n    let {} = {} + {};", dst, lhs, rhs, dst, lhs, rhs)),
            Instruction::ClSub { dst, lhs, rhs } => Some(format!("// CL:SUB {}, {}, {}\n    let {} = {} - {};", dst, lhs, rhs, dst, lhs, rhs)),
            Instruction::ClStore { addr, src } => Some(format!("// CL:STORE {}, {}\n    {} = {};", addr, src, addr, src)),
            Instruction::ClJump { label } => Some(format!("// CL:JMP {}", label)),
            Instruction::ClIf { pred, label } => Some(format!("// CL:IF {}, {}\n    if ({}) {{ goto {}; }}", pred, label, pred, label)),
            Instruction::Label(name) => Some(format!("// LABEL: {}", name)),
            Instruction::HybFork => Some("// HYB: fork".to_string()),
            Instruction::HybMerge => Some("// HYB: merge".to_string()),
            Instruction::HybCondExec { flag, then_label } => Some(format!("// HYB:COND_EXEC {}, {}", flag, then_label)),
            Instruction::HybReduce { src, dst, function } => Some(format!("// HYB:REDUCE {}, {}, {}\n    let {} = {}({});", src, dst, function, dst, function, src)),
            Instruction::QPrep { dst, dist_src } => Some(format!("// QPREP: {} from {}", dst, dist_src)),
            Instruction::QKernel { dst, src, kernel, ctx } => {
                let call = format!("{} = {}({});", dst, kernel, src);
                if let Some(c) = ctx {
                    Some(format!("// QKERNEL: {} = {}({}) in context {}\n    {}", dst, kernel, src, c, call))
                } else {
                    Some(format!("// QKERNEL: {} = {}({})\n    {}", dst, kernel, src, call))
                }
            }
            Instruction::QMeas { dst, src } => Some(format!("// QMEAS {}, {}\n    {} = measure {}[0];", dst, src, dst, src)),
            Instruction::QObserve { dst, src } => Some(format!("// QOBSERVE {}, {}", dst, src)),
            Instruction::Halt => Some("// HALT".to_string()),
            Instruction::NoOp => None,
        }
    }

    fn emit_qasm_functions(&self) -> Option<String> {
        if let Instruction::QKernel { kernel, .. } = self {
            let mut func = String::new();
            func.push_str(&format!("def {}(qbit x) {{\n", kernel));
            func.push_str("    // ... kernel logic here\n");
            func.push_str("}\n");
            return Some(func);
        }
        None
    }
}

/// Emit a full OpenQASM 3.0 program from a CQAM program
pub fn emit_qasm_program(program: &[Instruction]) -> String {
    let mut lines = vec![];
    let mut emitted_funcs = HashSet::new();

    lines.push("OPENQASM 3.0;\n".to_string());
    lines.push("// === BEGIN CQAM GENERATED QASM ===".to_string());
    lines.push("".to_string());

    // Function block header
    for instr in program {
        if let Some(func) = instr.emit_qasm_functions() {
            let key = match instr {
                Instruction::QKernel { kernel, .. } => kernel,
                _ => continue,
            };
            if emitted_funcs.insert(key.clone()) {
                lines.push(func.trim_end().to_string());
                lines.push("".to_string());
            }
        }
    }

    lines.push("// === MAIN PROGRAM ===".to_string());
    lines.push("".to_string());

    for instr in program {
        if let Some(block) = instr.to_qasm() {
            for line in block.lines() {
                lines.push(line.trim_end().to_string());
            }
            lines.push("".to_string());
        }
    }

    lines.push("// === END CQAM GENERATED QASM ===".to_string());
    lines.join("\n")
}
