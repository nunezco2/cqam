use crate::context::ExecutionContext;
use crate::resource::ResourceDelta;
use cqam_core::instruction::Instruction;
use cqam_core::register::CValue;

/// Execute a single instruction
pub fn execute_instruction(ctx: &mut ExecutionContext, instr: Instruction) {
    match &instr {
        Instruction::ClLoad { dst, src } => {
            let value = parse_cvalue(src);
            ctx.registers.store_c(dst, value);
        }
        Instruction::ClAdd { dst, lhs, rhs } => {
            let lv = ctx.registers.load_c(lhs).cloned();
            let rv = ctx.registers.load_c(rhs).cloned();
            let result = match (lv, rv) {
                (Some(CValue::Int(a)), Some(CValue::Int(b))) => CValue::Int(a + b),
                (Some(CValue::Float(a)), Some(CValue::Float(b))) => CValue::Float(a + b),
                _ => panic!("Type mismatch or missing operands in ClAdd"),
            };
            ctx.registers.store_c(dst, result);
        }
        Instruction::ClSub { dst, lhs, rhs } => {
            let lv = ctx.registers.load_c(lhs).cloned();
            let rv = ctx.registers.load_c(rhs).cloned();
            let result = match (lv, rv) {
                (Some(CValue::Int(a)), Some(CValue::Int(b))) => CValue::Int(a - b),
                (Some(CValue::Float(a)), Some(CValue::Float(b))) => CValue::Float(a - b),
                _ => panic!("Type mismatch or missing operands in ClSub"),
            };
            ctx.registers.store_c(dst, result);
        }
        Instruction::ClStore { addr, src } => {
            if let Some(CValue::Int(val)) = ctx.registers.load_c(src) {
                ctx.cmem.store(addr, *val);
            } else {
                panic!("ClStore expects an integer source value");
            }
        }
        Instruction::ClJump { label } => {
            ctx.pc = resolve_label(label, &ctx.program);
            return;
        }
        Instruction::ClIf { pred, label } => {
            if let Some(CValue::Bool(true)) = ctx.registers.load_c(pred) {
                ctx.pc = resolve_label(label, &ctx.program);
                return;
            }
        }
        _ => {
            println!("Unhandled or NoOp: {:?}", instr);
        }
    }

    // Apply resource delta
    let delta = default_resource_for(&instr);
    ctx.resource_tracker.apply_delta(&delta);
    ctx.advance_pc();
}

/// Run a full program assuming Vec<Instruction> in context
pub fn run_program(ctx: &mut ExecutionContext) {
    while let Some(instr) = ctx.current_line() {
        execute_instruction(ctx, instr.clone());
    }
}

/// Resource usage delta per instruction
pub fn default_resource_for(instr: &Instruction) -> ResourceDelta {
    match instr {
        Instruction::ClAdd { .. } => ResourceDelta { time: 1, space: 1, ..Default::default() },
        Instruction::ClSub { .. } => ResourceDelta { time: 1, space: 1, ..Default::default() },
        Instruction::ClLoad { .. } => ResourceDelta { time: 1, space: 1, ..Default::default() },
        Instruction::ClStore { .. } => ResourceDelta { time: 1, space: 1, ..Default::default() },
        Instruction::ClJump { .. } => ResourceDelta { time: 1, space: 0, ..Default::default() },
        Instruction::ClIf { .. } => ResourceDelta { time: 1, space: 0, ..Default::default() },
        Instruction::QPrep { .. } => ResourceDelta { time: 2, space: 2, superposition: 1.0, ..Default::default() },
        Instruction::QKernel { .. } => ResourceDelta { time: 3, space: 2, superposition: 0.5, entanglement: 0.7, ..Default::default() },
        Instruction::QMeas { .. } => ResourceDelta { time: 1, space: 1, ..Default::default() },
        Instruction::QObserve { .. } => ResourceDelta { time: 1, space: 1, interference: 0.3, ..Default::default() },
        Instruction::HybFork => ResourceDelta { time: 1, space: 0, ..Default::default() },
        Instruction::HybMerge => ResourceDelta { time: 1, space: 0, ..Default::default() },
        Instruction::HybCondExec { .. } => ResourceDelta { time: 1, space: 0, ..Default::default() },
        Instruction::HybReduce { function, .. } => match function.as_str() {
            "round" | "floor" | "trunc" => ResourceDelta { time: 1, space: 1, ..Default::default() },
            "ceil" | "abs" => ResourceDelta { time: 2, space: 1, ..Default::default() },
            "negate" => ResourceDelta { time: 2, space: 1, interference: 0.2, ..Default::default() },
            _ => ResourceDelta { time: 2, space: 1, ..Default::default() },
        },
        _ => ResourceDelta::default(),
    }
}

/// Helper: parse literal string to CValue
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

/// Resolve label in Vec<Instruction>
pub fn resolve_label(label: &str, program: &[Instruction]) -> usize {
    program.iter().position(|instr| matches!(instr, Instruction::Label(l) if l == label)).unwrap_or(0)
}
