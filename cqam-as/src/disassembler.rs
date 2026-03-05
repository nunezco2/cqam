//! Disassembler: converts binary instruction words back to human-readable CQAM assembly text.

use std::collections::HashMap;

use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;
use cqam_core::opcode;

// =============================================================================
// Public API
// =============================================================================

/// Disassemble a sequence of binary instruction words into assembly text.
///
/// Each instruction is decoded and formatted as one line. The output is
/// suitable for re-assembly (round-trip property).
///
/// # Arguments
///
/// * `code` - The encoded instruction words.
/// * `debug_map` - Optional debug symbol table mapping label IDs to names.
///   If `None`, labels are given synthetic names `_L{id}` and jump targets
///   are formatted as `@{addr}`.
///
/// # Output format
///
/// ```text
/// LABEL: start
/// ILDI R0, 42
/// JMP start
/// HALT
/// ```
///
/// When debug symbols are unavailable:
///
/// ```text
/// LABEL: _L0
/// ILDI R0, 42
/// JMP @0
/// HALT
/// ```
///
/// # Errors
///
/// Returns `CqamError::InvalidOpcode` for unrecognized opcode bytes.
pub fn disassemble(
    code: &[u32],
    debug_map: Option<&HashMap<u16, String>>,
) -> Result<String, CqamError> {
    let empty = HashMap::new();
    let dmap = debug_map.unwrap_or(&empty);

    let mut lines = Vec::with_capacity(code.len());
    for &word in code {
        lines.push(disassemble_one(word, Some(dmap))?);
    }
    Ok(lines.join("\n"))
}

/// Disassemble a single instruction word into its text representation.
///
/// # Arguments
///
/// * `word` - The 32-bit encoded instruction.
/// * `debug_map` - Optional debug symbol table for label name resolution.
///
/// # Errors
///
/// Returns `CqamError::InvalidOpcode` for unrecognized opcode bytes.
pub fn disassemble_one(
    word: u32,
    debug_map: Option<&HashMap<u16, String>>,
) -> Result<String, CqamError> {
    let empty = HashMap::new();
    let dmap = debug_map.unwrap_or(&empty);

    let instr = opcode::decode_with_debug(word, dmap)?;
    Ok(format_instruction(&instr))
}

// =============================================================================
// Instruction formatting
// =============================================================================

/// Format a decoded `Instruction` as a single line of assembly text.
///
/// This is the inverse of parsing: every instruction variant has a
/// deterministic text representation that the parser can round-trip.
fn format_instruction(instr: &Instruction) -> String {
    match instr {
        Instruction::Nop => "NOP".to_string(),
        Instruction::Label(name) => format!("LABEL: {}", name),

        // Integer arithmetic
        Instruction::IAdd { dst, lhs, rhs } => format!("IADD R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::ISub { dst, lhs, rhs } => format!("ISUB R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IMul { dst, lhs, rhs } => format!("IMUL R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IDiv { dst, lhs, rhs } => format!("IDIV R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IMod { dst, lhs, rhs } => format!("IMOD R{}, R{}, R{}", dst, lhs, rhs),

        // Integer bitwise
        Instruction::IAnd { dst, lhs, rhs } => format!("IAND R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IOr { dst, lhs, rhs } => format!("IOR R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IXor { dst, lhs, rhs } => format!("IXOR R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::INot { dst, src } => format!("INOT R{}, R{}", dst, src),
        Instruction::IShl { dst, src, amt } => format!("ISHL R{}, R{}, {}", dst, src, amt),
        Instruction::IShr { dst, src, amt } => format!("ISHR R{}, R{}, {}", dst, src, amt),

        // Integer memory
        Instruction::ILdi { dst, imm } => format!("ILDI R{}, {}", dst, imm),
        Instruction::ILdm { dst, addr } => format!("ILDM R{}, {}", dst, addr),
        Instruction::IStr { src, addr } => format!("ISTR R{}, {}", src, addr),

        // Integer comparison
        Instruction::IEq { dst, lhs, rhs } => format!("IEQ R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::ILt { dst, lhs, rhs } => format!("ILT R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IGt { dst, lhs, rhs } => format!("IGT R{}, R{}, R{}", dst, lhs, rhs),

        // Float arithmetic
        Instruction::FAdd { dst, lhs, rhs } => format!("FADD F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FSub { dst, lhs, rhs } => format!("FSUB F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FMul { dst, lhs, rhs } => format!("FMUL F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FDiv { dst, lhs, rhs } => format!("FDIV F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FLdi { dst, imm } => format!("FLDI F{}, {}", dst, imm),
        Instruction::FLdm { dst, addr } => format!("FLDM F{}, {}", dst, addr),
        Instruction::FStr { src, addr } => format!("FSTR F{}, {}", src, addr),
        Instruction::FEq { dst, lhs, rhs } => format!("FEQ R{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FLt { dst, lhs, rhs } => format!("FLT R{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FGt { dst, lhs, rhs } => format!("FGT R{}, F{}, F{}", dst, lhs, rhs),

        // Complex arithmetic
        Instruction::ZAdd { dst, lhs, rhs } => format!("ZADD Z{}, Z{}, Z{}", dst, lhs, rhs),
        Instruction::ZSub { dst, lhs, rhs } => format!("ZSUB Z{}, Z{}, Z{}", dst, lhs, rhs),
        Instruction::ZMul { dst, lhs, rhs } => format!("ZMUL Z{}, Z{}, Z{}", dst, lhs, rhs),
        Instruction::ZDiv { dst, lhs, rhs } => format!("ZDIV Z{}, Z{}, Z{}", dst, lhs, rhs),
        Instruction::ZLdi { dst, imm_re, imm_im } => {
            format!("ZLDI Z{}, {}, {}", dst, imm_re, imm_im)
        }
        Instruction::ZLdm { dst, addr } => format!("ZLDM Z{}, {}", dst, addr),
        Instruction::ZStr { src, addr } => format!("ZSTR Z{}, {}", src, addr),

        // Register-indirect memory
        Instruction::ILdx { dst, addr_reg } => format!("ILDX R{}, R{}", dst, addr_reg),
        Instruction::IStrx { src, addr_reg } => format!("ISTRX R{}, R{}", src, addr_reg),
        Instruction::FLdx { dst, addr_reg } => format!("FLDX F{}, R{}", dst, addr_reg),
        Instruction::FStrx { src, addr_reg } => format!("FSTRX F{}, R{}", src, addr_reg),
        Instruction::ZLdx { dst, addr_reg } => format!("ZLDX Z{}, R{}", dst, addr_reg),
        Instruction::ZStrx { src, addr_reg } => format!("ZSTRX Z{}, R{}", src, addr_reg),

        // Type conversion
        Instruction::CvtIF { dst_f, src_i } => format!("CVTIF F{}, R{}", dst_f, src_i),
        Instruction::CvtFI { dst_i, src_f } => format!("CVTFI R{}, F{}", dst_i, src_f),
        Instruction::CvtFZ { dst_z, src_f } => format!("CVTFZ Z{}, F{}", dst_z, src_f),
        Instruction::CvtZF { dst_f, src_z } => format!("CVTZF F{}, Z{}", dst_f, src_z),

        // Control flow
        Instruction::Jmp { target } => format!("JMP {}", target),
        Instruction::Jif { pred, target } => format!("JIF R{}, {}", pred, target),
        Instruction::Call { target } => format!("CALL {}", target),
        Instruction::Ret => "RET".to_string(),
        Instruction::Halt => "HALT".to_string(),

        // Quantum
        Instruction::QPrep { dst, dist } => format!("QPREP Q{}, {}", dst, dist),
        Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
            format!("QKERNEL Q{}, Q{}, {}, R{}, R{}", dst, src, kernel, ctx0, ctx1)
        }
        Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 } => {
            if *mode == 0 && *ctx0 == 0 && *ctx1 == 0 {
                format!("QOBSERVE H{}, Q{}", dst_h, src_q)
            } else if *mode == 1 {
                format!("QOBSERVE H{}, Q{}, {}, R{}", dst_h, src_q, mode, ctx0)
            } else {
                format!("QOBSERVE H{}, Q{}, {}, R{}, R{}", dst_h, src_q, mode, ctx0, ctx1)
            }
        }
        Instruction::QSample { dst_h, src_q, mode, ctx0, ctx1 } => {
            if *mode == 0 && *ctx0 == 0 && *ctx1 == 0 {
                format!("QSAMPLE H{}, Q{}", dst_h, src_q)
            } else if *mode == 1 {
                format!("QSAMPLE H{}, Q{}, {}, R{}", dst_h, src_q, mode, ctx0)
            } else {
                format!("QSAMPLE H{}, Q{}, {}, R{}, R{}", dst_h, src_q, mode, ctx0, ctx1)
            }
        }
        Instruction::QLoad { dst_q, addr } => format!("QLOAD Q{}, {}", dst_q, addr),
        Instruction::QStore { src_q, addr } => format!("QSTORE Q{}, {}", src_q, addr),

        // Hybrid
        Instruction::HFork => "HFORK".to_string(),
        Instruction::HMerge => "HMERGE".to_string(),
        Instruction::HCExec { flag, target } => {
            format!("HCEXEC {}, {}", flag, target)
        }
        Instruction::HReduce { src, dst, func } => {
            format!("HREDUCE H{}, R{}, {}", src, dst, func)
        }

        // Interrupt handling
        Instruction::Reti => "RETI".to_string(),
        Instruction::SetIV { trap_id, target } => {
            format!("SETIV {}, {}", trap_id, target)
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_nop() {
        assert_eq!(format_instruction(&Instruction::Nop), "NOP");
    }

    #[test]
    fn test_format_iadd() {
        let instr = Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 };
        assert_eq!(format_instruction(&instr), "IADD R0, R1, R2");
    }

    #[test]
    fn test_format_label() {
        let instr = Instruction::Label("my_loop".to_string());
        assert_eq!(format_instruction(&instr), "LABEL: my_loop");
    }

    #[test]
    fn test_format_qkernel() {
        let instr = Instruction::QKernel {
            dst: 1, src: 0, kernel: 2, ctx0: 3, ctx1: 4,
        };
        assert_eq!(format_instruction(&instr), "QKERNEL Q1, Q0, 2, R3, R4");
    }

    #[test]
    fn test_format_zldi() {
        let instr = Instruction::ZLdi { dst: 3, imm_re: 5, imm_im: -2 };
        assert_eq!(format_instruction(&instr), "ZLDI Z3, 5, -2");
    }

    #[test]
    fn test_disassemble_empty() {
        let text = disassemble(&[], None).unwrap();
        assert_eq!(text, "");
    }
}
