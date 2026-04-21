//! Opcode byte constants and format-level limits for the CQAM ISA.
//!
//! Contains the `op` module (one constant per ISA instruction) and the
//! bit-width limits used by the encoder and decoder.

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

    // -- Environment call (0x2F) -----------------------------------------------
    pub const ECALL: u8 = 0x2F;

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

    // -- Quantum kernel operations (0x40-0x44) --------------------------------
    // 0x40 reserved (was QSAMPLE, removed: violates physical realism)
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

    // -- Product state preparation (0x5D-0x5E) --------------------------------
    pub const QPREPS: u8 = 0x5D;
    pub const QPREPSM: u8 = 0x5E;

    // -- Configuration query (0x58) -------------------------------------------
    pub const IQCFG: u8 = 0x58;

    // -- Thread configuration (0x59-0x5C) -------------------------------------
    pub const ICCFG: u8 = 0x59;
    pub const ITID: u8 = 0x5A;
    pub const HATMS: u8 = 0x5B;
    pub const HATME: u8 = 0x5C;

    // -- Integer increment/decrement (0x60-0x61) ------------------------------
    pub const IINC: u8 = 0x60;
    pub const IDEC: u8 = 0x61;

    // -- Register move (0x62-0x64) -------------------------------------------
    pub const IMOV: u8 = 0x62;
    pub const FMOV: u8 = 0x63;
    pub const ZMOV: u8 = 0x64;

    // -- Quantum register handle swap (0x65) ----------------------------------
    pub const QXCH: u8 = 0x65;

    // -- Hybrid operations (0x38-0x3B) ----------------------------------------
    pub const HFORK: u8 = 0x38;
    pub const HMERGE: u8 = 0x39;
    pub const JMPF: u8 = 0x3A;
    pub const HREDUCE: u8 = 0x3B;
}

// =============================================================================
// Format helper constants
// =============================================================================

/// Maximum value for a 4-bit register index (R/F/Z files).
pub(super) const MAX_REG4: u8 = 15;

/// Maximum value for a 3-bit register index (Q/H files).
pub(super) const MAX_REG3: u8 = 7;

/// Maximum value for a 6-bit shift amount.
pub(super) const MAX_SHIFT: u8 = 63;

/// Maximum value for a 5-bit kernel ID.
pub(super) const MAX_KERNEL: u8 = 31;

/// Maximum value for a 5-bit function ID.
pub(super) const MAX_FUNC: u8 = 31;

/// Maximum value for a 2-bit mode field.
pub(super) const MAX_MODE: u8 = 3;

/// Maximum value for a 3-bit distribution ID.
pub(super) const MAX_DIST: u8 = 7;

/// Maximum value for a 2-bit file selector field.
pub(super) const MAX_FILE_SEL: u8 = 2;

/// Maximum value for a 16-bit address.
pub(super) const MAX_ADDR16: u32 = 0xFFFF;

/// Maximum value for a 24-bit address.
pub(super) const MAX_ADDR24: u32 = 0x00FF_FFFF;
