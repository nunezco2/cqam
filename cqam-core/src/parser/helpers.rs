//! Low-level parsing utilities for tokenizing and interpreting operands.
//!
//! Contains `strip_comments`, register/immediate parsers, and compound
//! operand helpers (RRR, RR, etc.) used by the instruction and section parsers.

use super::types::ParseResult;
use crate::error::CqamError;
use crate::instruction::Instruction;

// =============================================================================
// Internal helpers
// =============================================================================

/// Strip comments from a line.
///
/// Removes everything from the first `//` or `#` to end of line.
/// Lines starting with `#!` are NOT stripped (they are pragmas, handled
/// before this function is called).
pub(crate) fn strip_comments(line: &str) -> &str {
    // Guard: pragma lines are never stripped
    if line.trim_start().starts_with("#!") {
        return line;
    }

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
pub(crate) fn extract_opcode_and_remainder(line: &str) -> (&str, &str) {
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
pub(crate) fn parse_operands(remainder: &str) -> Vec<&str> {
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

/// Parse a kernel mnemonic or numeric ID.
pub(crate) fn parse_kernel_id(token: &str, instr: &str, line_num: usize) -> Result<crate::instruction::KernelId, CqamError> {
    if let Some(id) = crate::instruction::KernelId::from_mnemonic(token) {
        Ok(id)
    } else if let Some(raw) = parse_u8(token) {
        crate::instruction::KernelId::try_from(raw).map_err(|_| CqamError::ParseError {
            line: line_num,
            message: format!(
                "{}: invalid kernel ID {} (expected UNIT, ENTG, QFFT, DIFF, GROV, DROT, PHSH, QIFT, CTLU, DIAG, PERM, or numeric ID 0-10)",
                instr, raw
            ),
        })
    } else {
        Err(CqamError::ParseError {
            line: line_num,
            message: format!(
                "{}: unknown kernel '{}' (expected UNIT, ENTG, QFFT, DIFF, GROV, DROT, PHSH, QIFT, CTLU, DIAG, PERM, or numeric ID)",
                instr, token
            ),
        })
    }
}

/// Parse a bare integer token as i8.
pub fn parse_i8(token: &str) -> Option<i8> {
    let token = token.trim();
    token.parse().ok()
}

/// Parse a bare integer token as i16, supporting decimal and hex (0x prefix).
pub(crate) fn parse_i16(token: &str) -> Option<i16> {
    let token = token.trim();
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        i16::from_str_radix(hex, 16).ok()
    } else {
        token.parse().ok()
    }
}

/// Parse a bare integer token as u16, supporting decimal and hex (0x prefix).
pub(crate) fn parse_u16(token: &str) -> Option<u16> {
    let token = token.trim();
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        token.parse().ok()
    }
}

/// Helper: parse a 3-register instruction (RRR-type).
pub(crate) fn parse_rrr<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
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
pub(crate) fn parse_rr<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
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
pub(crate) fn parse_rr_u8<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
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

/// Helper: parse a masked quantum instruction (Q-reg dst, Q-reg src, R-reg mask).
pub(crate) fn parse_qqr<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
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
        message: format!("{}: invalid Q-register '{}'", name, ops[0]),
    })?;
    let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid Q-register '{}'", name, ops[1]),
    })?;
    let mask_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid R-register '{}'", name, ops[2]),
    })?;
    Ok(build(dst, src, mask_reg))
}

/// Helper: parse reg, i16 instruction (e.g. ILDI, FLDI).
pub(crate) fn parse_reg_i16<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
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

/// Parse a rotation axis token: numeric (0-2) or named (X, Y, Z).
pub(crate) fn parse_rot_axis(token: &str) -> Option<crate::instruction::RotAxis> {
    use crate::instruction::RotAxis;
    let token = token.trim();
    match token {
        "X" | "x" | "0" => Some(RotAxis::X),
        "Y" | "y" | "1" => Some(RotAxis::Y),
        "Z" | "z" | "2" => Some(RotAxis::Z),
        _ => None,
    }
}

/// Parse an observe mode token: numeric (0-3) or named (DIST, PROB, AMP, SAMPLE).
pub(crate) fn parse_observe_mode(token: &str) -> Option<crate::instruction::ObserveMode> {
    use crate::instruction::ObserveMode;
    let token = token.trim();
    match token {
        "DIST" | "dist" => Some(ObserveMode::Dist),
        "PROB" | "prob" => Some(ObserveMode::Prob),
        "AMP" | "amp" => Some(ObserveMode::Amp),
        "SAMPLE" | "sample" => Some(ObserveMode::Sample),
        _ => parse_u8(token).and_then(|v| ObserveMode::try_from(v).ok()),
    }
}

/// Helper: parse QOBSERVE with 2-5 operands.
///
/// Syntax forms:
///   QOBSERVE H0, Q0              -> mode=0, ctx0=0, ctx1=0 (backward compat)
///   QOBSERVE H0, Q0, PROB        -> mode=1, ctx0=0, ctx1=0
///   QOBSERVE H0, Q0, PROB, R3    -> mode=1, ctx0=3, ctx1=0
///   QOBSERVE H0, Q0, AMP, R3, R4 -> mode=2, ctx0=3, ctx1=4
pub(crate) fn parse_qobserve(ops: &[&str], name: &str, line_num: usize) -> ParseResult {
    if ops.len() < 2 || ops.len() > 5 {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!("{} requires 2-5 operands, got {}", name, ops.len()),
        });
    }
    let dst_h = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid destination register '{}'", name, ops[0]),
    })?;
    let src_q = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!("{}: invalid source register '{}'", name, ops[1]),
    })?;
    let mode = if ops.len() >= 3 {
        parse_observe_mode(ops[2]).ok_or_else(|| CqamError::ParseError {
            line: line_num,
            message: format!("{}: invalid mode '{}'", name, ops[2]),
        })?
    } else {
        crate::instruction::ObserveMode::Dist
    };
    let ctx0 = if ops.len() >= 4 {
        parse_reg(ops[3]).ok_or_else(|| CqamError::ParseError {
            line: line_num,
            message: format!("{}: invalid ctx0 register '{}'", name, ops[3]),
        })?
    } else {
        0
    };
    let ctx1 = if ops.len() >= 5 {
        parse_reg(ops[4]).ok_or_else(|| CqamError::ParseError {
            line: line_num,
            message: format!("{}: invalid ctx1 register '{}'", name, ops[4]),
        })?
    } else {
        0
    };
    Ok(Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 })
}

/// Helper: parse reg, u16 instruction (e.g. ILDM, ISTR).
pub(crate) fn parse_reg_u16<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
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
