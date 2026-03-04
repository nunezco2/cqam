// cqam-core/src/parser.rs
//
// Phase 4: Parser for the flat-prefix ISA syntax with numeric operands.
// Returns Result<Instruction, CqamError> instead of silently returning Nop.

use crate::error::CqamError;
use crate::instruction::Instruction;

/// Convenience type alias for parser results.
pub type ParseResult = Result<Instruction, CqamError>;

// =============================================================================
// Public API
// =============================================================================

/// Parse a single line of CQAM source into an Instruction.
///
/// Handles all instruction variants in the Phase 2 ISA.
///
/// Comments (`#` and `//`) are stripped. Blank lines return `Ok(Nop)`.
/// Unknown instructions return `Err(CqamError::ParseError { ... })`.
/// Missing or invalid operands return `Err(CqamError::ParseError { ... })`.
///
/// The `line_num` parameter is used for error reporting (1-based line number).
pub fn parse_instruction(line: &str) -> ParseResult {
    parse_instruction_at(line, 0)
}

/// Parse a single line with a line number for error reporting.
pub fn parse_instruction_at(line: &str, line_num: usize) -> ParseResult {
    let line = strip_comments(line).trim();

    if line.is_empty() {
        return Ok(Instruction::Nop);
    }

    // Special case: LABEL: prefix
    if let Some(rest) = line.strip_prefix("LABEL:") {
        let name = rest.trim();
        if name.is_empty() {
            return Err(CqamError::ParseError {
                line: line_num,
                message: "LABEL requires a name".to_string(),
            });
        }
        return Ok(Instruction::Label(name.to_string()));
    }

    let (opcode, remainder) = extract_opcode_and_remainder(line);
    let ops = parse_operands(remainder);

    match opcode {
        // -- Integer arithmetic (3-register) ----------------------------------
        "IADD" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IAdd { dst, lhs, rhs }, "IADD", line_num),
        "ISUB" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::ISub { dst, lhs, rhs }, "ISUB", line_num),
        "IMUL" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IMul { dst, lhs, rhs }, "IMUL", line_num),
        "IDIV" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IDiv { dst, lhs, rhs }, "IDIV", line_num),
        "IMOD" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IMod { dst, lhs, rhs }, "IMOD", line_num),

        // -- Integer bitwise --------------------------------------------------
        "IAND" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IAnd { dst, lhs, rhs }, "IAND", line_num),
        "IOR"  => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IOr  { dst, lhs, rhs }, "IOR", line_num),
        "IXOR" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IXor { dst, lhs, rhs }, "IXOR", line_num),
        "INOT" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("INOT requires 2 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("INOT: invalid destination register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("INOT: invalid source register '{}'", ops[1]),
            })?;
            Ok(Instruction::INot { dst, src })
        }
        "ISHL" => parse_rr_u8(&ops, |dst, src, amt| Instruction::IShl { dst, src, amt }, "ISHL", line_num),
        "ISHR" => parse_rr_u8(&ops, |dst, src, amt| Instruction::IShr { dst, src, amt }, "ISHR", line_num),

        // -- Integer memory ---------------------------------------------------
        "ILDI" => parse_reg_i16(&ops, |dst, imm| Instruction::ILdi { dst, imm }, "ILDI", line_num),
        "ILDM" => parse_reg_u16(&ops, |dst, addr| Instruction::ILdm { dst, addr }, "ILDM", line_num),
        "ISTR" => parse_reg_u16(&ops, |src, addr| Instruction::IStr { src, addr }, "ISTR", line_num),

        // -- Integer comparison -----------------------------------------------
        "IEQ" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IEq { dst, lhs, rhs }, "IEQ", line_num),
        "ILT" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::ILt { dst, lhs, rhs }, "ILT", line_num),
        "IGT" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::IGt { dst, lhs, rhs }, "IGT", line_num),

        // -- Float arithmetic ------------------------------------------------
        "FADD" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::FAdd { dst, lhs, rhs }, "FADD", line_num),
        "FSUB" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::FSub { dst, lhs, rhs }, "FSUB", line_num),
        "FMUL" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::FMul { dst, lhs, rhs }, "FMUL", line_num),
        "FDIV" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::FDiv { dst, lhs, rhs }, "FDIV", line_num),
        "FLDI" => parse_reg_i16(&ops, |dst, imm| Instruction::FLdi { dst, imm }, "FLDI", line_num),
        "FLDM" => parse_reg_u16(&ops, |dst, addr| Instruction::FLdm { dst, addr }, "FLDM", line_num),
        "FSTR" => parse_reg_u16(&ops, |src, addr| Instruction::FStr { src, addr }, "FSTR", line_num),
        "FEQ" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::FEq { dst, lhs, rhs }, "FEQ", line_num),
        "FLT" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::FLt { dst, lhs, rhs }, "FLT", line_num),
        "FGT" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::FGt { dst, lhs, rhs }, "FGT", line_num),

        // -- Complex arithmetic -----------------------------------------------
        "ZADD" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::ZAdd { dst, lhs, rhs }, "ZADD", line_num),
        "ZSUB" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::ZSub { dst, lhs, rhs }, "ZSUB", line_num),
        "ZMUL" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::ZMul { dst, lhs, rhs }, "ZMUL", line_num),
        "ZDIV" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::ZDiv { dst, lhs, rhs }, "ZDIV", line_num),
        "ZLDI" => {
            if ops.len() != 3 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("ZLDI requires 3 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("ZLDI: invalid register '{}'", ops[0]),
            })?;
            let imm_re = parse_i8(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("ZLDI: invalid real immediate '{}'", ops[1]),
            })?;
            let imm_im = parse_i8(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("ZLDI: invalid imaginary immediate '{}'", ops[2]),
            })?;
            Ok(Instruction::ZLdi { dst, imm_re, imm_im })
        }
        "ZLDM" => parse_reg_u16(&ops, |dst, addr| Instruction::ZLdm { dst, addr }, "ZLDM", line_num),
        "ZSTR" => parse_reg_u16(&ops, |src, addr| Instruction::ZStr { src, addr }, "ZSTR", line_num),

        // -- Type conversion --------------------------------------------------
        "CVTIF" => parse_rr(&ops, |dst_f, src_i| Instruction::CvtIF { dst_f, src_i }, "CVTIF", line_num),
        "CVTFI" => parse_rr(&ops, |dst_i, src_f| Instruction::CvtFI { dst_i, src_f }, "CVTFI", line_num),
        "CVTFZ" => parse_rr(&ops, |dst_z, src_f| Instruction::CvtFZ { dst_z, src_f }, "CVTFZ", line_num),
        "CVTZF" => parse_rr(&ops, |dst_f, src_z| Instruction::CvtZF { dst_f, src_z }, "CVTZF", line_num),

        // -- Control flow -----------------------------------------------------
        "JMP" => {
            let label = remainder.trim();
            if label.is_empty() {
                Err(CqamError::ParseError {
                    line: line_num,
                    message: "JMP requires a target label".to_string(),
                })
            } else {
                Ok(Instruction::Jmp { target: label.to_string() })
            }
        }
        "JIF" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("JIF requires 2 operands, got {}", ops.len()),
                });
            }
            let pred = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("JIF: invalid predicate register '{}'", ops[0]),
            })?;
            let target = ops[1].to_string();
            if target.is_empty() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "JIF: missing target label".to_string(),
                });
            }
            Ok(Instruction::Jif { pred, target })
        }
        "CALL" => {
            let label = remainder.trim();
            if label.is_empty() {
                Err(CqamError::ParseError {
                    line: line_num,
                    message: "CALL requires a target label".to_string(),
                })
            } else {
                Ok(Instruction::Call { target: label.to_string() })
            }
        }
        "RET" => Ok(Instruction::Ret),
        "HALT" => Ok(Instruction::Halt),

        // -- Quantum ----------------------------------------------------------
        "QPREP" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QPREP requires 2 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREP: invalid register '{}'", ops[0]),
            })?;
            let dist = parse_u8(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREP: invalid distribution ID '{}'", ops[1]),
            })?;
            Ok(Instruction::QPrep { dst, dist })
        }
        "QKERNEL" => {
            if ops.len() != 5 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QKERNEL requires 5 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNEL: invalid dst register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNEL: invalid src register '{}'", ops[1]),
            })?;
            let kernel = parse_u8(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNEL: invalid kernel ID '{}'", ops[2]),
            })?;
            let ctx0 = parse_reg(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNEL: invalid ctx0 register '{}'", ops[3]),
            })?;
            let ctx1 = parse_reg(ops[4]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNEL: invalid ctx1 register '{}'", ops[4]),
            })?;
            Ok(Instruction::QKernel { dst, src, kernel, ctx0, ctx1 })
        }
        "QOBSERVE" => parse_rr(&ops, |dst_h, src_q| Instruction::QObserve { dst_h, src_q }, "QOBSERVE", line_num),
        "QLOAD" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QLOAD requires 2 operands, got {}", ops.len()),
                });
            }
            let dst_q = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QLOAD: invalid register '{}'", ops[0]),
            })?;
            let addr = parse_u8(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QLOAD: invalid address '{}'", ops[1]),
            })?;
            Ok(Instruction::QLoad { dst_q, addr })
        }
        "QSTORE" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QSTORE requires 2 operands, got {}", ops.len()),
                });
            }
            let src_q = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QSTORE: invalid register '{}'", ops[0]),
            })?;
            let addr = parse_u8(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QSTORE: invalid address '{}'", ops[1]),
            })?;
            Ok(Instruction::QStore { src_q, addr })
        }

        // -- Hybrid -----------------------------------------------------------
        "HFORK" => Ok(Instruction::HFork),
        "HMERGE" => Ok(Instruction::HMerge),
        "HCEXEC" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("HCEXEC requires 2 operands, got {}", ops.len()),
                });
            }
            let flag = parse_u8(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("HCEXEC: invalid flag ID '{}'", ops[0]),
            })?;
            let target = ops[1].to_string();
            if target.is_empty() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "HCEXEC: missing target label".to_string(),
                });
            }
            Ok(Instruction::HCExec { flag, target })
        }
        "HREDUCE" => {
            if ops.len() != 3 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("HREDUCE requires 3 operands, got {}", ops.len()),
                });
            }
            let src = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("HREDUCE: invalid src register '{}'", ops[0]),
            })?;
            let dst = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("HREDUCE: invalid dst register '{}'", ops[1]),
            })?;
            let func = parse_u8(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("HREDUCE: invalid function ID '{}'", ops[2]),
            })?;
            Ok(Instruction::HReduce { src, dst, func })
        }

        // NOP explicitly
        "NOP" => Ok(Instruction::Nop),

        // Unknown instruction
        _ => Err(CqamError::ParseError {
            line: line_num,
            message: format!("Unknown instruction: '{}'", opcode),
        }),
    }
}

/// Parse a complete multi-line CQAM program into a vector of instructions.
///
/// - Iterates over each line of `source`
/// - Calls `parse_instruction_at()` on each line with 1-based line number
/// - Filters out `Nop` results (blank lines, comments)
/// - Propagates parse errors
pub fn parse_program(source: &str) -> Result<Vec<Instruction>, CqamError> {
    let mut instructions = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let instr = parse_instruction_at(line, idx + 1)?;
        if !matches!(instr, Instruction::Nop) {
            instructions.push(instr);
        }
    }
    Ok(instructions)
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Strip comments from a line.
///
/// Removes everything from the first `//` or `#` to end of line.
fn strip_comments(line: &str) -> &str {
    let double_slash_pos = line.find("//");
    let hash_pos = line.find('#');

    let comment_pos = match (double_slash_pos, hash_pos) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    match comment_pos {
        Some(pos) => line[..pos].trim_end(),
        None => line,
    }
}

/// Extract the opcode token and operand remainder from a line.
fn extract_opcode_and_remainder(line: &str) -> (&str, &str) {
    let boundary = line
        .find(|c: char| c.is_whitespace() || c == ',')
        .unwrap_or(line.len());

    let opcode = &line[..boundary];
    let remainder = if boundary < line.len() {
        line[boundary..].trim_start()
    } else {
        ""
    };

    (opcode, remainder)
}

/// Parse a comma-separated operand string into trimmed tokens.
fn parse_operands(remainder: &str) -> Vec<&str> {
    remainder
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse a register name with any recognized prefix (R, F, Z, Q, H)
/// and return the numeric index.
///
/// Examples:
/// - `parse_reg("R3")` -> `Some(3)`
/// - `parse_reg("F15")` -> `Some(15)`
/// - `parse_reg("Q7")` -> `Some(7)`
/// - `parse_reg("42")` -> `None` (no prefix match)
pub fn parse_reg(token: &str) -> Option<u8> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    let first = token.as_bytes()[0];
    match first {
        b'R' | b'F' | b'Z' | b'Q' | b'H' => {
            let num_part = &token[1..];
            let idx: u8 = num_part.parse().ok()?;
            // Validate range: R/F/Z allow 0-15, Q/H allow 0-7
            match first {
                b'R' | b'F' | b'Z' => {
                    if idx < 16 { Some(idx) } else { None }
                }
                b'Q' | b'H' => {
                    if idx < 8 { Some(idx) } else { None }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Parse a bare integer token as u8.
pub fn parse_u8(token: &str) -> Option<u8> {
    let token = token.trim();
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).ok()
    } else {
        token.parse().ok()
    }
}

/// Parse a bare integer token as i8.
pub fn parse_i8(token: &str) -> Option<i8> {
    let token = token.trim();
    token.parse().ok()
}

/// Parse a bare integer token as i16, supporting decimal and hex (0x prefix).
pub fn parse_i16(token: &str) -> Option<i16> {
    let token = token.trim();
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        i16::from_str_radix(hex, 16).ok()
    } else {
        token.parse().ok()
    }
}

/// Parse a bare integer token as u16, supporting decimal and hex (0x prefix).
pub fn parse_u16(token: &str) -> Option<u16> {
    let token = token.trim();
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        token.parse().ok()
    }
}

/// Helper: parse a 3-register instruction (RRR-type).
fn parse_rrr<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
where
    F: FnOnce(u8, u8, u8) -> Instruction,
{
    if ops.len() != 3 {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!("{} requires 3 operands, got {}", name, ops.len()),
        });
    }
    let a = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[0]),
    })?;
    let b = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[1]),
    })?;
    let c = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[2]),
    })?;
    Ok(build(a, b, c))
}

/// Helper: parse a 2-register instruction (RR-type).
fn parse_rr<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
where
    F: FnOnce(u8, u8) -> Instruction,
{
    if ops.len() != 2 {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!("{} requires 2 operands, got {}", name, ops.len()),
        });
    }
    let a = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[0]),
    })?;
    let b = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[1]),
    })?;
    Ok(build(a, b))
}

/// Helper: parse reg, reg, u8 instruction (e.g. ISHL, ISHR).
fn parse_rr_u8<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
where
    F: FnOnce(u8, u8, u8) -> Instruction,
{
    if ops.len() != 3 {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!("{} requires 3 operands, got {}", name, ops.len()),
        });
    }
    let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[0]),
    })?;
    let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[1]),
    })?;
    let val = parse_u8(ops[2]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid value '{}'", name, ops[2]),
    })?;
    Ok(build(dst, src, val))
}

/// Helper: parse reg, i16 instruction (e.g. ILDI, FLDI).
fn parse_reg_i16<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
where
    F: FnOnce(u8, i16) -> Instruction,
{
    if ops.len() != 2 {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!("{} requires 2 operands, got {}", name, ops.len()),
        });
    }
    let reg = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[0]),
    })?;
    let imm = parse_i16(ops[1]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid immediate '{}'", name, ops[1]),
    })?;
    Ok(build(reg, imm))
}

/// Helper: parse reg, u16 instruction (e.g. ILDM, ISTR).
fn parse_reg_u16<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
where
    F: FnOnce(u8, u16) -> Instruction,
{
    if ops.len() != 2 {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!("{} requires 2 operands, got {}", name, ops.len()),
        });
    }
    let reg = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid register '{}'", name, ops[0]),
    })?;
    let addr = parse_u16(ops[1]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid address '{}'", name, ops[1]),
    })?;
    Ok(build(reg, addr))
}
