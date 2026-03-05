// cqam-core/src/instruction.rs
//
// Phase 2: Complete ISA definition with numeric operands.
// This file replaces the old string-based Instruction enum entirely.

/// Instruction represents a single operation in the CQAM ISA.
///
/// All register operands are `u8` indices into their respective register files:
/// - Integer registers: R0-R15 (index 0-15)
/// - Float registers: F0-F15 (index 0-15)
/// - Complex registers: Z0-Z15 (index 0-15)
/// - Quantum registers: Q0-Q7 (index 0-7)
/// - Hybrid registers: H0-H7 (index 0-7)
///
/// Memory addresses are `u16` (CMEM: 64K cells) or `u8` (QMEM: 256 slots).
/// Kernel, distribution, flag, and reduction function selectors are `u8` IDs.
/// Jump targets are label names (`String`) in the IR; Phase 5 resolves them
/// to numeric addresses during binary encoding.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // -- No-op / pseudo -------------------------------------------------------

    /// No operation. Used as placeholder during parsing for blank lines,
    /// comments, and unrecognized instructions. Filtered out by `parse_program`.
    Nop,

    /// Label definition: a named jump target, resolved during program loading.
    /// Labels are pseudo-instructions that occupy no execution time.
    Label(String),

    // -- Integer arithmetic (R-file: i64 x 16) --------------------------------

    /// Integer addition: R[dst] = R[lhs] + R[rhs]
    IAdd { dst: u8, lhs: u8, rhs: u8 },

    /// Integer subtraction: R[dst] = R[lhs] - R[rhs]
    ISub { dst: u8, lhs: u8, rhs: u8 },

    /// Integer multiplication: R[dst] = R[lhs] * R[rhs]
    IMul { dst: u8, lhs: u8, rhs: u8 },

    /// Integer division: R[dst] = R[lhs] / R[rhs]
    /// Traps on division by zero (Arithmetic trap).
    IDiv { dst: u8, lhs: u8, rhs: u8 },

    /// Integer modulo: R[dst] = R[lhs] % R[rhs]
    /// Traps on division by zero (Arithmetic trap).
    IMod { dst: u8, lhs: u8, rhs: u8 },

    // -- Integer bitwise (R-file) ---------------------------------------------

    /// Bitwise AND: R[dst] = R[lhs] & R[rhs]
    IAnd { dst: u8, lhs: u8, rhs: u8 },

    /// Bitwise OR: R[dst] = R[lhs] | R[rhs]
    IOr { dst: u8, lhs: u8, rhs: u8 },

    /// Bitwise XOR: R[dst] = R[lhs] ^ R[rhs]
    IXor { dst: u8, lhs: u8, rhs: u8 },

    /// Bitwise NOT: R[dst] = !R[src]
    INot { dst: u8, src: u8 },

    /// Shift left: R[dst] = R[src] << amt
    /// amt is a literal shift amount (0-63).
    IShl { dst: u8, src: u8, amt: u8 },

    /// Shift right (arithmetic): R[dst] = R[src] >> amt
    /// amt is a literal shift amount (0-63).
    IShr { dst: u8, src: u8, amt: u8 },

    // -- Integer memory -------------------------------------------------------

    /// Load 16-bit signed immediate into integer register.
    /// R[dst] = sign_extend(imm)
    ILdi { dst: u8, imm: i16 },

    /// Load from classical memory: R[dst] = CMEM[addr]
    ILdm { dst: u8, addr: u16 },

    /// Store to classical memory: CMEM[addr] = R[src]
    IStr { src: u8, addr: u16 },

    // -- Integer comparison ---------------------------------------------------

    /// Integer equality: R[dst] = (R[lhs] == R[rhs]) ? 1 : 0
    IEq { dst: u8, lhs: u8, rhs: u8 },

    /// Integer less-than: R[dst] = (R[lhs] < R[rhs]) ? 1 : 0
    ILt { dst: u8, lhs: u8, rhs: u8 },

    /// Integer greater-than: R[dst] = (R[lhs] > R[rhs]) ? 1 : 0
    IGt { dst: u8, lhs: u8, rhs: u8 },

    // -- Float arithmetic (F-file: f64 x 16) ----------------------------------

    /// Float addition: F[dst] = F[lhs] + F[rhs]
    FAdd { dst: u8, lhs: u8, rhs: u8 },

    /// Float subtraction: F[dst] = F[lhs] - F[rhs]
    FSub { dst: u8, lhs: u8, rhs: u8 },

    /// Float multiplication: F[dst] = F[lhs] * F[rhs]
    FMul { dst: u8, lhs: u8, rhs: u8 },

    /// Float division: F[dst] = F[lhs] / F[rhs]
    FDiv { dst: u8, lhs: u8, rhs: u8 },

    /// Load scaled immediate into float register.
    /// F[dst] = imm as f64
    FLdi { dst: u8, imm: i16 },

    /// Load from CMEM, reinterpret i64 bits as f64.
    /// F[dst] = f64::from_bits(CMEM[addr] as u64)
    FLdm { dst: u8, addr: u16 },

    /// Store f64 bits to CMEM.
    /// CMEM[addr] = F[src].to_bits() as i64
    FStr { src: u8, addr: u16 },

    /// Float equality (exact): result stored as i64 in R-file.
    /// R[dst] = (F[lhs] == F[rhs]) ? 1 : 0
    ///
    /// NOTE: Result is written to the integer register file, not the float file,
    /// because the result is boolean (0 or 1 as i64).
    FEq { dst: u8, lhs: u8, rhs: u8 },

    /// Float less-than: R[dst] = (F[lhs] < F[rhs]) ? 1 : 0
    FLt { dst: u8, lhs: u8, rhs: u8 },

    /// Float greater-than: R[dst] = (F[lhs] > F[rhs]) ? 1 : 0
    FGt { dst: u8, lhs: u8, rhs: u8 },

    // -- Complex arithmetic (Z-file: (f64, f64) x 16) ------------------------

    /// Complex addition: Z[dst] = Z[lhs] + Z[rhs]
    ZAdd { dst: u8, lhs: u8, rhs: u8 },

    /// Complex subtraction: Z[dst] = Z[lhs] - Z[rhs]
    ZSub { dst: u8, lhs: u8, rhs: u8 },

    /// Complex multiplication: Z[dst] = Z[lhs] * Z[rhs]
    /// (a+bi)(c+di) = (ac-bd) + (ad+bc)i
    ZMul { dst: u8, lhs: u8, rhs: u8 },

    /// Complex division: Z[dst] = Z[lhs] / Z[rhs]
    /// Traps if Z[rhs] == (0, 0).
    ZDiv { dst: u8, lhs: u8, rhs: u8 },

    /// Load complex immediate: Z[dst] = (imm_re as f64, imm_im as f64)
    ZLdi { dst: u8, imm_re: i8, imm_im: i8 },

    /// Load complex from two consecutive CMEM cells.
    /// Z[dst] = (f64::from_bits(CMEM[addr] as u64), f64::from_bits(CMEM[addr+1] as u64))
    ZLdm { dst: u8, addr: u16 },

    /// Store complex to two consecutive CMEM cells.
    /// CMEM[addr]   = Z[src].0.to_bits() as i64
    /// CMEM[addr+1] = Z[src].1.to_bits() as i64
    ZStr { src: u8, addr: u16 },

    // -- Register-indirect memory (Phase 4) -----------------------------------

    /// Indirect integer load: R[dst] = CMEM[R[addr_reg] as u16]
    ILdx { dst: u8, addr_reg: u8 },

    /// Indirect integer store: CMEM[R[addr_reg] as u16] = R[src]
    IStrx { src: u8, addr_reg: u8 },

    /// Indirect float load: F[dst] = f64::from_bits(CMEM[R[addr_reg] as u16])
    FLdx { dst: u8, addr_reg: u8 },

    /// Indirect float store: CMEM[R[addr_reg] as u16] = F[src].to_bits()
    FStrx { src: u8, addr_reg: u8 },

    /// Indirect complex load: Z[dst] = complex from CMEM[R[addr_reg]]..+1
    ZLdx { dst: u8, addr_reg: u8 },

    /// Indirect complex store: CMEM[R[addr_reg]]..+1 = Z[src]
    ZStrx { src: u8, addr_reg: u8 },

    // -- Type conversion ------------------------------------------------------

    /// Convert integer to float: F[dst_f] = R[src_i] as f64
    CvtIF { dst_f: u8, src_i: u8 },

    /// Convert float to integer (truncation): R[dst_i] = F[src_f] as i64
    CvtFI { dst_i: u8, src_f: u8 },

    /// Convert float to complex (zero imaginary): Z[dst_z] = (F[src_f], 0.0)
    CvtFZ { dst_z: u8, src_f: u8 },

    /// Convert complex to float (real part): F[dst_f] = Z[src_z].0
    CvtZF { dst_f: u8, src_z: u8 },

    // -- Control flow ---------------------------------------------------------

    /// Unconditional jump to label.
    /// PC = address_of(target)
    Jmp { target: String },

    /// Conditional jump: if R[pred] != 0, jump to label.
    /// PC = address_of(target) if R[pred] != 0, else PC += 1
    Jif { pred: u8, target: String },

    /// Call subroutine: push PC+1 onto call stack, jump to label.
    Call { target: String },

    /// Return from subroutine: pop call stack, jump to saved address.
    /// If call stack is empty, acts as HALT.
    Ret,

    /// Halt execution. Sets trap_halt flag in PSW.
    Halt,

    // -- Quantum (Q-file: QDist<u16> x 8) ------------------------------------

    /// Prepare quantum register with distribution.
    /// Q[dst] = new_qdist(dist)
    /// dist: 0=uniform, 1=zero, 2=bell, 3=ghz (see dist_id module)
    QPrep { dst: u8, dist: u8 },

    /// Apply quantum kernel transformation.
    /// Q[dst] = kernel(Q[src], R[ctx0], R[ctx1])
    /// kernel: kernel ID (see kernel_id module)
    /// ctx0, ctx1: integer register indices providing classical context
    QKernel { dst: u8, src: u8, kernel: u8, ctx0: u8, ctx1: u8 },

    /// Destructively observe (measure) a quantum register.
    /// H[dst_h] = measure(Q[src_q])
    /// The quantum register Q[src_q] is consumed (set to None).
    /// The measurement result is stored as HybridValue::Dist in H[dst_h].
    QObserve { dst_h: u8, src_q: u8 },

    /// Load quantum distribution from QMEM into quantum register.
    /// Q[dst_q] = QMEM[addr]
    QLoad { dst_q: u8, addr: u8 },

    /// Store quantum register into QMEM slot.
    /// QMEM[addr] = Q[src_q]
    QStore { src_q: u8, addr: u8 },

    // -- Hybrid (H-file: HybridValue x 8) ------------------------------------

    /// Fork hybrid execution. Sets PSW fork flags.
    /// (Full thread spawning deferred to later phases.)
    HFork,

    /// Merge hybrid execution branches. Sets PSW merge flags.
    /// (Full thread joining deferred to later phases.)
    HMerge,

    /// Conditional execution based on PSW flag.
    /// if PSW.flag[flag] then PC = address_of(target)
    /// flag: flag ID (see flag_id module)
    HCExec { flag: u8, target: String },

    /// Reduce hybrid value to classical value.
    /// The output register file depends on the reduction function:
    /// - round/floor/ceil/trunc/abs/negate (0-5): H[src] -> R[dst] (int)
    /// - magnitude/phase/real/imag (6-9): H[src] -> F[dst] (float)
    /// - mean/mode/argmax/variance (10-13): H[src] -> F[dst] or R[dst]
    ///   func: reduction function ID (see reduce_fn module)
    HReduce { src: u8, dst: u8, func: u8 },

    // -- Interrupt handling (Phase 8) -----------------------------------------

    /// Return from interrupt handler.
    /// Pop saved PC from call stack, clear maskable trap flags, resume.
    Reti,

    /// Set interrupt vector: register a handler address for a trap ID.
    /// trap_id: 0=Arithmetic, 1=QuantumError, 2=SyncFailure
    /// target: label name (resolved to address during encoding)
    SetIV { trap_id: u8, target: String },
}

// =============================================================================
// Named constant modules for numeric IDs
// =============================================================================

/// Trap IDs for SetIV instruction.
pub mod trap_id {
    /// Arithmetic fault (division by zero, overflow).
    pub const ARITHMETIC: u8 = 0;
    /// Quantum fidelity dropped below threshold.
    pub const QUANTUM_ERROR: u8 = 1;
    /// Hybrid branch synchronization failure.
    pub const SYNC_FAILURE: u8 = 2;
}

/// Helper: name string for a trap ID (for display/debug).
pub fn trap_id_name(id: u8) -> &'static str {
    match id {
        trap_id::ARITHMETIC => "arithmetic",
        trap_id::QUANTUM_ERROR => "quantum_error",
        trap_id::SYNC_FAILURE => "sync_failure",
        _ => "unknown",
    }
}

/// Distribution IDs for QPrep.
pub mod dist_id {
    /// Uniform distribution: equal probability over all basis states.
    pub const UNIFORM: u8 = 0;
    /// Zero state: delta distribution at |0>.
    pub const ZERO: u8 = 1;
    /// Bell state: correlated pair distribution.
    pub const BELL: u8 = 2;
    /// GHZ state: multi-register correlation.
    pub const GHZ: u8 = 3;
}

/// Kernel IDs for QKernel.
pub mod kernel_id {
    /// Initialization kernel.
    pub const INIT: u8 = 0;
    /// Entanglement kernel.
    pub const ENTANGLE: u8 = 1;
    /// Quantum Fourier Transform.
    pub const FOURIER: u8 = 2;
    /// Grover diffusion operator.
    pub const DIFFUSE: u8 = 3;
    /// Grover iteration (oracle + diffusion).
    pub const GROVER_ITER: u8 = 4;
}

/// PSW flag IDs for HCExec.
pub mod flag_id {
    /// Zero flag.
    pub const ZF: u8 = 0;
    /// Negative flag.
    pub const NF: u8 = 1;
    /// Overflow flag.
    pub const OF: u8 = 2;
    /// Predicate flag.
    pub const PF: u8 = 3;
    /// Quantum active flag.
    pub const QF: u8 = 4;
    /// Superposition flag.
    pub const SF: u8 = 5;
    /// Entanglement flag.
    pub const EF: u8 = 6;
    /// Hybrid mode flag.
    pub const HF: u8 = 7;
}

/// Reduction function IDs for HReduce.
pub mod reduce_fn {
    // Float -> Int reductions
    /// Round to nearest integer.
    pub const ROUND: u8 = 0;
    /// Floor (round toward negative infinity).
    pub const FLOOR: u8 = 1;
    /// Ceiling (round toward positive infinity).
    pub const CEIL: u8 = 2;
    /// Truncate (round toward zero).
    pub const TRUNC: u8 = 3;
    /// Absolute value (as integer).
    pub const ABS: u8 = 4;
    /// Negate (as integer).
    pub const NEGATE: u8 = 5;

    // Complex -> Float reductions
    /// Complex magnitude: sqrt(re^2 + im^2).
    pub const MAGNITUDE: u8 = 6;
    /// Complex phase: atan2(im, re).
    pub const PHASE: u8 = 7;
    /// Real part of complex.
    pub const REAL: u8 = 8;
    /// Imaginary part of complex.
    pub const IMAG: u8 = 9;

    // Distribution reductions
    /// Mean of distribution.
    pub const MEAN: u8 = 10;
    /// Mode of distribution (most probable value).
    pub const MODE: u8 = 11;
    /// Argmax of distribution (index of most probable value).
    pub const ARGMAX: u8 = 12;
    /// Variance of distribution.
    pub const VARIANCE: u8 = 13;
}

/// Helper: name string for a distribution ID (for display/debug).
pub fn dist_name(id: u8) -> &'static str {
    match id {
        dist_id::UNIFORM => "uniform",
        dist_id::ZERO => "zero",
        dist_id::BELL => "bell",
        dist_id::GHZ => "ghz",
        _ => "unknown",
    }
}

/// Helper: name string for a kernel ID (for display/debug).
pub fn kernel_name(id: u8) -> &'static str {
    match id {
        kernel_id::INIT => "init",
        kernel_id::ENTANGLE => "entangle",
        kernel_id::FOURIER => "fourier",
        kernel_id::DIFFUSE => "diffuse",
        kernel_id::GROVER_ITER => "grover_iter",
        _ => "unknown",
    }
}

/// Helper: name string for a flag ID (for display/debug).
pub fn flag_name(id: u8) -> &'static str {
    match id {
        flag_id::ZF => "ZF",
        flag_id::NF => "NF",
        flag_id::OF => "OF",
        flag_id::PF => "PF",
        flag_id::QF => "QF",
        flag_id::SF => "SF",
        flag_id::EF => "EF",
        flag_id::HF => "HF",
        _ => "unknown",
    }
}

/// Helper: name string for a reduction function ID (for display/debug).
pub fn reduce_fn_name(id: u8) -> &'static str {
    match id {
        reduce_fn::ROUND => "round",
        reduce_fn::FLOOR => "floor",
        reduce_fn::CEIL => "ceil",
        reduce_fn::TRUNC => "trunc",
        reduce_fn::ABS => "abs",
        reduce_fn::NEGATE => "negate",
        reduce_fn::MAGNITUDE => "magnitude",
        reduce_fn::PHASE => "phase",
        reduce_fn::REAL => "real",
        reduce_fn::IMAG => "imag",
        reduce_fn::MEAN => "mean",
        reduce_fn::MODE => "mode",
        reduce_fn::ARGMAX => "argmax",
        reduce_fn::VARIANCE => "variance",
        _ => "unknown",
    }
}
