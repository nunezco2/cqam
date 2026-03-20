//! Instruction encoding: `Instruction` -> 32-bit word.
//!
//! Contains the main `encode()` function, the `encode_label()` convenience
//! wrapper, all format-specific encoding helpers, and validation utilities.

use std::collections::HashMap;

use crate::error::CqamError;
use crate::instruction::*;

use super::constants::op;
use super::constants::{
    MAX_ADDR16, MAX_ADDR24, MAX_DIST, MAX_FILE_SEL, MAX_FUNC, MAX_KERNEL, MAX_MODE, MAX_REG3,
    MAX_REG4, MAX_SHIFT,
};

/// Encode an [`Instruction`] into a 32-bit instruction word.
///
/// # Arguments
///
/// * `instr` - The instruction to encode.
/// * `label_map` - Maps label names to resolved word addresses. Required for
///   encoding JMP, JIF, CALL, JMPF, and LABEL instructions.
///
/// # Errors
///
/// Returns [`CqamError`](crate::error::CqamError) if:
/// - A jump/call target label is not found in `label_map`
///   ([`CqamError::UnresolvedLabel`](crate::error::CqamError::UnresolvedLabel)).
/// - A conditional branch target address exceeds 16 bits
///   ([`CqamError::AddressOverflow`](crate::error::CqamError::AddressOverflow)).
/// - A register index exceeds its file's range
///   ([`CqamError::OperandOverflow`](crate::error::CqamError::OperandOverflow)).
/// - A shift amount, kernel ID, or function ID exceeds its field width.
///
/// # Instruction word formats
///
/// See `reference/opcodes.md` for the bit-level layout of each format.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use cqam_core::instruction::Instruction;
/// use cqam_core::opcode::encode;
///
/// // N-format: HALT encodes as [0x2B][0x00][0x00][0x00]
/// let word = encode(&Instruction::Halt, &HashMap::new()).unwrap();
/// assert_eq!(word, 0x2B000000);
///
/// // RRR-format: IADD R2, R3, R4 -> [0x01][2][3][4][padding]
/// let word = encode(
///     &Instruction::IAdd { dst: 2, lhs: 3, rhs: 4 },
///     &HashMap::new(),
/// ).unwrap();
/// assert_eq!(word, 0x01234000);
/// ```
pub fn encode(instr: &Instruction, label_map: &HashMap<String, u32>) -> Result<u32, CqamError> {
    match instr {
        // -- N-format (no operands) -------------------------------------------
        Instruction::Nop => Ok(encode_n(op::NOP)),
        Instruction::Ret => Ok(encode_n(op::RET)),
        Instruction::Halt => Ok(encode_n(op::HALT)),
        Instruction::HFork => Ok(encode_n(op::HFORK)),
        Instruction::HMerge => Ok(encode_n(op::HMERGE)),
        Instruction::HAtmS => Ok(encode_n(op::HATMS)),
        Instruction::HAtmE => Ok(encode_n(op::HATME)),
        Instruction::Reti => Ok(encode_n(op::RETI)),
        Instruction::Ecall { proc_id } => encode_rr(op::ECALL, u8::from(*proc_id), 0),

        // -- RRR-format (3-register) ------------------------------------------
        Instruction::IAdd { dst, lhs, rhs } => encode_rrr(op::IADD, *dst, *lhs, *rhs),
        Instruction::ISub { dst, lhs, rhs } => encode_rrr(op::ISUB, *dst, *lhs, *rhs),
        Instruction::IMul { dst, lhs, rhs } => encode_rrr(op::IMUL, *dst, *lhs, *rhs),
        Instruction::IDiv { dst, lhs, rhs } => encode_rrr(op::IDIV, *dst, *lhs, *rhs),
        Instruction::IMod { dst, lhs, rhs } => encode_rrr(op::IMOD, *dst, *lhs, *rhs),
        Instruction::IAnd { dst, lhs, rhs } => encode_rrr(op::IAND, *dst, *lhs, *rhs),
        Instruction::IOr { dst, lhs, rhs } => encode_rrr(op::IOR, *dst, *lhs, *rhs),
        Instruction::IXor { dst, lhs, rhs } => encode_rrr(op::IXOR, *dst, *lhs, *rhs),
        Instruction::IEq { dst, lhs, rhs } => encode_rrr(op::IEQ, *dst, *lhs, *rhs),
        Instruction::ILt { dst, lhs, rhs } => encode_rrr(op::ILT, *dst, *lhs, *rhs),
        Instruction::IGt { dst, lhs, rhs } => encode_rrr(op::IGT, *dst, *lhs, *rhs),
        Instruction::FAdd { dst, lhs, rhs } => encode_rrr(op::FADD, *dst, *lhs, *rhs),
        Instruction::FSub { dst, lhs, rhs } => encode_rrr(op::FSUB, *dst, *lhs, *rhs),
        Instruction::FMul { dst, lhs, rhs } => encode_rrr(op::FMUL, *dst, *lhs, *rhs),
        Instruction::FDiv { dst, lhs, rhs } => encode_rrr(op::FDIV, *dst, *lhs, *rhs),
        Instruction::FEq { dst, lhs, rhs } => encode_rrr(op::FEQ, *dst, *lhs, *rhs),
        Instruction::FLt { dst, lhs, rhs } => encode_rrr(op::FLT, *dst, *lhs, *rhs),
        Instruction::FGt { dst, lhs, rhs } => encode_rrr(op::FGT, *dst, *lhs, *rhs),
        Instruction::ZAdd { dst, lhs, rhs } => encode_rrr(op::ZADD, *dst, *lhs, *rhs),
        Instruction::ZSub { dst, lhs, rhs } => encode_rrr(op::ZSUB, *dst, *lhs, *rhs),
        Instruction::ZMul { dst, lhs, rhs } => encode_rrr(op::ZMUL, *dst, *lhs, *rhs),
        Instruction::ZDiv { dst, lhs, rhs } => encode_rrr(op::ZDIV, *dst, *lhs, *rhs),

        // -- RR-format (2-register) -------------------------------------------
        Instruction::INot { dst, src } => encode_rr(op::INOT, *dst, *src),
        Instruction::CvtIF { dst_f, src_i } => encode_rr(op::CVTIF, *dst_f, *src_i),
        Instruction::CvtFI { dst_i, src_f } => encode_rr(op::CVTFI, *dst_i, *src_f),
        Instruction::CvtFZ { dst_z, src_f } => encode_rr(op::CVTFZ, *dst_z, *src_f),
        Instruction::CvtZF { dst_f, src_z } => encode_rr(op::CVTZF, *dst_f, *src_z),

        // -- Configuration query (R1-format, encoded as RR with src=0) --------
        Instruction::IQCfg { dst } => encode_rr(op::IQCFG, *dst, 0),
        Instruction::ICCfg { dst } => encode_rr(op::ICCFG, *dst, 0),
        Instruction::ITid { dst } => encode_rr(op::ITID, *dst, 0),

        // -- RRS-format (2-register + shift) ----------------------------------
        Instruction::IShl { dst, src, amt } => encode_rrs(op::ISHL, *dst, *src, *amt),
        Instruction::IShr { dst, src, amt } => encode_rrs(op::ISHR, *dst, *src, *amt),

        // -- RI-format (register + 16-bit immediate) --------------------------
        Instruction::ILdi { dst, imm } => encode_ri(op::ILDI, *dst, *imm),
        Instruction::FLdi { dst, imm } => encode_ri(op::FLDI, *dst, *imm),

        // -- ZI-format (complex immediate) ------------------------------------
        Instruction::ZLdi { dst, imm_re, imm_im } => {
            encode_zi(op::ZLDI, *dst, *imm_re, *imm_im)
        }

        // -- RA-format (register + 16-bit address) ----------------------------
        Instruction::ILdm { dst, addr } => encode_ra(op::ILDM, *dst, *addr),
        Instruction::IStr { src, addr } => encode_ra(op::ISTR, *src, *addr),
        Instruction::FLdm { dst, addr } => encode_ra(op::FLDM, *dst, *addr),
        Instruction::FStr { src, addr } => encode_ra(op::FSTR, *src, *addr),
        Instruction::ZLdm { dst, addr } => encode_ra(op::ZLDM, *dst, *addr),
        Instruction::ZStr { src, addr } => encode_ra(op::ZSTR, *src, *addr),

        // -- RR-format (register-indirect memory) -----------------------------
        Instruction::ILdx { dst, addr_reg } => encode_rr(op::ILDX, *dst, *addr_reg),
        Instruction::IStrx { src, addr_reg } => encode_rr(op::ISTRX, *src, *addr_reg),
        Instruction::FLdx { dst, addr_reg } => encode_rr(op::FLDX, *dst, *addr_reg),
        Instruction::FStrx { src, addr_reg } => encode_rr(op::FSTRX, *src, *addr_reg),
        Instruction::ZLdx { dst, addr_reg } => encode_rr(op::ZLDX, *dst, *addr_reg),
        Instruction::ZStrx { src, addr_reg } => encode_rr(op::ZSTRX, *src, *addr_reg),

        // -- J-format (24-bit jump address) -----------------------------------
        Instruction::Jmp { target } => {
            let addr = resolve_label(target, label_map)?;
            encode_j(op::JMP, addr)
        }
        Instruction::Call { target } => {
            let addr = resolve_label(target, label_map)?;
            encode_j(op::CALL, addr)
        }

        // -- JR-format (predicate + 16-bit address) ---------------------------
        Instruction::Jif { pred, target } => {
            let addr = resolve_label_u16(target, label_map)?;
            encode_jr(op::JIF, *pred, addr)
        }
        Instruction::JmpF { flag, target } => {
            let addr = resolve_label_u16(target, label_map)?;
            encode_jr(op::JMPF, u8::from(*flag), addr)
        }
        Instruction::SetIV { trap_id, target } => {
            let addr = resolve_label_u16(target, label_map)?;
            encode_jr(op::SETIV, u8::from(*trap_id), addr)
        }

        // -- QP-format (quantum prepare) --------------------------------------
        Instruction::QPrep { dst, dist } => encode_qp(op::QPREP, *dst, u8::from(*dist)),

        // -- Q-format (quantum kernel) ----------------------------------------
        Instruction::QKernel {
            dst,
            src,
            kernel,
            ctx0,
            ctx1,
        } => encode_q(op::QKERNEL, *dst, *src, u8::from(*kernel), *ctx0, *ctx1),

        Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 } =>
            encode_q(op::QKERNELF, *dst, *src, u8::from(*kernel), *fctx0, *fctx1),

        Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 } =>
            encode_q(op::QKERNELZ, *dst, *src, u8::from(*kernel), *zctx0, *zctx1),

        // -- QMK-format (masked gate operations) ----------------------------------
        Instruction::QHadM { dst, src, mask_reg } =>
            encode_qmk(op::QHADM, *dst, *src, *mask_reg),
        Instruction::QFlip { dst, src, mask_reg } =>
            encode_qmk(op::QFLIP, *dst, *src, *mask_reg),
        Instruction::QPhase { dst, src, mask_reg } =>
            encode_qmk(op::QPHASE, *dst, *src, *mask_reg),

        // -- Q2R2-format (two-qubit CNOT gate) ------------------------------------
        Instruction::QCnot { dst, src, ctrl_qubit_reg, tgt_qubit_reg } =>
            encode_q2r2(op::QCNOT, *dst, *src, *ctrl_qubit_reg, *tgt_qubit_reg),

        // -- QROT-format (parameterized rotation) ---------------------------------
        Instruction::QRot { dst, src, qubit_reg, axis, angle_freg } =>
            encode_qrot(op::QROT, *dst, *src, *qubit_reg, u8::from(*axis), *angle_freg),

        // -- QMEAS-format (partial measurement) -----------------------------------
        Instruction::QMeas { dst_r, src_q, qubit_reg } => {
            validate_reg4(*dst_r, "dst_r")?;
            validate_reg3(*src_q, "src_q")?;
            validate_reg4(*qubit_reg, "qubit_reg")?;
            Ok(((op::QMEAS as u32) << 24)
                | ((*dst_r as u32) << 20)
                | ((*src_q as u32) << 17)
                | ((*qubit_reg as u32) << 13))
        }

        // -- QQQ-format (tensor product: 3 Q-register fields) --------------------
        Instruction::QTensor { dst, src0, src1 } =>
            encode_qqq(op::QTENSOR, *dst, *src0, *src1),

        // -- Q2R2-format (custom unitary, CZ, SWAP) ------------------------------
        Instruction::QCustom { dst, src, base_addr_reg, dim_reg } =>
            encode_q2r2(op::QCUSTOM, *dst, *src, *base_addr_reg, *dim_reg),

        Instruction::QCz { dst, src, ctrl_qubit_reg, tgt_qubit_reg } =>
            encode_q2r2(op::QCZ, *dst, *src, *ctrl_qubit_reg, *tgt_qubit_reg),

        Instruction::QSwap { dst, src, qubit_a_reg, qubit_b_reg } =>
            encode_q2r2(op::QSWAP, *dst, *src, *qubit_a_reg, *qubit_b_reg),

        // -- QMIXED-format: [opcode:8][dst:3][base_addr_reg:4][count_reg:4][pad:13]
        Instruction::QMixed { dst, base_addr_reg, count_reg } => {
            validate_reg3(*dst, "dst")?;
            validate_reg4(*base_addr_reg, "base_addr_reg")?;
            validate_reg4(*count_reg, "count_reg")?;
            Ok(((op::QMIXED as u32) << 24)
                | ((*dst as u32) << 21)
                | ((*base_addr_reg as u32) << 17)
                | ((*count_reg as u32) << 13))
        }

        // -- QPREPN-format: [opcode:8][dst:3][dist:3][qubit_count_reg:4][pad:14]
        Instruction::QPrepN { dst, dist, qubit_count_reg } => {
            validate_reg3(*dst, "dst")?;
            let dist_u8 = u8::from(*dist);
            validate_reg4(*qubit_count_reg, "qubit_count_reg")?;
            Ok(((op::QPREPN as u32) << 24)
                | ((*dst as u32) << 21)
                | ((dist_u8 as u32) << 18)
                | ((*qubit_count_reg as u32) << 14))
        }

        // -- Trig functions (RR and RRR format) --
        Instruction::FSin { dst, src } => encode_rr(op::FSIN, *dst, *src),
        Instruction::FCos { dst, src } => encode_rr(op::FCOS, *dst, *src),
        Instruction::FAtan2 { dst, lhs, rhs } => encode_rrr(op::FATAN2, *dst, *lhs, *rhs),
        Instruction::FSqrt { dst, src } => encode_rr(op::FSQRT, *dst, *src),

        // -- QPTRACE: QMK-format (dst:3, src:3, reg:4) --
        Instruction::QPtrace { dst, src, num_qubits_a_reg } =>
            encode_qmk(op::QPTRACE, *dst, *src, *num_qubits_a_reg),

        // -- QRESET: QMK-format (dst:3, src:3, reg:4) --
        Instruction::QReset { dst, src, qubit_reg } =>
            encode_qmk(op::QRESET, *dst, *src, *qubit_reg),

        // -- QPREPS-format: [opcode:8][dst_q:3][z_start:3][count:3][pad:15]
        Instruction::QPreps { dst, z_start, count } => {
            validate_reg3(*dst, "dst")?;
            if *z_start > MAX_REG3 {
                return Err(CqamError::OperandOverflow {
                    field: "z_start".to_string(),
                    value: *z_start as u32,
                    max: MAX_REG3 as u32,
                });
            }
            if *count > MAX_REG3 {
                return Err(CqamError::OperandOverflow {
                    field: "count".to_string(),
                    value: *count as u32,
                    max: MAX_REG3 as u32,
                });
            }
            Ok(((op::QPREPS as u32) << 24)
                | ((*dst as u32) << 21)
                | ((*z_start as u32) << 18)
                | ((*count as u32) << 15))
        }

        // -- QPREPSM-format: [opcode:8][dst_q:3][r_base:4][r_count:4][pad:13]
        Instruction::QPrepsm { dst, r_base, r_count } => {
            validate_reg3(*dst, "dst")?;
            validate_reg4(*r_base, "r_base")?;
            validate_reg4(*r_count, "r_count")?;
            Ok(((op::QPREPSM as u32) << 24)
                | ((*dst as u32) << 21)
                | ((*r_base as u32) << 17)
                | ((*r_count as u32) << 13))
        }

        // -- QR-format (quantum prepare from register) ----------------------------
        Instruction::QPrepR { dst, dist_reg } =>
            encode_qr(op::QPREPR, *dst, *dist_reg),

        // -- QE-format (quantum encode from registers) ----------------------------
        Instruction::QEncode { dst, src_base, count, file_sel } =>
            encode_qe(op::QENCODE, *dst, *src_base, *count, u8::from(*file_sel)),

        // -- QO-format (quantum observe, extended) --------------------------------
        Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 } =>
            encode_qo_ext(op::QOBSERVE, *dst_h, *src_q, u8::from(*mode), *ctx0, *ctx1),

        // -- QS-format (quantum memory) ---------------------------------------
        Instruction::QLoad { dst_q, addr } => encode_qs(op::QLOAD, *dst_q, *addr),
        Instruction::QStore { src_q, addr } => encode_qs(op::QSTORE, *src_q, *addr),

        // -- HR-format (hybrid reduce) ----------------------------------------
        Instruction::HReduce { src, dst, func } => encode_hr(op::HREDUCE, *src, *dst, u8::from(*func)),

        // -- L-format (label pseudo-instruction) ------------------------------
        Instruction::Label(name) => {
            // Use the word address as the label_id when it fits in 16 bits;
            // the assembler assigns proper sequential IDs during assembly.
            let addr = label_map.get(name).copied().unwrap_or(0);
            let label_id = if addr <= 0xFFFF { addr as u16 } else { 0 };
            Ok(encode_l(op::LABEL, label_id))
        }
    }
}

/// Encode a label pseudo-instruction with its numeric ID.
///
/// This is a convenience wrapper for the assembler, which assigns
/// sequential numeric IDs to labels rather than using the label_map
/// address approach in `encode()`.
///
/// # Arguments
///
/// * `label_id` - The numeric ID assigned to this label during pass 1.
///
/// # Returns
///
/// A u32 word in L-format: `[LABEL:8][label_id:16][_:8]`.
pub fn encode_label(label_id: u16) -> u32 {
    encode_l(op::LABEL, label_id)
}

// =============================================================================
// Internal encoding helpers
// =============================================================================

/// Encode an RRR-format word: [opcode:8][dst:4][lhs:4][rhs:4][_:12]
fn encode_rrr(opcode: u8, dst: u8, lhs: u8, rhs: u8) -> Result<u32, CqamError> {
    validate_reg4(dst, "dst")?;
    validate_reg4(lhs, "lhs")?;
    validate_reg4(rhs, "rhs")?;
    Ok(((opcode as u32) << 24)
        | ((dst as u32) << 20)
        | ((lhs as u32) << 16)
        | ((rhs as u32) << 12))
}

/// Encode an RR-format word: [opcode:8][dst:4][src:4][_:16]
fn encode_rr(opcode: u8, dst: u8, src: u8) -> Result<u32, CqamError> {
    validate_reg4(dst, "dst")?;
    validate_reg4(src, "src")?;
    Ok(((opcode as u32) << 24)
        | ((dst as u32) << 20)
        | ((src as u32) << 16))
}

/// Encode an RRS-format word: [opcode:8][dst:4][src:4][amt:6][_:10]
fn encode_rrs(opcode: u8, dst: u8, src: u8, amt: u8) -> Result<u32, CqamError> {
    validate_reg4(dst, "dst")?;
    validate_reg4(src, "src")?;
    if amt > MAX_SHIFT {
        return Err(CqamError::OperandOverflow {
            field: "shift_amount".to_string(),
            value: amt as u32,
            max: MAX_SHIFT as u32,
        });
    }
    Ok(((opcode as u32) << 24)
        | ((dst as u32) << 20)
        | ((src as u32) << 16)
        | ((amt as u32) << 10))
}

/// Encode an RI-format word: [opcode:8][dst:4][_:4][imm16:16]
fn encode_ri(opcode: u8, dst: u8, imm: i16) -> Result<u32, CqamError> {
    validate_reg4(dst, "dst")?;
    Ok(((opcode as u32) << 24)
        | ((dst as u32) << 20)
        | ((imm as u16) as u32))
}

/// Encode an RA-format word: [opcode:8][reg:4][_:4][addr16:16]
fn encode_ra(opcode: u8, reg: u8, addr: u16) -> Result<u32, CqamError> {
    validate_reg4(reg, "reg")?;
    Ok(((opcode as u32) << 24)
        | ((reg as u32) << 20)
        | (addr as u32))
}

/// Encode a J-format word: [opcode:8][addr24:24]
fn encode_j(opcode: u8, addr: u32) -> Result<u32, CqamError> {
    if addr > MAX_ADDR24 {
        return Err(CqamError::AddressOverflow {
            label: String::new(),
            address: addr,
            max_bits: 24,
        });
    }
    Ok(((opcode as u32) << 24) | (addr & 0x00FF_FFFF))
}

/// Encode a JR-format word: [opcode:8][pred:4][_:4][addr16:16]
fn encode_jr(opcode: u8, pred: u8, addr: u16) -> Result<u32, CqamError> {
    validate_reg4(pred, "pred")?;
    Ok(((opcode as u32) << 24)
        | ((pred as u32) << 20)
        | (addr as u32))
}

/// Encode a Q-format word: [opcode:8][dst_q:3][src_q:3][kernel:5][ctx0:4][ctx1:4][_:5]
fn encode_q(
    opcode: u8,
    dst_q: u8,
    src_q: u8,
    kernel: u8,
    ctx0: u8,
    ctx1: u8,
) -> Result<u32, CqamError> {
    validate_reg3(dst_q, "dst_q")?;
    validate_reg3(src_q, "src_q")?;
    if kernel > MAX_KERNEL {
        return Err(CqamError::OperandOverflow {
            field: "kernel_id".to_string(),
            value: kernel as u32,
            max: MAX_KERNEL as u32,
        });
    }
    validate_reg4(ctx0, "ctx0")?;
    validate_reg4(ctx1, "ctx1")?;
    Ok(((opcode as u32) << 24)
        | ((dst_q as u32) << 21)
        | ((src_q as u32) << 18)
        | ((kernel as u32) << 13)
        | ((ctx0 as u32) << 9)
        | ((ctx1 as u32) << 5))
}

/// Encode a Q2R2-format word: [opcode:8][q_dst:3][q_src:3][r_a:4][r_b:4][pad:10]
fn encode_q2r2(opcode: u8, q_dst: u8, q_src: u8, r_a: u8, r_b: u8) -> Result<u32, CqamError> {
    validate_reg3(q_dst, "q_dst")?;
    validate_reg3(q_src, "q_src")?;
    validate_reg4(r_a, "r_a")?;
    validate_reg4(r_b, "r_b")?;
    Ok(((opcode as u32) << 24)
        | ((q_dst as u32) << 21)
        | ((q_src as u32) << 18)
        | ((r_a as u32) << 14)
        | ((r_b as u32) << 10))
}

/// Encode a QROT-format word: [opcode:8][q_dst:3][q_src:3][qubit_reg:4][axis:2][angle_freg:4][pad:8]
fn encode_qrot(opcode: u8, q_dst: u8, q_src: u8, qubit_reg: u8, axis: u8, angle_freg: u8) -> Result<u32, CqamError> {
    validate_reg3(q_dst, "q_dst")?;
    validate_reg3(q_src, "q_src")?;
    validate_reg4(qubit_reg, "qubit_reg")?;
    if axis > 2 {
        return Err(CqamError::OperandOverflow { field: "axis".to_string(), value: axis as u32, max: 2 });
    }
    validate_reg4(angle_freg, "angle_freg")?;
    Ok(((opcode as u32) << 24)
        | ((q_dst as u32) << 21)
        | ((q_src as u32) << 18)
        | ((qubit_reg as u32) << 14)
        | ((axis as u32) << 12)
        | ((angle_freg as u32) << 8))
}

/// Encode an N-format word: [opcode:8][_:24]
fn encode_n(opcode: u8) -> u32 {
    (opcode as u32) << 24
}

/// Encode a ZI-format word: [opcode:8][dst:4][_:4][imm_re:8][imm_im:8]
fn encode_zi(opcode: u8, dst: u8, imm_re: i8, imm_im: i8) -> Result<u32, CqamError> {
    validate_reg4(dst, "dst")?;
    Ok(((opcode as u32) << 24)
        | ((dst as u32) << 20)
        | (((imm_re as u8) as u32) << 8)
        | ((imm_im as u8) as u32))
}

/// Encode a QP-format word: [opcode:8][dst_q:3][dist:3][_:18]
fn encode_qp(opcode: u8, dst_q: u8, dist: u8) -> Result<u32, CqamError> {
    validate_reg3(dst_q, "dst_q")?;
    if dist > MAX_DIST {
        return Err(CqamError::OperandOverflow {
            field: "dist_id".to_string(),
            value: dist as u32,
            max: MAX_DIST as u32,
        });
    }
    Ok(((opcode as u32) << 24)
        | ((dst_q as u32) << 21)
        | ((dist as u32) << 18))
}

/// Encode a QR-format word: [opcode:8][dst_q:3][_:1][dist_reg:4][_:16]
fn encode_qr(opcode: u8, dst_q: u8, dist_reg: u8) -> Result<u32, CqamError> {
    validate_reg3(dst_q, "dst_q")?;
    validate_reg4(dist_reg, "dist_reg")?;
    Ok(((opcode as u32) << 24)
        | ((dst_q as u32) << 21)
        | ((dist_reg as u32) << 16))
}

/// Encode a QE-format word: [opcode:8][dst_q:3][_:1][src_base:4][count:4][file_sel:2][_:10]
fn encode_qe(opcode: u8, dst_q: u8, src_base: u8, count: u8, file_sel: u8) -> Result<u32, CqamError> {
    validate_reg3(dst_q, "dst_q")?;
    validate_reg4(src_base, "src_base")?;
    if count > 15 {
        return Err(CqamError::OperandOverflow {
            field: "count".to_string(),
            value: count as u32,
            max: 15,
        });
    }
    if file_sel > MAX_FILE_SEL {
        return Err(CqamError::OperandOverflow {
            field: "file_sel".to_string(),
            value: file_sel as u32,
            max: MAX_FILE_SEL as u32,
        });
    }
    Ok(((opcode as u32) << 24)
        | ((dst_q as u32) << 21)
        | ((src_base as u32) << 16)
        | ((count as u32) << 12)
        | ((file_sel as u32) << 10))
}

/// Encode a QMK-format word: [opcode:8][dst_q:3][src_q:3][_:4][mask_reg:4][_:10]
fn encode_qmk(opcode: u8, dst_q: u8, src_q: u8, mask_reg: u8) -> Result<u32, CqamError> {
    validate_reg3(dst_q, "dst_q")?;
    validate_reg3(src_q, "src_q")?;
    validate_reg4(mask_reg, "mask_reg")?;
    Ok(((opcode as u32) << 24)
        | ((dst_q as u32) << 21)
        | ((src_q as u32) << 18)
        | ((mask_reg as u32) << 10))
}

/// Encode a QQQ-format word: [opcode:8][q0:3][q1:3][q2:3][_:15]
fn encode_qqq(opcode: u8, q0: u8, q1: u8, q2: u8) -> Result<u32, CqamError> {
    validate_reg3(q0, "q0")?;
    validate_reg3(q1, "q1")?;
    validate_reg3(q2, "q2")?;
    Ok(((opcode as u32) << 24)
        | ((q0 as u32) << 21)
        | ((q1 as u32) << 18)
        | ((q2 as u32) << 15))
}

/// Encode a QS-format word: [opcode:8][qreg:3][_:5][addr:8][_:8]
fn encode_qs(opcode: u8, qreg: u8, addr: u8) -> Result<u32, CqamError> {
    validate_reg3(qreg, "qreg")?;
    Ok(((opcode as u32) << 24)
        | ((qreg as u32) << 21)
        | ((addr as u32) << 8))
}

/// Encode extended QO-format word:
/// [opcode:8][dst_h:3][src_q:3][mode:2][ctx0:4][ctx1:4][_:8]
fn encode_qo_ext(
    opcode: u8,
    dst_h: u8,
    src_q: u8,
    mode: u8,
    ctx0: u8,
    ctx1: u8,
) -> Result<u32, CqamError> {
    validate_reg3(dst_h, "dst_h")?;
    validate_reg3(src_q, "src_q")?;
    if mode > MAX_MODE {
        return Err(CqamError::OperandOverflow {
            field: "mode".to_string(),
            value: mode as u32,
            max: MAX_MODE as u32,
        });
    }
    validate_reg4(ctx0, "ctx0")?;
    validate_reg4(ctx1, "ctx1")?;
    Ok(((opcode as u32) << 24)
        | ((dst_h as u32) << 21)
        | ((src_q as u32) << 18)
        | ((mode as u32) << 16)
        | ((ctx0 as u32) << 12)
        | ((ctx1 as u32) << 8))
}

/// Encode an HR-format word: [opcode:8][src:4][dst:4][func:5][_:11]
fn encode_hr(opcode: u8, src: u8, dst: u8, func: u8) -> Result<u32, CqamError> {
    validate_reg4(src, "src")?;
    validate_reg4(dst, "dst")?;
    if func > MAX_FUNC {
        return Err(CqamError::OperandOverflow {
            field: "reduce_func".to_string(),
            value: func as u32,
            max: MAX_FUNC as u32,
        });
    }
    Ok(((opcode as u32) << 24)
        | ((src as u32) << 20)
        | ((dst as u32) << 16)
        | ((func as u32) << 11))
}

/// Encode an L-format word: [opcode:8][label_id:16][_:8]
fn encode_l(opcode: u8, label_id: u16) -> u32 {
    ((opcode as u32) << 24) | ((label_id as u32) << 8)
}

// =============================================================================
// Validation helpers
// =============================================================================

/// Validate a 4-bit register index (0-15).
fn validate_reg4(idx: u8, field: &str) -> Result<(), CqamError> {
    if idx > MAX_REG4 {
        return Err(CqamError::OperandOverflow {
            field: field.to_string(),
            value: idx as u32,
            max: MAX_REG4 as u32,
        });
    }
    Ok(())
}

/// Validate a 3-bit register index (0-7).
fn validate_reg3(idx: u8, field: &str) -> Result<(), CqamError> {
    if idx > MAX_REG3 {
        return Err(CqamError::OperandOverflow {
            field: field.to_string(),
            value: idx as u32,
            max: MAX_REG3 as u32,
        });
    }
    Ok(())
}

/// Resolve a label name to a word address, returning an error if not found.
fn resolve_label(name: &str, label_map: &HashMap<String, u32>) -> Result<u32, CqamError> {
    label_map
        .get(name)
        .copied()
        .ok_or_else(|| CqamError::UnresolvedLabel(name.to_string()))
}

/// Resolve a label and ensure it fits in 16 bits (for JR-format).
fn resolve_label_u16(
    name: &str,
    label_map: &HashMap<String, u32>,
) -> Result<u16, CqamError> {
    let addr = resolve_label(name, label_map)?;
    if addr > MAX_ADDR16 {
        return Err(CqamError::AddressOverflow {
            label: name.to_string(),
            address: addr,
            max_bits: 16,
        });
    }
    Ok(addr as u16)
}
