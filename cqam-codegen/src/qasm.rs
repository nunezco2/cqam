use cqam_core::instruction::Instruction;

/// Trait for converting CQAM instructions into OpenQASM 3.0 strings
pub trait QasmFormat {
    fn to_qasm(&self) -> Option<String>;
}

impl QasmFormat for Instruction {
    fn to_qasm(&self) -> Option<String> {
        match self {
            Instruction::ClLoad { dst, src } => Some(format!("// CL:LOAD {}, {}\nlet {} = {};", dst, src, dst, src)),
            Instruction::ClAdd { dst, lhs, rhs } => Some(format!("// CL:ADD {}, {}, {}\nlet {} = {} + {};", dst, lhs, rhs, dst, lhs, rhs)),
            Instruction::ClSub { dst, lhs, rhs } => Some(format!("// CL:SUB {}, {}, {}\nlet {} = {} - {};", dst, lhs, rhs, dst, lhs, rhs)),
            Instruction::ClStore { addr, src } => Some(format!("// CL:STORE {}, {}\n{} = {};", addr, src, addr, src)),
            Instruction::ClJump { label } => Some(format!("// CL:JMP {}", label)),
            Instruction::ClIf { pred, label } => Some(format!("// CL:IF {}, {}\nif ({}) {{ goto {}; }}", pred, label, pred, label)),
            Instruction::Label(name) => Some(format!("// LABEL: {}", name)),
            Instruction::HybFork => Some("// HYB: fork".to_string()),
            Instruction::HybMerge => Some("// HYB: merge".to_string()),
            Instruction::HybCondExec { flag, then_label } => Some(format!("// HYB:COND_EXEC {}, {}", flag, then_label)),
            Instruction::HybReduce { src, dst, function } => Some(format!("// HYB:REDUCE {}, {}, {}\nlet {} = {}({});", src, dst, function, dst, function, src)),
            Instruction::QPrep { dst, dist_src } => Some(format!("// QPREP: {} from {}", dst, dist_src)),
            Instruction::QKernel { dst, src, kernel, ctx } => {
                if let Some(c) = ctx {
                    Some(format!("// QKERNEL: {} = {}({}) in context {}", dst, kernel, src, c))
                } else {
                    Some(format!("// QKERNEL: {} = {}({})", dst, kernel, src))
                }
            }
            Instruction::QMeas { dst, src } => Some(format!("// QMEAS {}, {}\n{} = measure {}[0];", dst, src, dst, src)),
            Instruction::QObserve { dst, src } => Some(format!("// QOBSERVE {}, {}", dst, src)),
            Instruction::Halt => Some("// HALT".to_string()),
            Instruction::NoOp => None,
        }
    }
}

/// Emit a full OpenQASM 3.0 program from a CQAM program
pub fn emit_qasm_program(program: &[Instruction]) -> String {
    let mut lines = vec![];
    lines.push("OPENQASM 3.0;".to_string());
    lines.push("// --- BEGIN CQAM GENERATED QASM ---".to_string());

    // Static declarations (placeholder, can be refined later)
    lines.push("bit[1] m1;".to_string());
    lines.push("qubit[1] q1;".to_string());
    lines.push("int[32] R1;".to_string());
    lines.push("int[32] R2;".to_string());
    lines.push("int[32] result;".to_string());

    for instr in program {
        if let Some(line) = instr.to_qasm() {
            for l in line.lines() {
                lines.push(l.to_string());
            }
        }
    }

    lines.push("// --- END CQAM GENERATED QASM ---".to_string());
    lines.join("\n")
}
