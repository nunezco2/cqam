use cqam_core::instruction::Instruction;
use cqam_core::register::CValue;
use crate::context::ExecutionContext;


pub fn is_label(instr: &Instruction, label: &str) -> bool {
    matches!(instr, Instruction::Label(l) if l == label)
}

pub fn execute_hybrid(ctx: &mut ExecutionContext, instr: Instruction) {
    match instr {
        Instruction::HybFork => {
            ctx.psw.hf = true;
            ctx.psw.forked = true;
            println!("HYB: Forked hybrid control path.");
        }

        Instruction::HybMerge => {
            ctx.psw.hf = true;
            ctx.psw.merged = true;
            println!("HYB: Merged hybrid control paths.");
        }

        Instruction::HybCondExec { flag, then_label, .. } => {
            let cond = match flag.as_str() {
                "ZF" => ctx.psw.zf,
                "NF" => ctx.psw.nf,
                "QF" => ctx.psw.qf,
                "HF" => ctx.psw.hf,
                "PF" => ctx.psw.pf,
                _ => false,
            };

            ctx.psw.update_from_predicate(cond);
            if cond {
                if let Some(target) = ctx.program.iter().position(|instr| {
                    matches!(instr, Instruction::Label(l) if *l == then_label)
                }) {
                    ctx.pc = target;
                }
            }
        }

        Instruction::HybReduce { src, dst, function } => {
            if let Some(CValue::Float(x)) = ctx.registers.load_c(&src) {
                let reduced = match function.as_str() {
                    "round" => CValue::Int(x.round() as i64),
                    "floor" => CValue::Int(x.floor() as i64),
                    "trunc" => CValue::Int(x.trunc() as i64),
                    "ceil"  => CValue::Int(x.ceil() as i64),
                    "abs"   => CValue::Int(x.abs() as i64),
                    "negate" => CValue::Int((-x) as i64),
                    _ => CValue::Int(x.round() as i64), // fallback
                };
                ctx.registers.store_c(&dst, reduced);
                println!("HYB: Reduced {} to {} using {}.", src, dst, function);
            }
        }

        _ => {
            println!("Unhandled hybrid instruction: {:?}", instr);
        }
    }

    ctx.advance_pc();
}
