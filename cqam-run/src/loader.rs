use std::fs;
use cqam_core::instruction::Instruction;

pub fn load_program(path: &str) -> Vec<Instruction> {
    let content = fs::read_to_string(path).expect("Failed to read program file.");
    content.lines()
        .filter_map(|line| parse_line(line.trim()))
        .collect()
}

// Temporary simple parser (can be replaced with robust parser)
pub fn parse_line(line: &str) -> Option<Instruction> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() { return None; }

    match parts[0] {
        "QPREP" => Some(Instruction::QPrep { dst: parts[1].into(), dist_src: "".into() }),
        "QKERNEL" => Some(Instruction::QKernel {
            dst: parts[1].into(),
            src: parts[2].into(),
            kernel: parts[3].into(),
            ctx: None,
        }),
        "QMEAS" => Some(Instruction::QMeas { dst: parts[1].into(), src: parts[2].into() }),
        "CLADD" => Some(Instruction::ClAdd { dst: parts[1].into(), lhs: parts[2].into(), rhs: parts[3].into() }),
        "CLSUB" => Some(Instruction::ClSub { dst: parts[1].into(), lhs: parts[2].into(), rhs: parts[3].into() }),
        "CLLOAD" => Some(Instruction::ClLoad { dst: parts[1].into(), src: parts[2].into() }),
        "CLSTORE" => Some(Instruction::ClStore { addr: parts[1].into(), src: parts[2].into() }),
        "CLJUMP" => Some(Instruction::ClJump { label: parts[1].into() }),
        "CLIF" => Some(Instruction::ClIf { pred: parts[1].into(), label: parts[2].into() }),
        "HYBFORK" => Some(Instruction::HybFork),
        "HYBMERGE" => Some(Instruction::HybMerge),
        "HYBCONDEXEC" => Some(Instruction::HybCondExec { flag: parts[1].into(), then_label: parts[2].into() }),
        "HYBREDUCE" => Some(Instruction::HybReduce { src: parts[1].into(), dst: parts[2].into(), function: parts[3].into() }),
        _ => Some(Instruction::Label(line.to_string())),
    }
}
