//! Instruction decoding: 32-bit word -> `Instruction`.
//!
//! Contains the main `decode()` and `decode_with_debug()` functions,
//! the `mnemonic()` lookup, and all bit-field extraction helpers.

use std::collections::HashMap;

use crate::error::CqamError;
use crate::instruction::*;

use super::constants::op;

/// Decode a 32-bit instruction word into an `Instruction`.
///
/// Jump and call targets are represented as address strings of the form
/// `@NNNN` where NNNN is the decimal word address. Label pseudo-instructions
/// are decoded with synthetic names `_L{id}`.
///
/// # Errors
///
/// Returns `CqamError::InvalidOpcode` if the opcode byte is not recognized.
/// Returns `CqamError::DecodeError` for structural issues (e.g., reserved
/// bits are non-zero in strict mode).
pub fn decode(word: u32) -> Result<Instruction, CqamError> {
    decode_with_debug(word, &HashMap::new())
}

/// Decode a 32-bit instruction word with debug symbol resolution.
///
/// If the word is a LABEL instruction and the label's numeric ID appears
/// in `debug_map`, the original label name is restored. Otherwise, a
/// synthetic name `_L{id}` is generated.
///
/// For jump/call instructions, addresses are formatted as `@{addr}` strings.
/// A future enhancement could resolve addresses back to label names using
/// a reverse lookup.
///
/// # Errors
///
/// Same as `decode()`.
pub fn decode_with_debug(
    word: u32,
    debug_map: &HashMap<u16, String>,
) -> Result<Instruction, CqamError> {
    let opcode = extract_opcode(word);

    match opcode {
        // -- N-format (no operands) -------------------------------------------
        op::NOP => Ok(Instruction::Nop),
        op::RET => Ok(Instruction::Ret),
        op::HALT => Ok(Instruction::Halt),
        op::HFORK => Ok(Instruction::HFork),
        op::HMERGE => Ok(Instruction::HMerge),
        op::HATMS => Ok(Instruction::HAtmS),
        op::HATME => Ok(Instruction::HAtmE),
        op::RETI => Ok(Instruction::Reti),
        op::ECALL => {
            let raw = extract_reg4(word, 20);
            let proc_id = crate::instruction::ProcId::try_from(raw)?;
            Ok(Instruction::Ecall { proc_id })
        }

        // -- RRR-format (3-register) ------------------------------------------
        op::IADD => decode_rrr(word, |dst, lhs, rhs| Instruction::IAdd { dst, lhs, rhs }),
        op::ISUB => decode_rrr(word, |dst, lhs, rhs| Instruction::ISub { dst, lhs, rhs }),
        op::IMUL => decode_rrr(word, |dst, lhs, rhs| Instruction::IMul { dst, lhs, rhs }),
        op::IDIV => decode_rrr(word, |dst, lhs, rhs| Instruction::IDiv { dst, lhs, rhs }),
        op::IMOD => decode_rrr(word, |dst, lhs, rhs| Instruction::IMod { dst, lhs, rhs }),
        op::IAND => decode_rrr(word, |dst, lhs, rhs| Instruction::IAnd { dst, lhs, rhs }),
        op::IOR => decode_rrr(word, |dst, lhs, rhs| Instruction::IOr { dst, lhs, rhs }),
        op::IXOR => decode_rrr(word, |dst, lhs, rhs| Instruction::IXor { dst, lhs, rhs }),
        op::IEQ => decode_rrr(word, |dst, lhs, rhs| Instruction::IEq { dst, lhs, rhs }),
        op::ILT => decode_rrr(word, |dst, lhs, rhs| Instruction::ILt { dst, lhs, rhs }),
        op::IGT => decode_rrr(word, |dst, lhs, rhs| Instruction::IGt { dst, lhs, rhs }),
        op::FADD => decode_rrr(word, |dst, lhs, rhs| Instruction::FAdd { dst, lhs, rhs }),
        op::FSUB => decode_rrr(word, |dst, lhs, rhs| Instruction::FSub { dst, lhs, rhs }),
        op::FMUL => decode_rrr(word, |dst, lhs, rhs| Instruction::FMul { dst, lhs, rhs }),
        op::FDIV => decode_rrr(word, |dst, lhs, rhs| Instruction::FDiv { dst, lhs, rhs }),
        op::FEQ => decode_rrr(word, |dst, lhs, rhs| Instruction::FEq { dst, lhs, rhs }),
        op::FLT => decode_rrr(word, |dst, lhs, rhs| Instruction::FLt { dst, lhs, rhs }),
        op::FGT => decode_rrr(word, |dst, lhs, rhs| Instruction::FGt { dst, lhs, rhs }),
        op::ZADD => decode_rrr(word, |dst, lhs, rhs| Instruction::ZAdd { dst, lhs, rhs }),
        op::ZSUB => decode_rrr(word, |dst, lhs, rhs| Instruction::ZSub { dst, lhs, rhs }),
        op::ZMUL => decode_rrr(word, |dst, lhs, rhs| Instruction::ZMul { dst, lhs, rhs }),
        op::ZDIV => decode_rrr(word, |dst, lhs, rhs| Instruction::ZDiv { dst, lhs, rhs }),

        // -- RR-format (2-register) -------------------------------------------
        op::INOT => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::INot { dst, src })
        }
        op::IINC => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::IInc { dst, src })
        }
        op::IDEC => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::IDec { dst, src })
        }
        op::IMOV => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::IMov { dst, src })
        }
        op::FMOV => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::FMov { dst, src })
        }
        op::ZMOV => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::ZMov { dst, src })
        }
        op::CVTIF => {
            let dst_f = extract_reg4(word, 20);
            let src_i = extract_reg4(word, 16);
            Ok(Instruction::CvtIF { dst_f, src_i })
        }
        op::CVTFI => {
            let dst_i = extract_reg4(word, 20);
            let src_f = extract_reg4(word, 16);
            Ok(Instruction::CvtFI { dst_i, src_f })
        }
        op::CVTFZ => {
            let dst_z = extract_reg4(word, 20);
            let src_f = extract_reg4(word, 16);
            Ok(Instruction::CvtFZ { dst_z, src_f })
        }
        op::CVTZF => {
            let dst_f = extract_reg4(word, 20);
            let src_z = extract_reg4(word, 16);
            Ok(Instruction::CvtZF { dst_f, src_z })
        }

        // -- Configuration query (R1-format) ----------------------------------
        op::IQCFG => {
            let dst = extract_reg4(word, 20);
            Ok(Instruction::IQCfg { dst })
        }
        op::ICCFG => {
            let dst = extract_reg4(word, 20);
            Ok(Instruction::ICCfg { dst })
        }
        op::ITID => {
            let dst = extract_reg4(word, 20);
            Ok(Instruction::ITid { dst })
        }

        // -- RR-format (register-indirect memory) -----------------------------
        op::ILDX => {
            let dst = extract_reg4(word, 20);
            let addr_reg = extract_reg4(word, 16);
            Ok(Instruction::ILdx { dst, addr_reg })
        }
        op::ISTRX => {
            let src = extract_reg4(word, 20);
            let addr_reg = extract_reg4(word, 16);
            Ok(Instruction::IStrx { src, addr_reg })
        }
        op::FLDX => {
            let dst = extract_reg4(word, 20);
            let addr_reg = extract_reg4(word, 16);
            Ok(Instruction::FLdx { dst, addr_reg })
        }
        op::FSTRX => {
            let src = extract_reg4(word, 20);
            let addr_reg = extract_reg4(word, 16);
            Ok(Instruction::FStrx { src, addr_reg })
        }
        op::ZLDX => {
            let dst = extract_reg4(word, 20);
            let addr_reg = extract_reg4(word, 16);
            Ok(Instruction::ZLdx { dst, addr_reg })
        }
        op::ZSTRX => {
            let src = extract_reg4(word, 20);
            let addr_reg = extract_reg4(word, 16);
            Ok(Instruction::ZStrx { src, addr_reg })
        }

        // -- RRS-format (2-register + shift) ----------------------------------
        op::ISHL => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            let amt = extract_u6(word, 10);
            Ok(Instruction::IShl { dst, src, amt })
        }
        op::ISHR => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            let amt = extract_u6(word, 10);
            Ok(Instruction::IShr { dst, src, amt })
        }

        // -- RI-format (register + 16-bit immediate) --------------------------
        op::ILDI => {
            let dst = extract_reg4(word, 20);
            let imm = extract_i16(word);
            Ok(Instruction::ILdi { dst, imm })
        }
        op::FLDI => {
            let dst = extract_reg4(word, 20);
            let imm = extract_i16(word);
            Ok(Instruction::FLdi { dst, imm })
        }

        // -- ZI-format (complex immediate) ------------------------------------
        op::ZLDI => {
            let dst = extract_reg4(word, 20);
            let imm_re = extract_u8(word, 8) as i8;
            let imm_im = extract_u8(word, 0) as i8;
            Ok(Instruction::ZLdi { dst, imm_re, imm_im })
        }

        // -- RA-format (register + 16-bit address) ----------------------------
        op::ILDM => {
            let dst = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::ILdm { dst, addr })
        }
        op::ISTR => {
            let src = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::IStr { src, addr })
        }
        op::FLDM => {
            let dst = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::FLdm { dst, addr })
        }
        op::FSTR => {
            let src = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::FStr { src, addr })
        }
        op::ZLDM => {
            let dst = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::ZLdm { dst, addr })
        }
        op::ZSTR => {
            let src = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::ZStr { src, addr })
        }

        // -- J-format (24-bit jump address) -----------------------------------
        op::JMP => {
            let addr = extract_u24(word);
            Ok(Instruction::Jmp {
                target: format!("@{}", addr),
            })
        }
        op::CALL => {
            let addr = extract_u24(word);
            Ok(Instruction::Call {
                target: format!("@{}", addr),
            })
        }

        // -- JR-format (predicate + 16-bit address) ---------------------------
        op::JIF => {
            let pred = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::Jif {
                pred,
                target: format!("@{}", addr),
            })
        }
        op::JMPF => {
            let raw = extract_reg4(word, 20);
            let flag = crate::instruction::FlagId::try_from(raw)?;
            let addr = extract_u16(word);
            Ok(Instruction::JmpF {
                flag,
                target: format!("@{}", addr),
            })
        }
        op::JMPFN => {
            let raw = extract_reg4(word, 20);
            let flag = crate::instruction::FlagId::try_from(raw)?;
            let addr = extract_u16(word);
            Ok(Instruction::JmpFN {
                flag,
                target: format!("@{}", addr),
            })
        }
        op::JGT => {
            let addr = extract_u24(word);
            Ok(Instruction::Jgt {
                target: format!("@{}", addr),
            })
        }
        op::JLE => {
            let addr = extract_u24(word);
            Ok(Instruction::Jle {
                target: format!("@{}", addr),
            })
        }
        op::ICMP => {
            // lhs at bit 16, rhs at bit 12 (dst=0 is ignored)
            let lhs = extract_reg4(word, 16);
            let rhs = extract_reg4(word, 12);
            Ok(Instruction::ICmp { lhs, rhs })
        }
        op::ICMPI => {
            let src = extract_reg4(word, 20);
            let imm = extract_i16(word);
            Ok(Instruction::ICmpI { src, imm })
        }
        op::SETIV => {
            let raw = extract_reg4(word, 20);
            let trap_id = crate::instruction::TrapId::try_from(raw)?;
            let addr = extract_u16(word);
            Ok(Instruction::SetIV {
                trap_id,
                target: format!("@{}", addr),
            })
        }

        // -- QP-format (quantum prepare) --------------------------------------
        op::QPREP => {
            let dst = extract_reg3(word, 21);
            let raw_dist = extract_reg3(word, 18);
            let dist = crate::instruction::DistId::try_from(raw_dist)?;
            Ok(Instruction::QPrep { dst, dist })
        }

        // -- Q-format (quantum kernel) ----------------------------------------
        op::QKERNEL => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let raw_kernel = extract_u5(word, 13);
            let kernel = crate::instruction::KernelId::try_from(raw_kernel)?;
            let ctx0 = extract_reg4(word, 9);
            let ctx1 = extract_reg4(word, 5);
            Ok(Instruction::QKernel {
                dst,
                src,
                kernel,
                ctx0,
                ctx1,
            })
        }

        // -- Q-format (quantum kernel with float context) ---------------------
        op::QKERNELF => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let raw_kernel = extract_u5(word, 13);
            let kernel = crate::instruction::KernelId::try_from(raw_kernel)?;
            let fctx0 = extract_reg4(word, 9);
            let fctx1 = extract_reg4(word, 5);
            Ok(Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 })
        }

        // -- Q-format (quantum kernel with complex context) -------------------
        op::QKERNELZ => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let raw_kernel = extract_u5(word, 13);
            let kernel = crate::instruction::KernelId::try_from(raw_kernel)?;
            let zctx0 = extract_reg4(word, 9);
            let zctx1 = extract_reg4(word, 5);
            Ok(Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 })
        }

        // -- QXch-format (quantum register handle swap) ---------------------------
        op::QXCH => {
            let qa = extract_reg3(word, 21);
            let qb = extract_reg3(word, 18);
            Ok(Instruction::QXch { qa, qb })
        }

        // -- QR-format (quantum prepare from register) ----------------------------
        op::QPREPR => {
            let dst = extract_reg3(word, 21);
            let dist_reg = extract_reg4(word, 16);
            Ok(Instruction::QPrepR { dst, dist_reg })
        }

        // -- QE-format (quantum encode from registers) ----------------------------
        op::QENCODE => {
            let dst = extract_reg3(word, 21);
            let src_base = extract_reg4(word, 16);
            let count = extract_reg4(word, 12);
            let raw_file_sel = extract_u2(word, 10);
            let file_sel = crate::instruction::FileSel::try_from(raw_file_sel)?;
            Ok(Instruction::QEncode { dst, src_base, count, file_sel })
        }

        // -- QMK-format (masked gate operations) ----------------------------------
        op::QHADM => {
            let dst_q = extract_reg3(word, 21);
            let src_q = extract_reg3(word, 18);
            let mask_reg = extract_reg4(word, 10);
            Ok(Instruction::QHadM { dst: dst_q, src: src_q, mask_reg })
        }
        op::QFLIP => {
            let dst_q = extract_reg3(word, 21);
            let src_q = extract_reg3(word, 18);
            let mask_reg = extract_reg4(word, 10);
            Ok(Instruction::QFlip { dst: dst_q, src: src_q, mask_reg })
        }
        op::QPHASE => {
            let dst_q = extract_reg3(word, 21);
            let src_q = extract_reg3(word, 18);
            let mask_reg = extract_reg4(word, 10);
            Ok(Instruction::QPhase { dst: dst_q, src: src_q, mask_reg })
        }

        // -- Q2R2-format (two-qubit CNOT gate) ------------------------------------
        op::QCNOT => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let ctrl_qubit_reg = extract_reg4(word, 14);
            let tgt_qubit_reg = extract_reg4(word, 10);
            Ok(Instruction::QCnot { dst, src, ctrl_qubit_reg, tgt_qubit_reg })
        }

        // -- QROT-format (parameterized rotation) ---------------------------------
        op::QROT => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let qubit_reg = extract_reg4(word, 14);
            let raw_axis = ((word >> 12) & 0x3) as u8;
            let axis = crate::instruction::RotAxis::try_from(raw_axis)?;
            let angle_freg = extract_reg4(word, 8);
            Ok(Instruction::QRot { dst, src, qubit_reg, axis, angle_freg })
        }

        // -- QMEAS-format (partial measurement) -----------------------------------
        op::QMEAS => {
            let dst_r = extract_reg4(word, 20);
            let src_q = extract_reg3(word, 17);
            let qubit_reg = extract_reg4(word, 13);
            Ok(Instruction::QMeas { dst_r, src_q, qubit_reg })
        }

        // -- QQQ-format (tensor product) ------------------------------------------
        op::QTENSOR => {
            let dst = extract_reg3(word, 21);
            let src0 = extract_reg3(word, 18);
            let src1 = extract_reg3(word, 15);
            Ok(Instruction::QTensor { dst, src0, src1 })
        }

        // -- Q2R2-format (custom unitary, CZ, SWAP) ------------------------------
        op::QCUSTOM => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let base_addr_reg = extract_reg4(word, 14);
            let dim_reg = extract_reg4(word, 10);
            Ok(Instruction::QCustom { dst, src, base_addr_reg, dim_reg })
        }

        op::QCZ => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let ctrl_qubit_reg = extract_reg4(word, 14);
            let tgt_qubit_reg = extract_reg4(word, 10);
            Ok(Instruction::QCz { dst, src, ctrl_qubit_reg, tgt_qubit_reg })
        }

        op::QSWAP => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let qubit_a_reg = extract_reg4(word, 14);
            let qubit_b_reg = extract_reg4(word, 10);
            Ok(Instruction::QSwap { dst, src, qubit_a_reg, qubit_b_reg })
        }

        // -- QMIXED: mixed-state preparation from classical memory --
        op::QMIXED => {
            let dst = extract_reg3(word, 21);
            let base_addr_reg = extract_reg4(word, 17);
            let count_reg = extract_reg4(word, 13);
            Ok(Instruction::QMixed { dst, base_addr_reg, count_reg })
        }

        // -- QPREPN: variable qubit count state preparation --
        op::QPREPN => {
            let dst = extract_reg3(word, 21);
            let raw_dist = extract_reg3(word, 18);
            let dist = crate::instruction::DistId::try_from(raw_dist)?;
            let qubit_count_reg = extract_reg4(word, 14);
            Ok(Instruction::QPrepN { dst, dist, qubit_count_reg })
        }

        // -- Transcendental float operations (FSIN, FCOS, FATAN2, FSQRT) --
        op::FSIN => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::FSin { dst, src })
        }
        op::FCOS => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::FCos { dst, src })
        }
        op::FATAN2 => decode_rrr(word, |dst, lhs, rhs| Instruction::FAtan2 { dst, lhs, rhs }),
        op::FSQRT => {
            let dst = extract_reg4(word, 20);
            let src = extract_reg4(word, 16);
            Ok(Instruction::FSqrt { dst, src })
        }

        // -- QPTRACE: partial trace over subsystem B --
        op::QPTRACE => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let num_qubits_a_reg = extract_reg4(word, 10);
            Ok(Instruction::QPtrace { dst, src, num_qubits_a_reg })
        }

        // -- QRESET: measure and conditionally flip a qubit to |0> --
        op::QRESET => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let qubit_reg = extract_reg4(word, 10);
            Ok(Instruction::QReset { dst, src, qubit_reg })
        }

        // -- QPREPS: register-direct product state prep --
        op::QPREPS => {
            let dst = extract_reg3(word, 21);
            let z_start = extract_reg3(word, 18);
            let count = extract_reg3(word, 15);
            Ok(Instruction::QPreps { dst, z_start, count })
        }

        // -- QPREPSM: CMEM-indirect product state prep --
        op::QPREPSM => {
            let dst = extract_reg3(word, 21);
            let r_base = extract_reg4(word, 17);
            let r_count = extract_reg4(word, 13);
            Ok(Instruction::QPrepsm { dst, r_base, r_count })
        }

        // -- QO-format (quantum observe, extended) --------------------------------
        op::QOBSERVE => {
            let dst_h = extract_reg3(word, 21);
            let src_q = extract_reg3(word, 18);
            let raw_mode = extract_u2(word, 16);
            let mode = crate::instruction::ObserveMode::try_from(raw_mode)?;
            let ctx0 = extract_reg4(word, 12);
            let ctx1 = extract_reg4(word, 8);
            Ok(Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 })
        }
        // 0x40 reserved (was QSAMPLE, removed)

        // -- QS-format (quantum memory) ---------------------------------------
        op::QLOAD => {
            let dst_q = extract_reg3(word, 21);
            let addr = extract_u8(word, 8);
            Ok(Instruction::QLoad { dst_q, addr })
        }
        op::QSTORE => {
            let src_q = extract_reg3(word, 21);
            let addr = extract_u8(word, 8);
            Ok(Instruction::QStore { src_q, addr })
        }

        // -- HR-format (hybrid reduce) ----------------------------------------
        op::HREDUCE => {
            let src = extract_reg4(word, 20);
            let dst = extract_reg4(word, 16);
            let raw_func = extract_u5(word, 11);
            let func = crate::instruction::ReduceFn::try_from(raw_func)?;
            Ok(Instruction::HReduce { src, dst, func })
        }

        // -- L-format (label pseudo-instruction) ------------------------------
        op::LABEL => {
            let label_id = extract_u16_at(word, 8);
            let name = debug_map
                .get(&label_id)
                .cloned()
                .unwrap_or_else(|| format!("_L{}", label_id));
            Ok(Instruction::Label(name))
        }

        // -- Unknown opcode ---------------------------------------------------
        _ => Err(CqamError::InvalidOpcode(opcode)),
    }
}

/// Return the mnemonic string for an opcode byte, or `None` if unrecognized.
///
/// # Examples
///
/// ```
/// use cqam_core::opcode::{mnemonic, op};
/// assert_eq!(mnemonic(op::IADD), Some("IADD"));
/// assert_eq!(mnemonic(0xFF), None);
/// ```
pub fn mnemonic(opcode: u8) -> Option<&'static str> {
    match opcode {
        op::NOP => Some("NOP"),
        op::IADD => Some("IADD"),
        op::ISUB => Some("ISUB"),
        op::IMUL => Some("IMUL"),
        op::IDIV => Some("IDIV"),
        op::IMOD => Some("IMOD"),
        op::IAND => Some("IAND"),
        op::IOR => Some("IOR"),
        op::IXOR => Some("IXOR"),
        op::INOT => Some("INOT"),
        op::IINC => Some("IINC"),
        op::IDEC => Some("IDEC"),
        op::IMOV => Some("IMOV"),
        op::FMOV => Some("FMOV"),
        op::ZMOV => Some("ZMOV"),
        op::ISHL => Some("ISHL"),
        op::ISHR => Some("ISHR"),
        op::ILDI => Some("ILDI"),
        op::ILDM => Some("ILDM"),
        op::ISTR => Some("ISTR"),
        op::IEQ => Some("IEQ"),
        op::ILT => Some("ILT"),
        op::IGT => Some("IGT"),
        op::FADD => Some("FADD"),
        op::FSUB => Some("FSUB"),
        op::FMUL => Some("FMUL"),
        op::FDIV => Some("FDIV"),
        op::FLDI => Some("FLDI"),
        op::FLDM => Some("FLDM"),
        op::FSTR => Some("FSTR"),
        op::FEQ => Some("FEQ"),
        op::FLT => Some("FLT"),
        op::FGT => Some("FGT"),
        op::ZADD => Some("ZADD"),
        op::ZSUB => Some("ZSUB"),
        op::ZMUL => Some("ZMUL"),
        op::ZDIV => Some("ZDIV"),
        op::ZLDI => Some("ZLDI"),
        op::ZLDM => Some("ZLDM"),
        op::ZSTR => Some("ZSTR"),
        op::CVTIF => Some("CVTIF"),
        op::CVTFI => Some("CVTFI"),
        op::CVTFZ => Some("CVTFZ"),
        op::CVTZF => Some("CVTZF"),
        op::IQCFG => Some("IQCFG"),
        op::ICCFG => Some("ICCFG"),
        op::ITID => Some("ITID"),
        op::HATMS => Some("HATMS"),
        op::HATME => Some("HATME"),
        op::JMP => Some("JMP"),
        op::JIF => Some("JIF"),
        op::CALL => Some("CALL"),
        op::RET => Some("RET"),
        op::HALT => Some("HALT"),
        op::LABEL => Some("LABEL"),
        op::QPREP => Some("QPREP"),
        op::QKERNEL => Some("QKERNEL"),
        op::QOBSERVE => Some("QOBSERVE"),
        op::QLOAD => Some("QLOAD"),
        op::QSTORE => Some("QSTORE"),
        op::QKERNELF => Some("QKERNELF"),
        op::QKERNELZ => Some("QKERNELZ"),
        op::QXCH => Some("QXCH"),
        op::QPREPR => Some("QPREPR"),
        op::QENCODE => Some("QENCODE"),
        op::QHADM => Some("QHADM"),
        op::QFLIP => Some("QFLIP"),
        op::QPHASE => Some("QPHASE"),
        op::QCNOT => Some("QCNOT"),
        op::QROT => Some("QROT"),
        op::QMEAS => Some("QMEAS"),
        op::QTENSOR => Some("QTENSOR"),
        op::QCUSTOM => Some("QCUSTOM"),
        op::QCZ => Some("QCZ"),
        op::QSWAP => Some("QSWAP"),
        op::QMIXED => Some("QMIXED"),
        op::QPREPN => Some("QPREPN"),
        op::FSIN => Some("FSIN"),
        op::FCOS => Some("FCOS"),
        op::FATAN2 => Some("FATAN2"),
        op::FSQRT => Some("FSQRT"),
        op::QPTRACE => Some("QPTRACE"),
        op::QRESET => Some("QRESET"),
        op::QPREPS => Some("QPREPS"),
        op::QPREPSM => Some("QPREPSM"),
        op::ILDX => Some("ILDX"),
        op::ISTRX => Some("ISTRX"),
        op::FLDX => Some("FLDX"),
        op::FSTRX => Some("FSTRX"),
        op::ZLDX => Some("ZLDX"),
        op::ZSTRX => Some("ZSTRX"),
        op::HFORK => Some("HFORK"),
        op::HMERGE => Some("HMERGE"),
        op::JMPF => Some("JMPF"),
        op::JMPFN => Some("JMPFN"),
        op::JGT => Some("JGT"),
        op::JLE => Some("JLE"),
        op::ICMP => Some("ICMP"),
        op::ICMPI => Some("ICMPI"),
        op::HREDUCE => Some("HREDUCE"),
        op::RETI => Some("RETI"),
        op::SETIV => Some("SETIV"),
        op::ECALL => Some("ECALL"),
        _ => None,
    }
}

// =============================================================================
// Internal decoding helpers
// =============================================================================

/// Extract the 8-bit opcode from a 32-bit instruction word.
fn extract_opcode(word: u32) -> u8 {
    ((word >> 24) & 0xFF) as u8
}

/// Extract a 4-bit field at the given bit position (MSB-relative).
fn extract_reg4(word: u32, shift: u32) -> u8 {
    ((word >> shift) & 0x0F) as u8
}

/// Extract a 3-bit field at the given bit position.
fn extract_reg3(word: u32, shift: u32) -> u8 {
    ((word >> shift) & 0x07) as u8
}

/// Extract a 2-bit field at the given bit position.
fn extract_u2(word: u32, shift: u32) -> u8 {
    ((word >> shift) & 0x03) as u8
}

/// Extract a 5-bit field at the given bit position.
fn extract_u5(word: u32, shift: u32) -> u8 {
    ((word >> shift) & 0x1F) as u8
}

/// Extract a 6-bit field at the given bit position.
fn extract_u6(word: u32, shift: u32) -> u8 {
    ((word >> shift) & 0x3F) as u8
}

/// Extract an 8-bit field at the given bit position.
fn extract_u8(word: u32, shift: u32) -> u8 {
    ((word >> shift) & 0xFF) as u8
}

/// Extract a 16-bit unsigned field from the low 16 bits.
fn extract_u16(word: u32) -> u16 {
    (word & 0xFFFF) as u16
}

/// Extract a 16-bit signed field from the low 16 bits.
fn extract_i16(word: u32) -> i16 {
    (word & 0xFFFF) as u16 as i16
}

/// Extract a 24-bit unsigned field from the low 24 bits.
fn extract_u24(word: u32) -> u32 {
    word & 0x00FF_FFFF
}

/// Extract a 16-bit field at a given bit position.
fn extract_u16_at(word: u32, shift: u32) -> u16 {
    ((word >> shift) & 0xFFFF) as u16
}

/// Decode helper for RRR-format instructions.
fn decode_rrr<F>(word: u32, make: F) -> Result<Instruction, CqamError>
where
    F: FnOnce(u8, u8, u8) -> Instruction,
{
    let dst = extract_reg4(word, 20);
    let lhs = extract_reg4(word, 16);
    let rhs = extract_reg4(word, 12);
    Ok(make(dst, lhs, rhs))
}
