//! Binary opcode encoding and decoding for the CQAM ISA.
//!
//! Every `Instruction` variant maps to a unique 8-bit opcode. The remaining
//! 24 bits carry operands in one of several fixed formats (N, RR, RRR, RI,
//! RA, J, JR, Q, QP, QO, QS, HR, ZI, L). See `reference/opcodes.md` for
//! the complete bit-level layout of each format.

use std::collections::HashMap;

use crate::error::CqamError;
use crate::instruction::Instruction;

// =============================================================================
// Opcode constants
// =============================================================================

/// Opcode byte constants for every ISA instruction.
///
/// Grouped by domain with reserved gaps for future expansion.
/// See `reference/opcodes.md` for the complete opcode table.
pub mod op {
    // -- NOP (0x00) -----------------------------------------------------------
    pub const NOP: u8 = 0x00;

    // -- Integer operations (0x01-0x11) ---------------------------------------
    pub const IADD: u8 = 0x01;
    pub const ISUB: u8 = 0x02;
    pub const IMUL: u8 = 0x03;
    pub const IDIV: u8 = 0x04;
    pub const IMOD: u8 = 0x05;
    pub const IAND: u8 = 0x06;
    pub const IOR: u8 = 0x07;
    pub const IXOR: u8 = 0x08;
    pub const INOT: u8 = 0x09;
    pub const ISHL: u8 = 0x0A;
    pub const ISHR: u8 = 0x0B;
    pub const ILDI: u8 = 0x0C;
    pub const ILDM: u8 = 0x0D;
    pub const ISTR: u8 = 0x0E;
    pub const IEQ: u8 = 0x0F;
    pub const ILT: u8 = 0x10;
    pub const IGT: u8 = 0x11;

    // -- Float operations (0x12-0x1B) -----------------------------------------
    pub const FADD: u8 = 0x12;
    pub const FSUB: u8 = 0x13;
    pub const FMUL: u8 = 0x14;
    pub const FDIV: u8 = 0x15;
    pub const FLDI: u8 = 0x16;
    pub const FLDM: u8 = 0x17;
    pub const FSTR: u8 = 0x18;
    pub const FEQ: u8 = 0x19;
    pub const FLT: u8 = 0x1A;
    pub const FGT: u8 = 0x1B;

    // -- Complex operations (0x1C-0x22) ---------------------------------------
    pub const ZADD: u8 = 0x1C;
    pub const ZSUB: u8 = 0x1D;
    pub const ZMUL: u8 = 0x1E;
    pub const ZDIV: u8 = 0x1F;
    pub const ZLDI: u8 = 0x20;
    pub const ZLDM: u8 = 0x21;
    pub const ZSTR: u8 = 0x22;

    // -- Type conversions (0x23-0x26) -----------------------------------------
    pub const CVTIF: u8 = 0x23;
    pub const CVTFI: u8 = 0x24;
    pub const CVTFZ: u8 = 0x25;
    pub const CVTZF: u8 = 0x26;

    // -- Control flow (0x27-0x2C) ---------------------------------------------
    pub const JMP: u8 = 0x27;
    pub const JIF: u8 = 0x28;
    pub const CALL: u8 = 0x29;
    pub const RET: u8 = 0x2A;
    pub const HALT: u8 = 0x2B;
    pub const LABEL: u8 = 0x2C;

    // -- Interrupt handling (0x2D-0x2E) ---------------------------------------
    pub const RETI: u8 = 0x2D;
    pub const SETIV: u8 = 0x2E;

    // -- Quantum operations (0x30-0x34) ---------------------------------------
    pub const QPREP: u8 = 0x30;
    pub const QKERNEL: u8 = 0x31;
    pub const QOBSERVE: u8 = 0x32;
    pub const QLOAD: u8 = 0x33;
    pub const QSTORE: u8 = 0x34;

    // -- Register-indirect memory (0x35-0x37, 0x3C-0x3E) ---------------------
    pub const ILDX: u8 = 0x35;
    pub const ISTRX: u8 = 0x36;
    pub const FLDX: u8 = 0x37;
    pub const FSTRX: u8 = 0x3C;
    pub const ZLDX: u8 = 0x3D;
    pub const ZSTRX: u8 = 0x3E;

    // -- Quantum sampling and kernel operations (0x40-0x44) -------------------
    pub const QSAMPLE: u8 = 0x40;
    pub const QKERNELF: u8 = 0x41;
    pub const QKERNELZ: u8 = 0x42;
    pub const QPREPR: u8 = 0x43;
    pub const QENCODE: u8 = 0x44;

    // -- Masked register-level gate operations (0x45-0x47) --------------------
    pub const QHADM: u8 = 0x45;
    pub const QFLIP: u8 = 0x46;
    pub const QPHASE: u8 = 0x47;

    // -- Qubit-level gate operations (0x48-0x4A) ------------------------------
    pub const QCNOT: u8 = 0x48;
    pub const QROT: u8 = 0x49;
    pub const QMEAS: u8 = 0x4A;

    // -- Extended quantum operations (0x4B-0x4E) ------------------------------
    pub const QTENSOR: u8 = 0x4B;
    pub const QCUSTOM: u8 = 0x4C;
    pub const QCZ: u8 = 0x4D;
    pub const QSWAP: u8 = 0x4E;

    // -- Mixed-state, partial-trace, reset, and float math (0x4F-0x57) --------
    pub const QMIXED: u8 = 0x4F;
    pub const QPREPN: u8 = 0x51;
    pub const FSIN: u8 = 0x52;
    pub const FCOS: u8 = 0x53;
    pub const FATAN2: u8 = 0x54;
    pub const FSQRT: u8 = 0x55;
    pub const QPTRACE: u8 = 0x56;
    pub const QRESET: u8 = 0x57;

    // -- Configuration query (0x58) -------------------------------------------
    pub const IQCFG: u8 = 0x58;

    // -- Hybrid operations (0x38-0x3B) ----------------------------------------
    pub const HFORK: u8 = 0x38;
    pub const HMERGE: u8 = 0x39;
    pub const HCEXEC: u8 = 0x3A;
    pub const HREDUCE: u8 = 0x3B;
}

// =============================================================================
// Format helper constants
// =============================================================================

/// Maximum value for a 4-bit register index (R/F/Z files).
const MAX_REG4: u8 = 15;

/// Maximum value for a 3-bit register index (Q/H files).
const MAX_REG3: u8 = 7;

/// Maximum value for a 6-bit shift amount.
const MAX_SHIFT: u8 = 63;

/// Maximum value for a 5-bit kernel ID.
const MAX_KERNEL: u8 = 31;

/// Maximum value for a 5-bit function ID.
const MAX_FUNC: u8 = 31;

/// Maximum value for a 2-bit mode field.
const MAX_MODE: u8 = 3;

/// Maximum value for a 3-bit distribution ID.
const MAX_DIST: u8 = 7;

/// Maximum value for a 2-bit file selector field.
const MAX_FILE_SEL: u8 = 2;

/// Maximum value for a 16-bit address.
const MAX_ADDR16: u32 = 0xFFFF;

/// Maximum value for a 24-bit address.
const MAX_ADDR24: u32 = 0x00FF_FFFF;

// =============================================================================
// Encoding
// =============================================================================

/// Encode an [`Instruction`] into a 32-bit instruction word.
///
/// # Arguments
///
/// * `instr` - The instruction to encode.
/// * `label_map` - Maps label names to resolved word addresses. Required for
///   encoding JMP, JIF, CALL, HCEXEC, and LABEL instructions.
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
        Instruction::Reti => Ok(encode_n(op::RETI)),

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
        Instruction::HCExec { flag, target } => {
            let addr = resolve_label_u16(target, label_map)?;
            encode_jr(op::HCEXEC, *flag, addr)
        }
        Instruction::SetIV { trap_id, target } => {
            let addr = resolve_label_u16(target, label_map)?;
            encode_jr(op::SETIV, *trap_id, addr)
        }

        // -- QP-format (quantum prepare) --------------------------------------
        Instruction::QPrep { dst, dist } => encode_qp(op::QPREP, *dst, *dist),

        // -- Q-format (quantum kernel) ----------------------------------------
        Instruction::QKernel {
            dst,
            src,
            kernel,
            ctx0,
            ctx1,
        } => encode_q(op::QKERNEL, *dst, *src, *kernel, *ctx0, *ctx1),

        Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 } =>
            encode_q(op::QKERNELF, *dst, *src, *kernel, *fctx0, *fctx1),

        Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 } =>
            encode_q(op::QKERNELZ, *dst, *src, *kernel, *zctx0, *zctx1),

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
            encode_qrot(op::QROT, *dst, *src, *qubit_reg, *axis, *angle_freg),

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
            if *dist > MAX_DIST {
                return Err(CqamError::OperandOverflow { field: "dist".to_string(), value: *dist as u32, max: MAX_DIST as u32 });
            }
            validate_reg4(*qubit_count_reg, "qubit_count_reg")?;
            Ok(((op::QPREPN as u32) << 24)
                | ((*dst as u32) << 21)
                | ((*dist as u32) << 18)
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

        // -- QR-format (quantum prepare from register) ----------------------------
        Instruction::QPrepR { dst, dist_reg } =>
            encode_qr(op::QPREPR, *dst, *dist_reg),

        // -- QE-format (quantum encode from registers) ----------------------------
        Instruction::QEncode { dst, src_base, count, file_sel } =>
            encode_qe(op::QENCODE, *dst, *src_base, *count, *file_sel),

        // -- QO-format (quantum observe, extended) --------------------------------
        Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 } =>
            encode_qo_ext(op::QOBSERVE, *dst_h, *src_q, *mode, *ctx0, *ctx1),
        Instruction::QSample { dst_h, src_q, mode, ctx0, ctx1 } =>
            encode_qo_ext(op::QSAMPLE, *dst_h, *src_q, *mode, *ctx0, *ctx1),

        // -- QS-format (quantum memory) ---------------------------------------
        Instruction::QLoad { dst_q, addr } => encode_qs(op::QLOAD, *dst_q, *addr),
        Instruction::QStore { src_q, addr } => encode_qs(op::QSTORE, *src_q, *addr),

        // -- HR-format (hybrid reduce) ----------------------------------------
        Instruction::HReduce { src, dst, func } => encode_hr(op::HREDUCE, *src, *dst, *func),

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
        op::RETI => Ok(Instruction::Reti),

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
        op::HCEXEC => {
            let flag = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::HCExec {
                flag,
                target: format!("@{}", addr),
            })
        }
        op::SETIV => {
            let trap_id = extract_reg4(word, 20);
            let addr = extract_u16(word);
            Ok(Instruction::SetIV {
                trap_id,
                target: format!("@{}", addr),
            })
        }

        // -- QP-format (quantum prepare) --------------------------------------
        op::QPREP => {
            let dst = extract_reg3(word, 21);
            let dist = extract_reg3(word, 18);
            Ok(Instruction::QPrep { dst, dist })
        }

        // -- Q-format (quantum kernel) ----------------------------------------
        op::QKERNEL => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let kernel = extract_u5(word, 13);
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
            let kernel = extract_u5(word, 13);
            let fctx0 = extract_reg4(word, 9);
            let fctx1 = extract_reg4(word, 5);
            Ok(Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 })
        }

        // -- Q-format (quantum kernel with complex context) -------------------
        op::QKERNELZ => {
            let dst = extract_reg3(word, 21);
            let src = extract_reg3(word, 18);
            let kernel = extract_u5(word, 13);
            let zctx0 = extract_reg4(word, 9);
            let zctx1 = extract_reg4(word, 5);
            Ok(Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 })
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
            let file_sel = extract_u2(word, 10);
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
            let axis = ((word >> 12) & 0x3) as u8;
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
            let dist = extract_reg3(word, 18);
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

        // -- QO-format (quantum observe, extended) --------------------------------
        op::QOBSERVE => {
            let dst_h = extract_reg3(word, 21);
            let src_q = extract_reg3(word, 18);
            let mode = extract_u2(word, 16);
            let ctx0 = extract_reg4(word, 12);
            let ctx1 = extract_reg4(word, 8);
            Ok(Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 })
        }
        op::QSAMPLE => {
            let dst_h = extract_reg3(word, 21);
            let src_q = extract_reg3(word, 18);
            let mode = extract_u2(word, 16);
            let ctx0 = extract_reg4(word, 12);
            let ctx1 = extract_reg4(word, 8);
            Ok(Instruction::QSample { dst_h, src_q, mode, ctx0, ctx1 })
        }

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
            let func = extract_u5(word, 11);
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
        op::QSAMPLE => Some("QSAMPLE"),
        op::QKERNELF => Some("QKERNELF"),
        op::QKERNELZ => Some("QKERNELZ"),
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
        op::ILDX => Some("ILDX"),
        op::ISTRX => Some("ISTRX"),
        op::FLDX => Some("FLDX"),
        op::FSTRX => Some("FSTRX"),
        op::ZLDX => Some("ZLDX"),
        op::ZSTRX => Some("ZSTRX"),
        op::HFORK => Some("HFORK"),
        op::HMERGE => Some("HMERGE"),
        op::HCEXEC => Some("HCEXEC"),
        op::HREDUCE => Some("HREDUCE"),
        op::RETI => Some("RETI"),
        op::SETIV => Some("SETIV"),
        _ => None,
    }
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

// =============================================================================
// Tests (in-module unit tests)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a label map for testing.
    fn test_labels() -> HashMap<String, u32> {
        let mut m = HashMap::new();
        m.insert("loop".to_string(), 10);
        m.insert("end".to_string(), 42);
        m.insert("start".to_string(), 0);
        m
    }

    #[test]
    fn test_roundtrip_nop() {
        let labels = HashMap::new();
        let word = encode(&Instruction::Nop, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, Instruction::Nop);
    }

    #[test]
    fn test_roundtrip_iadd() {
        let labels = HashMap::new();
        let instr = Instruction::IAdd { dst: 2, lhs: 3, rhs: 4 };
        let word = encode(&instr, &labels).unwrap();
        assert_eq!(word, 0x01234000);
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, instr);
    }

    #[test]
    fn test_roundtrip_ildi_negative() {
        let labels = HashMap::new();
        let instr = Instruction::ILdi { dst: 5, imm: -100 };
        let word = encode(&instr, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, instr);
    }

    #[test]
    fn test_roundtrip_jmp() {
        let labels = test_labels();
        let instr = Instruction::Jmp { target: "end".to_string() };
        let word = encode(&instr, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, Instruction::Jmp { target: "@42".to_string() });
    }

    #[test]
    fn test_roundtrip_qkernel() {
        let labels = HashMap::new();
        let instr = Instruction::QKernel {
            dst: 1, src: 0, kernel: 2, ctx0: 3, ctx1: 4,
        };
        let word = encode(&instr, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, instr);
    }

    #[test]
    fn test_roundtrip_zldi() {
        let labels = HashMap::new();
        let instr = Instruction::ZLdi { dst: 3, imm_re: 5, imm_im: -2 };
        let word = encode(&instr, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, instr);
    }

    #[test]
    fn test_encode_unresolved_label() {
        let labels = HashMap::new();
        let instr = Instruction::Jmp { target: "nonexistent".to_string() };
        let result = encode(&instr, &labels);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_opcode() {
        let word: u32 = 0xFF_000000;
        let result = decode(word);
        assert!(result.is_err());
    }

    #[test]
    fn test_mnemonic_lookup() {
        assert_eq!(mnemonic(op::IADD), Some("IADD"));
        assert_eq!(mnemonic(op::QKERNEL), Some("QKERNEL"));
        assert_eq!(mnemonic(0xFE), None);
    }

    #[test]
    fn test_reg4_overflow() {
        let labels = HashMap::new();
        let instr = Instruction::IAdd { dst: 16, lhs: 0, rhs: 0 };
        let result = encode(&instr, &labels);
        assert!(result.is_err());
    }

    #[test]
    fn test_shift_amount_overflow() {
        let labels = HashMap::new();
        let instr = Instruction::IShl { dst: 0, src: 1, amt: 64 };
        let result = encode(&instr, &labels);
        assert!(result.is_err());
    }
}
