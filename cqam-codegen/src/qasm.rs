use cqam_core::instruction::Instruction;
use crate::emitter::QASMEmitter;

pub struct OpenQASMEmitter;

impl QASMEmitter for OpenQASMEmitter {
    fn emit_header(&self) -> String {
        "OPENQASM 3.0;\ninclude \"stdgates.inc\";\n".to_string()
    }

    fn emit_register_declarations(&self) -> String {
        "qubit[4] q;\nbit[4] c;\n".to_string()
    }

    fn emit_instruction(&self, instr: &Instruction) -> Option<String> {
        match instr {
            // Quantum
            Instruction::QPrep { dst, .. } => Some(format!("reset q[{}];", dst)),
            Instruction::QKernel { dst, src, kernel, .. } => match kernel.as_str() {
                "entangle" => Some(format!("cx q[{}], q[{}];", src, dst)),
                _ => Some(format!("// [QKernel] {} → {} via '{}'", src, dst, kernel)),
            },
            Instruction::QMeas { dst, src } =>
                Some(format!("c[{}] = measure q[{}];", dst, src)),
    
            // Classical passthrough as comment
            Instruction::ClAdd { dst, lhs, rhs } =>
                Some(format!("// CL: {} = {} + {}", dst, lhs, rhs)),
            Instruction::ClSub { dst, lhs, rhs } =>
                Some(format!("// CL: {} = {} - {}", dst, lhs, rhs)),
            Instruction::ClLoad { dst, src } =>
                Some(format!("// CL: {} ← mem[{}]", dst, src)),
            Instruction::ClStore { addr, src } =>
                Some(format!("// CL: mem[{}] ← {}", addr, src)),
            Instruction::ClIf { pred, label } =>
                Some(format!("// CL: IF {} → jump {}", pred, label)),
            Instruction::ClJump { label } =>
                Some(format!("// CL: unconditional jump {}", label)),
    
            // Hybrid passthrough as comment
            Instruction::HybFork => Some("// HYB: Fork control path".to_string()),
            Instruction::HybMerge => Some("// HYB: Merge control paths".to_string()),
            Instruction::HybCondExec { flag, then_label, .. } =>
                Some(format!("// HYB: CondExec if {} → {}", flag, then_label)),
            Instruction::HybReduce { src, dst, function } =>
                Some(format!("// HYB: Reduce {} → {} using '{}'", src, dst, function)),
    
            // Label and Halt
            Instruction::Label(label) => Some(format!("// Label: {}", label)),
            Instruction::Halt => Some("// HALT".to_string()),
    
            _ => Some("// [Unrecognized instruction]".to_string()),
        }
    }    

    fn emit_program(&self, program: &[Instruction]) -> String {
        let mut output = self.emit_header();
        output.push_str(&self.emit_register_declarations());
        for instr in program {
            if let Some(line) = self.emit_instruction(instr) {
                output.push_str(&format!("{}\n", line));
            }
        }
        output
    }
}
