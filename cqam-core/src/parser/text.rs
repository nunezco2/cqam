//! Instruction and program parsers.
//!
//! Contains `parse_instruction()`, `parse_instruction_at()` (the giant opcode
//! match), and `parse_program()` (the multi-pass orchestrator).

use super::types::{ParseResult, ProgramMetadata, ParsedProgram};
use super::sections::{parse_data_section, parse_shared_section, parse_private_section, parse_pragma, substitute_data_refs, substitute_shared_refs};
use super::helpers::*;
use crate::error::CqamError;
use crate::instruction::Instruction;

// =============================================================================
// Public API
// =============================================================================

/// Parse a single line of CQAM source into an Instruction.
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

        // -- Register-indirect memory -----------------------------------------
        "ILDX"  => parse_rr(&ops, |dst, addr_reg| Instruction::ILdx { dst, addr_reg }, "ILDX", line_num),
        "ISTRX" => parse_rr(&ops, |src, addr_reg| Instruction::IStrx { src, addr_reg }, "ISTRX", line_num),
        "FLDX"  => parse_rr(&ops, |dst, addr_reg| Instruction::FLdx { dst, addr_reg }, "FLDX", line_num),
        "FSTRX" => parse_rr(&ops, |src, addr_reg| Instruction::FStrx { src, addr_reg }, "FSTRX", line_num),
        "ZLDX"  => parse_rr(&ops, |dst, addr_reg| Instruction::ZLdx { dst, addr_reg }, "ZLDX", line_num),
        "ZSTRX" => parse_rr(&ops, |src, addr_reg| Instruction::ZStrx { src, addr_reg }, "ZSTRX", line_num),

        // -- Type conversion --------------------------------------------------
        "CVTIF" => parse_rr(&ops, |dst_f, src_i| Instruction::CvtIF { dst_f, src_i }, "CVTIF", line_num),
        "CVTFI" => parse_rr(&ops, |dst_i, src_f| Instruction::CvtFI { dst_i, src_f }, "CVTFI", line_num),
        "CVTFZ" => parse_rr(&ops, |dst_z, src_f| Instruction::CvtFZ { dst_z, src_f }, "CVTFZ", line_num),
        "CVTZF" => parse_rr(&ops, |dst_f, src_z| Instruction::CvtZF { dst_f, src_z }, "CVTZF", line_num),

        // -- Configuration query ----------------------------------------------
        "IQCFG" => {
            if ops.len() != 1 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("IQCFG requires 1 operand, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("IQCFG: invalid register '{}'", ops[0]),
            })?;
            Ok(Instruction::IQCfg { dst })
        }
        "ICCFG" => {
            if ops.len() != 1 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "ICCFG requires 1 operand".to_string(),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("ICCFG: invalid register '{}'", ops[0]),
            })?;
            Ok(Instruction::ICCfg { dst })
        }
        "ITID" => {
            if ops.len() != 1 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "ITID requires 1 operand".to_string(),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("ITID: invalid register '{}'", ops[0]),
            })?;
            Ok(Instruction::ITid { dst })
        }

        // -- Environment call -------------------------------------------------
        "ECALL" => {
            let arg = remainder.trim();
            if arg.is_empty() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "ECALL requires a procedure name or ID".to_string(),
                });
            }
            use crate::instruction::ProcId;
            let pid = if let Some(p) = ProcId::from_name(arg) {
                p
            } else {
                let raw = arg.parse::<u8>().map_err(|_| CqamError::ParseError {
                    line: line_num,
                    message: format!("ECALL: unknown procedure '{}'", arg),
                })?;
                ProcId::try_from(raw).map_err(|_| CqamError::ParseError {
                    line: line_num,
                    message: format!("ECALL: invalid proc_id {}", raw),
                })?
            };
            Ok(Instruction::Ecall { proc_id: pid })
        }

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
        "RETI" => Ok(Instruction::Reti),
        "HALT" => Ok(Instruction::Halt),
        "SETIV" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("SETIV requires 2 operands, got {}", ops.len()),
                });
            }
            let trap_id = crate::instruction::TrapId::from_token(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("SETIV: invalid trap ID '{}'", ops[0]),
            })?;
            let target = ops[1].to_string();
            if target.is_empty() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "SETIV: missing target label".to_string(),
                });
            }
            Ok(Instruction::SetIV { trap_id, target })
        }

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
            let dist = crate::instruction::DistId::from_token(ops[1]).ok_or_else(|| CqamError::ParseError {
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
            let kernel = parse_kernel_id(ops[0], "QKERNEL", line_num)?;
            let dst = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNEL: invalid dst register '{}'", ops[1]),
            })?;
            let src = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNEL: invalid src register '{}'", ops[2]),
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
        "QKERNELF" => {
            if ops.len() != 5 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QKERNELF requires 5 operands, got {}", ops.len()),
                });
            }
            let kernel = parse_kernel_id(ops[0], "QKERNELF", line_num)?;
            let dst = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELF: invalid dst register '{}'", ops[1]),
            })?;
            let src = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELF: invalid src register '{}'", ops[2]),
            })?;
            let fctx0 = parse_reg(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELF: invalid fctx0 register '{}'", ops[3]),
            })?;
            let fctx1 = parse_reg(ops[4]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELF: invalid fctx1 register '{}'", ops[4]),
            })?;
            Ok(Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 })
        }
        "QKERNELZ" => {
            if ops.len() != 5 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QKERNELZ requires 5 operands, got {}", ops.len()),
                });
            }
            let kernel = parse_kernel_id(ops[0], "QKERNELZ", line_num)?;
            let dst = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELZ: invalid dst register '{}'", ops[1]),
            })?;
            let src = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELZ: invalid src register '{}'", ops[2]),
            })?;
            let zctx0 = parse_reg(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELZ: invalid zctx0 register '{}'", ops[3]),
            })?;
            let zctx1 = parse_reg(ops[4]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELZ: invalid zctx1 register '{}'", ops[4]),
            })?;
            Ok(Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 })
        }
        "QOBSERVE" => parse_qobserve(&ops, "QOBSERVE", line_num),
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

        "QPREPR" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QPREPR requires 2 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPR: invalid destination register '{}'", ops[0]),
            })?;
            let dist_reg = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPR: invalid dist_reg register '{}'", ops[1]),
            })?;
            Ok(Instruction::QPrepR { dst, dist_reg })
        }
        "QHADM" => parse_qqr(
            &ops,
            |dst, src, mask_reg| Instruction::QHadM { dst, src, mask_reg },
            "QHADM",
            line_num,
        ),
        "QFLIP" => parse_qqr(
            &ops,
            |dst, src, mask_reg| Instruction::QFlip { dst, src, mask_reg },
            "QFLIP",
            line_num,
        ),
        "QPHASE" => parse_qqr(
            &ops,
            |dst, src, mask_reg| Instruction::QPhase { dst, src, mask_reg },
            "QPHASE",
            line_num,
        ),
        "QCNOT" => {
            if ops.len() != 4 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QCNOT requires 4 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCNOT: invalid dst register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCNOT: invalid src register '{}'", ops[1]),
            })?;
            let ctrl_qubit_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCNOT: invalid ctrl_qubit_reg '{}'", ops[2]),
            })?;
            let tgt_qubit_reg = parse_reg(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCNOT: invalid tgt_qubit_reg '{}'", ops[3]),
            })?;
            Ok(Instruction::QCnot { dst, src, ctrl_qubit_reg, tgt_qubit_reg })
        }
        "QROT" => {
            if ops.len() != 5 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QROT requires 5 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QROT: invalid dst register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QROT: invalid src register '{}'", ops[1]),
            })?;
            let qubit_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QROT: invalid qubit_reg '{}'", ops[2]),
            })?;
            let axis = parse_rot_axis(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QROT: invalid axis '{}' (expected 0/X, 1/Y, 2/Z)", ops[3]),
            })?;
            let angle_freg = parse_reg(ops[4]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QROT: invalid angle_freg '{}'", ops[4]),
            })?;
            Ok(Instruction::QRot { dst, src, qubit_reg, axis, angle_freg })
        }
        "QMEAS" => {
            if ops.len() != 3 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QMEAS requires 3 operands, got {}", ops.len()),
                });
            }
            let dst_r = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QMEAS: invalid dst_r register '{}'", ops[0]),
            })?;
            let src_q = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QMEAS: invalid src_q register '{}'", ops[1]),
            })?;
            let qubit_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QMEAS: invalid qubit_reg '{}'", ops[2]),
            })?;
            Ok(Instruction::QMeas { dst_r, src_q, qubit_reg })
        }
        "QTENSOR" => parse_qqr(
            &ops,
            |dst, src0, src1| Instruction::QTensor { dst, src0, src1 },
            "QTENSOR",
            line_num,
        ),
        "QCUSTOM" => {
            if ops.len() != 4 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QCUSTOM requires 4 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCUSTOM: invalid dst register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCUSTOM: invalid src register '{}'", ops[1]),
            })?;
            let base_addr_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCUSTOM: invalid base_addr_reg '{}'", ops[2]),
            })?;
            let dim_reg = parse_reg(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCUSTOM: invalid dim_reg '{}'", ops[3]),
            })?;
            Ok(Instruction::QCustom { dst, src, base_addr_reg, dim_reg })
        }
        "QCZ" => {
            if ops.len() != 4 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QCZ requires 4 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCZ: invalid dst register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCZ: invalid src register '{}'", ops[1]),
            })?;
            let ctrl_qubit_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCZ: invalid ctrl_qubit_reg '{}'", ops[2]),
            })?;
            let tgt_qubit_reg = parse_reg(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QCZ: invalid tgt_qubit_reg '{}'", ops[3]),
            })?;
            Ok(Instruction::QCz { dst, src, ctrl_qubit_reg, tgt_qubit_reg })
        }
        "QSWAP" => {
            if ops.len() != 4 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QSWAP requires 4 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QSWAP: invalid dst register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QSWAP: invalid src register '{}'", ops[1]),
            })?;
            let qubit_a_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QSWAP: invalid qubit_a_reg '{}'", ops[2]),
            })?;
            let qubit_b_reg = parse_reg(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QSWAP: invalid qubit_b_reg '{}'", ops[3]),
            })?;
            Ok(Instruction::QSwap { dst, src, qubit_a_reg, qubit_b_reg })
        }
        "QENCODE" => {
            if ops.len() != 4 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QENCODE requires 4 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QENCODE: invalid destination register '{}'", ops[0]),
            })?;
            let src_base = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QENCODE: invalid source base register '{}'", ops[1]),
            })?;
            let count = parse_u8(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QENCODE: invalid count '{}'", ops[2]),
            })?;
            let file_sel = {
                let raw = parse_u8(ops[3]).ok_or_else(|| CqamError::ParseError {
                    line: line_num,
                    message: format!("QENCODE: invalid file_sel '{}'", ops[3]),
                })?;
                crate::instruction::FileSel::try_from(raw).map_err(|_| CqamError::ParseError {
                    line: line_num,
                    message: format!(
                        "QENCODE: file_sel must be 0 (R), 1 (F), or 2 (Z), got {}",
                        raw
                    ),
                })?
            };
            Ok(Instruction::QEncode { dst, src_base, count, file_sel })
        }

        // -- Mixed-state, partial-trace, reset, and float math instructions -------
        "QMIXED" => {
            if ops.len() != 3 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QMIXED requires 3 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QMIXED: invalid dst register '{}'", ops[0]),
            })?;
            let base_addr_reg = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QMIXED: invalid base_addr_reg '{}'", ops[1]),
            })?;
            let count_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QMIXED: invalid count_reg '{}'", ops[2]),
            })?;
            Ok(Instruction::QMixed { dst, base_addr_reg, count_reg })
        }
        "QPREPN" => {
            if ops.len() != 3 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QPREPN requires 3 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPN: invalid dst register '{}'", ops[0]),
            })?;
            let dist = crate::instruction::DistId::from_token(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPN: invalid distribution ID '{}'", ops[1]),
            })?;
            let qubit_count_reg = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPN: invalid qubit_count_reg '{}'", ops[2]),
            })?;
            Ok(Instruction::QPrepN { dst, dist, qubit_count_reg })
        }
        "FSIN" => parse_rr(&ops, |dst, src| Instruction::FSin { dst, src }, "FSIN", line_num),
        "FCOS" => parse_rr(&ops, |dst, src| Instruction::FCos { dst, src }, "FCOS", line_num),
        "FATAN2" => parse_rrr(&ops, |dst, lhs, rhs| Instruction::FAtan2 { dst, lhs, rhs }, "FATAN2", line_num),
        "FSQRT" => parse_rr(&ops, |dst, src| Instruction::FSqrt { dst, src }, "FSQRT", line_num),
        "QPTRACE" => parse_qqr(
            &ops,
            |dst, src, num_qubits_a_reg| Instruction::QPtrace { dst, src, num_qubits_a_reg },
            "QPTRACE",
            line_num,
        ),
        "QRESET" => parse_qqr(
            &ops,
            |dst, src, qubit_reg| Instruction::QReset { dst, src, qubit_reg },
            "QRESET",
            line_num,
        ),

        "QPREPS" => {
            // QPREPS Qdst, Z_start, count
            if ops.len() != 3 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QPREPS requires 3 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPS: invalid destination register '{}'", ops[0]),
            })?;
            let z_start = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPS: invalid Z start register '{}'", ops[1]),
            })?;
            let count = parse_u8(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPS: invalid count '{}'", ops[2]),
            })?;
            // Assembler check: z_start + 2*count must not exceed Z-file size (8 regs = Z0-Z7)
            if (z_start as u16) + 2 * (count as u16) > 8 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!(
                        "QPREPS: z_start({}) + 2*count({}) = {} exceeds Z-file size (8)",
                        z_start, count,
                        z_start as u16 + 2 * count as u16
                    ),
                });
            }
            Ok(Instruction::QPreps { dst, z_start, count })
        }

        "QPREPSM" => {
            // QPREPSM Qdst, Rbase, Rcount
            if ops.len() != 3 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QPREPSM requires 3 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPSM: invalid destination register '{}'", ops[0]),
            })?;
            let r_base = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPSM: invalid base register '{}'", ops[1]),
            })?;
            let r_count = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QPREPSM: invalid count register '{}'", ops[2]),
            })?;
            Ok(Instruction::QPrepsm { dst, r_base, r_count })
        }

        // -- Hybrid -----------------------------------------------------------
        "HFORK" => Ok(Instruction::HFork),
        "HMERGE" => Ok(Instruction::HMerge),
        "HATMS" => Ok(Instruction::HAtmS),
        "HATME" => Ok(Instruction::HAtmE),
        "JMPF" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("JMPF requires 2 operands, got {}", ops.len()),
                });
            }
            let flag = if let Some(id) = crate::instruction::FlagId::from_mnemonic(ops[0]) {
                id
            } else {
                let raw = parse_u8(ops[0]).ok_or_else(|| CqamError::ParseError {
                    line: line_num,
                    message: format!("JMPF: unknown flag '{}' (expected ZF, NF, OF, PF, QF, SF, EF, HF, DF, CF, FK, MG, IF, AF, or numeric ID)", ops[0]),
                })?;
                crate::instruction::FlagId::try_from(raw).map_err(|_| CqamError::ParseError {
                    line: line_num,
                    message: format!("JMPF: invalid flag ID {}", raw),
                })?
            };
            let target = ops[1].to_string();
            if target.is_empty() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "JMPF: missing target label".to_string(),
                });
            }
            Ok(Instruction::JmpF { flag, target })
        }
        "HREDUCE" => {
            if ops.len() != 3 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("HREDUCE requires 3 operands, got {}", ops.len()),
                });
            }
            let func = if let Some(id) = crate::instruction::ReduceFn::from_mnemonic(ops[0]) {
                id
            } else {
                let raw = parse_u8(ops[0]).ok_or_else(|| CqamError::ParseError {
                    line: line_num,
                    message: format!(
                        "HREDUCE: unknown function '{}' (expected ROUND, FLOOR, CEILI, TRUNC, ABSOL, NEGAT, MAGNI, PHASE, REALP, IMAGP, MEANT, MODEV, ARGMX, VARNC, CONJZ, NEGTZ, EXPCT, or numeric ID)",
                        ops[0]
                    ),
                })?;
                crate::instruction::ReduceFn::try_from(raw).map_err(|_| CqamError::ParseError {
                    line: line_num,
                    message: format!("HREDUCE: invalid function ID {}", raw),
                })?
            };
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("HREDUCE: invalid src register '{}'", ops[1]),
            })?;
            let dst = parse_reg(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("HREDUCE: invalid dst register '{}'", ops[2]),
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
///
/// # Errors
///
/// Returns `Err(CqamError::ParseError { line, message })` on the first
/// malformed instruction. All subsequent lines are not parsed after the first
/// error.
///
/// # Examples
///
/// ```
/// use cqam_core::parser::parse_program;
/// use cqam_core::instruction::Instruction;
///
/// // Comments (# or //) and blank lines are ignored.
/// let source = "ILDI R0, 3\nILDI R1, 4\nIADD R2, R0, R1\nHALT\n";
///
/// let parsed = parse_program(source).unwrap();
/// assert_eq!(parsed.instructions.len(), 4);
/// assert!(matches!(parsed.instructions[2], Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 }));
/// ```
pub fn parse_program(source: &str) -> Result<ParsedProgram, CqamError> {
    let mut metadata = ProgramMetadata::default();

    // --- Pass 1: Split lines into sections, extract pragmas ----------------
    #[derive(PartialEq)]
    enum Section { Code, Data, Shared, Private }
    let mut current_section = Section::Code;
    let mut data_lines: Vec<(usize, &str)> = Vec::new();   // (1-based line num, line)
    let mut shared_lines: Vec<(usize, &str)> = Vec::new();
    let mut private_lines: Vec<(usize, &str)> = Vec::new();
    let mut code_lines: Vec<(usize, &str)> = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        // Pragma directives belong to no section
        if let Some(stripped) = trimmed.strip_prefix("#!") {
            parse_pragma(stripped.trim(), line_num, &mut metadata)?;
            continue;
        }

        // Section directives
        if trimmed.eq_ignore_ascii_case(".data") {
            current_section = Section::Data;
            continue;
        }
        if trimmed.eq_ignore_ascii_case(".shared") {
            current_section = Section::Shared;
            continue;
        }
        if trimmed.eq_ignore_ascii_case(".private") {
            current_section = Section::Private;
            continue;
        }
        if trimmed.eq_ignore_ascii_case(".code") {
            current_section = Section::Code;
            continue;
        }

        match current_section {
            Section::Data => data_lines.push((line_num, line)),
            Section::Shared => shared_lines.push((line_num, line)),
            Section::Private => private_lines.push((line_num, line)),
            Section::Code => code_lines.push((line_num, line)),
        }
    }

    // --- Pass 2: Parse data directives, build label table ------------------
    let data_section = parse_data_section(&data_lines)?;

    // --- Pass 2b: Parse .shared section (reuses data directive parser) -----
    let shared_section = parse_shared_section(&shared_lines)?;

    // --- Pass 2c: Parse .private section ----------------------------------
    let private_section = parse_private_section(&private_lines)?;

    // --- Pass 3: Substitute @label tokens, parse instructions --------------
    let mut instructions = Vec::new();
    for (line_num, line) in code_lines {
        let mut substituted = substitute_data_refs(line, &data_section);
        substituted = substitute_shared_refs(&substituted, &shared_section);
        let instr = parse_instruction_at(&substituted, line_num)?;
        if !matches!(instr, Instruction::Nop) {
            instructions.push(instr);
        }
    }

    Ok(ParsedProgram { instructions, metadata, data_section, shared_section, private_section })
}
