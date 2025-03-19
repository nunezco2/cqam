use crate::instruction::Instruction;

/// Parse a line of CQAM source into an Instruction
pub fn parse_instruction(line: &str) -> Instruction {
    let line = line.trim();

    if let Some(rest) = line.strip_prefix("CL:LOAD") {
        let parts: Vec<&str> = rest.split(',').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            return Instruction::ClLoad { dst: parts[0].into(), src: parts[1].into() };
        }
    }

    if let Some(rest) = line.strip_prefix("CL:ADD") {
        let parts: Vec<&str> = rest.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            return Instruction::ClAdd { dst: parts[0].into(), lhs: parts[1].into(), rhs: parts[2].into() };
        }
    }

    if let Some(rest) = line.strip_prefix("LABEL:") {
        return Instruction::Label(rest.trim().to_string());
    }

    Instruction::NoOp
}
