use std::fs;
use std::path::Path;
use std::collections::HashSet;
use cqam_core::instruction::Instruction;


/// Trait for converting CQAM instructions into OpenQASM 3.0 strings
pub trait QasmFormat {
    fn to_qasm(&self, expand_templates: bool) -> Option<String>;
    fn emit_qasm_functions(&self) -> Option<String> {
        None
    }
}

impl QasmFormat for Instruction {
    fn to_qasm(&self, expand_templates: bool) -> Option<String> {
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
            Instruction::HybCondExec { flag, then_label } => Some(format!("// HYB:COND_EXEC {}, {}\n    if ({}) {{ goto {}; }}", flag, then_label, flag, then_label)),
            Instruction::HybReduce { src, dst, function } => Some(format!("// HYB:REDUCE {}, {}, {}\n    let {} = {}({});", src, dst, function, dst, function, src)),
            Instruction::QPrep { dst, dist_src } => Some(format!("// QPREP: {} from {}", dst, dist_src)),
            Instruction::QKernel { dst, src, kernel, ctx } => {
                let call = format!("{} = {}({});", dst, kernel, src);
                let header = if let Some(c) = ctx {
                    format!("// QKERNEL: {} = {}({}) in context {}", dst, kernel, src, c)
                } else {
                    format!("// QKERNEL: {} = {}({})", dst, kernel, src)
                };

                if expand_templates {
                    let template_path = format!("kernels/qasm_templates/{}.qasm", kernel);
                    let template_code = fs::read_to_string(Path::new(&template_path)).unwrap_or_else(|_| "// [Missing QASM template]".to_string());
                    Some(format!("{}\n{}", header, template_code))
                } else {
                    Some(format!("{}\n    {}", header, call))
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
pub fn emit_qasm_program(program: &[Instruction], expand_templates: bool) -> String {
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
        if let Some(block) = instr.to_qasm(expand_templates) {
            for line in block.lines() {
                lines.push(line.trim_end().to_string());
            }
            lines.push("".to_string());
        }
    }

    lines.push("// === END CQAM GENERATED QASM ===".to_string());
    lines.join("\n")
}
