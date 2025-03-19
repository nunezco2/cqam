// cqam-codegen/src/qasm.rs

use cqam_core::instruction::Instruction;

/// Trait for converting CQAM instructions into OpenQASM 3.0 strings
pub trait QasmFormat {
    fn to_qasm(&self) -> Option<String>;
}

impl QasmFormat for Instruction {
    fn to_qasm(&self) -> Option<String> {
        match self {
            Instruction::ClLoad { dst, src } => {
                Some(format!("let {} = {};", dst, src))
            }
            Instruction::ClAdd { dst, lhs, rhs } => {
                Some(format!("let {} = {} + {};", dst, lhs, rhs))
            }
            Instruction::ClSub { dst, lhs, rhs } => {
                Some(format!("let {} = {} - {};", dst, lhs, rhs))
            }
            Instruction::ClStore { addr, src } => {
                Some(format!("// store {} -> {}", src, addr))
            }
            Instruction::ClJump { label } => {
                Some(format!("// jump to {}", label))
            }
            Instruction::ClIf { pred, label } => {
                Some(format!("if ({}) {{ // jump {} }}", pred, label))
            }
            Instruction::Label(name) => {
                Some(format!("// LABEL: {}", name))
            }
            Instruction::HybFork => Some("// HYB: fork".to_string()),
            Instruction::HybMerge => Some("// HYB: merge".to_string()),
            Instruction::HybCondExec { flag, then_label } => {
                Some(format!("// HYB: if {} -> {}", flag, then_label))
            }
            Instruction::HybReduce { src, dst, function } => {
                Some(format!("// HYB: reduce {} -> {} via {}", src, dst, function))
            }
            Instruction::QPrep { dst, dist_src } => {
                Some(format!("// QPREP: {} from {}", dst, dist_src))
            }
            Instruction::QKernel { dst, src, kernel, ctx } => {
                if let Some(c) = ctx {
                    Some(format!("// QKERNEL: {} = {}({}) in context {}", dst, kernel, src, c))
                } else {
                    Some(format!("// QKERNEL: {} = {}({})", dst, kernel, src))
                }
            }
            Instruction::QMeas { dst, src } => {
                Some(format!("// QMEAS: {} = measure({})", dst, src))
            }
            Instruction::QObserve { dst, src } => {
                Some(format!("// QOBSERVE: {} = observe({})", dst, src))
            }
            Instruction::NoOp => Some("// NO-OP".to_string()),
            _ => Some("// Unhandled instruction".to_string()),
        }
    }
}

pub fn emit_qasm_program(program: &[Instruction]) -> String {
    let mut lines = vec![];
    lines.push("OPENQASM 3.0;".to_string());
    lines.push("// --- BEGIN CQAM GENERATED QASM ---".to_string());
    for instr in program {
        if let Some(line) = instr.to_qasm() {
            lines.push(line);
        }
    }
    lines.push("// --- END CQAM GENERATED QASM ---".to_string());
    lines.join("\n")
}
