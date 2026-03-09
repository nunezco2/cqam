//! Text-format parser for the CQAM ISA.
//!
//! Parses flat-prefix assembly syntax with numeric operands into `Instruction`
//! values. All parse functions return `Result<Instruction, CqamError>` and
//! report errors with 1-based line numbers.

use crate::error::CqamError;
use crate::instruction::Instruction;

/// Convenience type alias for parser results.
pub type ParseResult = Result<Instruction, CqamError>;

/// Metadata extracted from `#!` pragma directives in a CQAM source file.
///
/// Pragmas are processed during parsing but do not generate instructions.
/// They provide configuration hints that the loader/runner can apply before
/// execution.
#[derive(Debug, Default, Clone)]
pub struct ProgramMetadata {
    /// Number of qubits requested by the program via `#! qubits N`.
    ///
    /// `None` means no pragma was found; use the default or CLI value.
    pub qubits: Option<u8>,
}

/// Result of parsing a complete CQAM program.
///
/// Contains both the instruction stream and any pragma metadata.
#[derive(Debug)]
pub struct ParsedProgram {
    /// The instruction stream (labels, ops, no Nops).
    pub instructions: Vec<Instruction>,

    /// Metadata from `#!` pragma directives.
    pub metadata: ProgramMetadata,
}

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

        // -- Environment call -------------------------------------------------
        "ECALL" => {
            let arg = remainder.trim();
            if arg.is_empty() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "ECALL requires a procedure name or ID".to_string(),
                });
            }
            use crate::instruction::proc_id;
            let pid = match arg {
                "PRINT_INT" => proc_id::PRINT_INT,
                "PRINT_FLOAT" => proc_id::PRINT_FLOAT,
                "PRINT_STR" => proc_id::PRINT_STR,
                "PRINT_CHAR" => proc_id::PRINT_CHAR,
                "DUMP_REGS" => proc_id::DUMP_REGS,
                _ => arg.parse::<u8>().map_err(|_| CqamError::ParseError {
                    line: line_num,
                    message: format!("ECALL: unknown procedure '{}'", arg),
                })?,
            };
            if pid > 15 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("ECALL: proc_id {} exceeds max 15", pid),
                });
            }
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
            let trap_id = parse_u8(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("SETIV: invalid trap ID '{}'", ops[0]),
            })?;
            if trap_id > 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("SETIV: trap ID must be 0-2, got {}", trap_id),
                });
            }
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
        "QKERNELF" => {
            if ops.len() != 5 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("QKERNELF requires 5 operands, got {}", ops.len()),
                });
            }
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELF: invalid dst register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELF: invalid src register '{}'", ops[1]),
            })?;
            let kernel = parse_u8(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELF: invalid kernel ID '{}'", ops[2]),
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
            let dst = parse_reg(ops[0]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELZ: invalid dst register '{}'", ops[0]),
            })?;
            let src = parse_reg(ops[1]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELZ: invalid src register '{}'", ops[1]),
            })?;
            let kernel = parse_u8(ops[2]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QKERNELZ: invalid kernel ID '{}'", ops[2]),
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
        "QSAMPLE" => parse_qobserve(&ops, "QSAMPLE", line_num),
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
            let file_sel = parse_u8(ops[3]).ok_or_else(|| CqamError::ParseError {
                line: line_num,
                message: format!("QENCODE: invalid file_sel '{}'", ops[3]),
            })?;
            if file_sel > 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!(
                        "QENCODE: file_sel must be 0 (R), 1 (F), or 2 (Z), got {}",
                        file_sel
                    ),
                });
            }
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
            let dist = parse_u8(ops[1]).ok_or_else(|| CqamError::ParseError {
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
    let mut instructions = Vec::new();
    let mut metadata = ProgramMetadata::default();

    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        // Check for pragma directive before comment stripping
        if let Some(stripped) = trimmed.strip_prefix("#!") {
            let pragma_content = stripped.trim();
            parse_pragma(pragma_content, idx + 1, &mut metadata)?;
            continue;
        }

        let instr = parse_instruction_at(line, idx + 1)?;
        if !matches!(instr, Instruction::Nop) {
            instructions.push(instr);
        }
    }
    Ok(ParsedProgram { instructions, metadata })
}

/// Parse a single pragma line (without the `#!` prefix).
///
/// Returns Ok(()) and updates metadata, or Err on malformed pragma.
fn parse_pragma(
    line: &str,
    line_num: usize,
    metadata: &mut ProgramMetadata,
) -> Result<(), CqamError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        // Empty pragma -- ignore
        return Ok(());
    }

    match tokens[0] {
        "qubits" => {
            if tokens.len() < 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "#! qubits requires a number".to_string(),
                });
            }
            let n: u8 = tokens[1].parse().map_err(|_| CqamError::ParseError {
                line: line_num,
                message: format!("#! qubits value must be a number, got '{}'", tokens[1]),
            })?;
            if n == 0 || n > 16 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("#! qubits must be 1..16, got {}", n),
                });
            }
            metadata.qubits = Some(n);
        }
        _ => {
            // Unknown pragma -- ignore for forward compatibility
        }
    }
    Ok(())
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Strip comments from a line.
///
/// Removes everything from the first `//` or `#` to end of line.
/// Lines starting with `#!` are NOT stripped (they are pragmas, handled
/// before this function is called).
fn strip_comments(line: &str) -> &str {
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

/// Helper: parse a masked quantum instruction (Q-reg dst, Q-reg src, R-reg mask).
fn parse_qqr<F>(ops: &[&str], build: F, name: &str, line_num: usize) -> ParseResult
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

/// Parse a rotation axis token: numeric (0-2) or named (X, Y, Z).
fn parse_rot_axis(token: &str) -> Option<u8> {
    let token = token.trim();
    match token {
        "X" | "x" | "0" => Some(0),
        "Y" | "y" | "1" => Some(1),
        "Z" | "z" | "2" => Some(2),
        _ => None,
    }
}

/// Parse an observe mode token: numeric (0-2) or named (DIST, PROB, AMP).
fn parse_observe_mode(token: &str) -> Option<u8> {
    let token = token.trim();
    match token {
        "DIST" | "dist" => Some(0),
        "PROB" | "prob" => Some(1),
        "AMP" | "amp" => Some(2),
        "SAMPLE" | "sample" => Some(3),
        _ => parse_u8(token).filter(|&v| v <= 3),
    }
}

/// Helper: parse QOBSERVE/QSAMPLE with 2-5 operands.
///
/// Syntax forms:
///   QOBSERVE H0, Q0              -> mode=0, ctx0=0, ctx1=0 (backward compat)
///   QOBSERVE H0, Q0, PROB        -> mode=1, ctx0=0, ctx1=0
///   QOBSERVE H0, Q0, PROB, R3    -> mode=1, ctx0=3, ctx1=0
///   QOBSERVE H0, Q0, AMP, R3, R4 -> mode=2, ctx0=3, ctx1=4
fn parse_qobserve(ops: &[&str], name: &str, line_num: usize) -> ParseResult {
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
        0
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
    match name {
        "QOBSERVE" => Ok(Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 }),
        "QSAMPLE" => Ok(Instruction::QSample { dst_h, src_q, mode, ctx0, ctx1 }),
        _ => unreachable!(),
    }
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
