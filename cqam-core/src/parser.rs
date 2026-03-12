//! Text-format parser for the CQAM ISA.
//!
//! Parses flat-prefix assembly syntax with numeric operands into `Instruction`
//! values. All parse functions return `Result<Instruction, CqamError>` and
//! report errors with 1-based line numbers.

use std::collections::HashMap;

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

/// Pre-loaded data from a `.data` section.
///
/// Each cell maps to one CMEM slot (one i64 per cell). Labels record the
/// starting address and length so that code can reference them with `@label`
/// and `@label.len`.
#[derive(Debug, Clone, Default)]
pub struct DataSection {
    /// Flat vector of i64 values to be loaded into CMEM[0..cells.len()].
    pub cells: Vec<i64>,

    /// label → (base_address, length_in_cells).
    pub labels: HashMap<String, (u16, u16)>,
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

    /// Pre-loaded data from the `.data` section (empty if none).
    pub data_section: DataSection,
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
        "JMPF" => {
            if ops.len() != 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("JMPF requires 2 operands, got {}", ops.len()),
                });
            }
            let flag = if let Some(id) = crate::instruction::flag_name_to_id(ops[0]) {
                id
            } else {
                parse_u8(ops[0]).ok_or_else(|| CqamError::ParseError {
                    line: line_num,
                    message: format!("JMPF: unknown flag '{}' (expected ZF, NF, OF, PF, QF, SF, EF, HF, DF, CF, FK, MG, IF, or numeric ID)", ops[0]),
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
    let mut metadata = ProgramMetadata::default();

    // --- Pass 1: Split lines into sections, extract pragmas ----------------
    #[derive(PartialEq)]
    enum Section { Code, Data }
    let mut current_section = Section::Code;
    let mut data_lines: Vec<(usize, &str)> = Vec::new();   // (1-based line num, line)
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
        if trimmed.eq_ignore_ascii_case(".code") {
            current_section = Section::Code;
            continue;
        }

        match current_section {
            Section::Data => data_lines.push((line_num, line)),
            Section::Code => code_lines.push((line_num, line)),
        }
    }

    // --- Pass 2: Parse data directives, build label table ------------------
    let data_section = parse_data_section(&data_lines)?;

    // --- Pass 3: Substitute @label tokens, parse instructions --------------
    let mut instructions = Vec::new();
    for (line_num, line) in code_lines {
        let substituted = substitute_data_refs(line, &data_section);
        let instr = parse_instruction_at(&substituted, line_num)?;
        if !matches!(instr, Instruction::Nop) {
            instructions.push(instr);
        }
    }

    Ok(ParsedProgram { instructions, metadata, data_section })
}

/// Parse `.data` section lines into a [`DataSection`].
///
/// Supported directives:
///   - `label:` — names the next allocation (must precede a data directive)
///   - `.ascii "string"` — stores one ASCII byte per cell, NUL-terminated
///   - `.asciiz "string"` — alias for `.ascii`
///   - `.i64 val1, val2, ...` — stores literal i64 values
///   - `.f64 val1, val2, ...` — stores f64 values bit-cast to i64
///   - `.c64 z1, z2, ...` — stores complex values (aJb format), 2 cells each;
///     a trailing comma continues on the next line
fn parse_data_section(lines: &[(usize, &str)]) -> Result<DataSection, CqamError> {
    let mut ds = DataSection::default();
    let mut pending_label: Option<(String, usize)> = None; // (name, line_num)

    let mut i = 0;
    while i < lines.len() {
        let (line_num, raw_line) = lines[i];
        i += 1;
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        // Label definition: `name:`
        if !line.starts_with('.') && line.ends_with(':') {
            let name = line[..line.len() - 1].trim();
            if name.is_empty() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: ".data label requires a name".to_string(),
                });
            }
            if ds.labels.contains_key(name) {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("duplicate .data label '{}'", name),
                });
            }
            pending_label = Some((name.to_string(), line_num));
            continue;
        }

        // Data directives
        let base_addr = ds.cells.len() as u16;
        let cells_before = ds.cells.len();
        let mut logical_count: Option<u16> = None;

        if let Some(rest) = line.strip_prefix(".org") {
            // .org N — advance allocation pointer to address N
            let rest = rest.trim();
            let addr: usize = rest.parse().map_err(|_| CqamError::ParseError {
                line: line_num,
                message: format!(".org: invalid address '{}'", rest),
            })?;
            if addr < ds.cells.len() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!(".org {} is below current position {}", addr, ds.cells.len()),
                });
            }
            ds.cells.resize(addr, 0);
            continue;
        } else if let Some(rest) = line.strip_prefix(".ascii") {
            parse_ascii_directive(rest.trim_start_matches('z'), line_num, &mut ds.cells)?;
        } else if let Some(rest) = line.strip_prefix(".i64") {
            parse_i64_directive(rest, line_num, &mut ds.cells)?;
        } else if let Some(rest) = line.strip_prefix(".f64") {
            parse_f64_directive(rest, line_num, &mut ds.cells)?;
        } else if let Some(rest) = line.strip_prefix(".c64") {
            // .c64 supports continuation: if a line ends with ',', the next
            // non-empty, non-comment line is a continuation of the same directive.
            let mut combined = rest.to_string();
            while combined.trim_end().ends_with(',') && i < lines.len() {
                let (_, next_raw) = lines[i];
                let next = strip_comments(next_raw).trim();
                if next.is_empty() {
                    i += 1;
                    continue;
                }
                // Stop if next line is a label or another directive
                if next.starts_with('.') || next.ends_with(':') {
                    break;
                }
                i += 1;
                combined.push_str(", ");
                combined.push_str(next);
            }
            let n = parse_c64_directive(&combined, line_num, &mut ds.cells)?;
            logical_count = Some(n as u16);
        } else {
            return Err(CqamError::ParseError {
                line: line_num,
                message: format!("unknown .data directive: {}", line),
            });
        }

        let count = logical_count.unwrap_or((ds.cells.len() - cells_before) as u16);

        // Register the pending label at this base address
        if let Some((name, _)) = pending_label.take() {
            ds.labels.insert(name, (base_addr, count));
        }
    }

    if let Some((name, ln)) = pending_label {
        return Err(CqamError::ParseError {
            line: ln,
            message: format!(".data label '{}' has no data directive", name),
        });
    }

    Ok(ds)
}

/// Parse `.ascii "..."` — one ASCII byte per cell, auto NUL-terminated.
fn parse_ascii_directive(
    rest: &str,
    line_num: usize,
    cells: &mut Vec<i64>,
) -> Result<(), CqamError> {
    let rest = rest.trim();
    if !rest.starts_with('"') || !rest.ends_with('"') || rest.len() < 2 {
        return Err(CqamError::ParseError {
            line: line_num,
            message: ".ascii requires a quoted string".to_string(),
        });
    }
    let inner = &rest[1..rest.len() - 1];
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => cells.push(10),   // newline
                Some('t') => cells.push(9),    // tab
                Some('\\') => cells.push(92),  // backslash
                Some('"') => cells.push(34),   // quote
                Some('0') => cells.push(0),    // NUL
                Some(c) => {
                    return Err(CqamError::ParseError {
                        line: line_num,
                        message: format!("unknown escape '\\{}'", c),
                    });
                }
                None => {
                    return Err(CqamError::ParseError {
                        line: line_num,
                        message: "trailing backslash in string".to_string(),
                    });
                }
            }
        } else {
            cells.push(ch as i64);
        }
    }
    // NUL terminator (so PRINT_STR can also detect end by zero)
    cells.push(0);
    Ok(())
}

/// Parse `.i64 val1, val2, ...`
fn parse_i64_directive(
    rest: &str,
    line_num: usize,
    cells: &mut Vec<i64>,
) -> Result<(), CqamError> {
    for tok in rest.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let val: i64 = tok.parse().map_err(|_| CqamError::ParseError {
            line: line_num,
            message: format!(".i64: invalid integer '{}'", tok),
        })?;
        cells.push(val);
    }
    Ok(())
}

/// Parse `.f64 val1, val2, ...` — stores f64 bit-patterns as i64.
fn parse_f64_directive(
    rest: &str,
    line_num: usize,
    cells: &mut Vec<i64>,
) -> Result<(), CqamError> {
    for tok in rest.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let val: f64 = tok.parse().map_err(|_| CqamError::ParseError {
            line: line_num,
            message: format!(".f64: invalid float '{}'", tok),
        })?;
        cells.push(val.to_bits() as i64);
    }
    Ok(())
}

/// Parse a single complex literal in `aJb` format into (re, im).
fn parse_complex_literal(tok: &str, line_num: usize) -> Result<(f64, f64), CqamError> {
    let pos = tok.find(['j', 'J']).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!(".c64: missing 'J' separator in complex literal '{}'", tok),
    })?;
    let re_str = &tok[..pos];
    let im_str = &tok[pos + 1..];
    if re_str.is_empty() {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!(".c64: missing real part in complex literal '{}'", tok),
        });
    }
    if im_str.is_empty() {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!(".c64: missing imaginary part in complex literal '{}'", tok),
        });
    }
    let re: f64 = re_str.parse().map_err(|_| CqamError::ParseError {
        line: line_num,
        message: format!(".c64: invalid real part '{}' in '{}'", re_str, tok),
    })?;
    let im: f64 = im_str.parse().map_err(|_| CqamError::ParseError {
        line: line_num,
        message: format!(".c64: invalid imaginary part '{}' in '{}'", im_str, tok),
    })?;
    Ok((re, im))
}

/// Parse `.c64 z1, z2, ...` — stores complex values (aJb format) as pairs of
/// f64 bit-patterns. Returns the number of complex values parsed (NOT the
/// number of cells, which is 2x that).
fn parse_c64_directive(
    rest: &str,
    line_num: usize,
    cells: &mut Vec<i64>,
) -> Result<usize, CqamError> {
    let mut value_count = 0;
    for tok in rest.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let (re, im) = parse_complex_literal(tok, line_num)?;
        cells.push(re.to_bits() as i64);
        cells.push(im.to_bits() as i64);
        value_count += 1;
    }
    Ok(value_count)
}

/// Replace `@label` with the CMEM base address and `@label.len` with the
/// cell count for all data labels found in the line.
fn substitute_data_refs(line: &str, ds: &DataSection) -> String {
    if ds.labels.is_empty() || !line.contains('@') {
        return line.to_string();
    }

    let mut result = line.to_string();

    // Process longest labels first to avoid partial substitution
    let mut labels: Vec<_> = ds.labels.iter().collect();
    labels.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (name, (base, len)) in &labels {
        // @label.len must be replaced before @label to avoid partial match
        let len_token = format!("@{}.len", name);
        result = result.replace(&len_token, &len.to_string());
        let addr_token = format!("@{}", name);
        result = result.replace(&addr_token, &base.to_string());
    }

    result
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

/// Parse a kernel mnemonic or numeric ID.
fn parse_kernel_id(token: &str, instr: &str, line_num: usize) -> Result<u8, CqamError> {
    if let Some(id) = crate::instruction::kernel_name_to_id(token) {
        Ok(id)
    } else if let Some(id) = parse_u8(token) {
        Ok(id)
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
