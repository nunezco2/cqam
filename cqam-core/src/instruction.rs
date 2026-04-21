//! ISA instruction set for the CQAM virtual machine.
//!
//! Defines the complete `Instruction` enum and the named-constant sub-modules
//! for distribution IDs, kernel IDs, flag IDs, trap IDs, and reduction functions.

/// A single operation in the CQAM ISA.
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
/// Jump targets are label names (`String`) resolved to numeric addresses during
/// binary encoding by `cqam_core::opcode::encode`.
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

    /// Integer increment: R[dst] = R[src] + 1
    /// Single-operand form: IINC Rd  (src = dst, in-place)
    /// Two-operand form:    IINC Rd, Rs
    IInc { dst: u8, src: u8 },

    /// Integer decrement: R[dst] = R[src] - 1
    /// Single-operand form: IDEC Rd  (src = dst, in-place)
    /// Two-operand form:    IDEC Rd, Rs
    IDec { dst: u8, src: u8 },

    /// Integer register copy: R[dst] = R[src]
    /// Updates ZF/SF from the copied value.
    IMov { dst: u8, src: u8 },

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

    /// Float register copy: F[dst] = F[src]
    /// Does NOT update PSW (no arithmetic).
    FMov { dst: u8, src: u8 },

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

    /// Complex register copy: Z[dst] = Z[src]
    /// Does NOT update PSW (no arithmetic).
    ZMov { dst: u8, src: u8 },

    /// Load complex immediate: Z[dst] = (imm_re as f64, imm_im as f64)
    ZLdi { dst: u8, imm_re: i8, imm_im: i8 },

    /// Load complex from two consecutive CMEM cells.
    /// Z[dst] = (f64::from_bits(CMEM[addr] as u64), f64::from_bits(CMEM[addr+1] as u64))
    ZLdm { dst: u8, addr: u16 },

    /// Store complex to two consecutive CMEM cells.
    /// CMEM[addr]   = Z[src].0.to_bits() as i64
    /// CMEM[addr+1] = Z[src].1.to_bits() as i64
    ZStr { src: u8, addr: u16 },

    // -- Register-indirect memory ---------------------------------------------

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

    // -- Configuration query --------------------------------------------------

    /// Load the configured qubit count into an integer register.
    /// R[dst] = ctx.config.default_qubits as i64
    ///
    /// Traps (Arithmetic) if the value is 0 or exceeds the architectural
    /// maximum for the active backend.
    IQCfg { dst: u8 },

    /// Load the configured thread count into an integer register.
    /// R[dst] = ctx.thread_count as i64
    /// Mirrors IQCFG for qubits.
    ICCfg { dst: u8 },

    /// Load the current thread index into an integer register.
    /// R[dst] = ctx.thread_id as i64
    /// Returns 0 when not inside an HFORK/HMERGE block.
    ITid { dst: u8 },

    /// Environment call: invoke a built-in host procedure.
    /// Does not push the call stack (executes synchronously, falls through to PC+1).
    /// Arguments are passed via registers per the calling convention.
    Ecall { proc_id: ProcId },

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
    /// dist: 0=zero, 1=uniform, 2=bell, 3=ghz (see dist_id module)
    QPrep { dst: u8, dist: DistId },

    /// Apply quantum kernel transformation.
    /// Q[dst] = kernel(Q[src], R[ctx0], R[ctx1])
    /// kernel: kernel ID (see kernel_id module)
    /// ctx0, ctx1: integer register indices providing classical context
    QKernel { dst: u8, src: u8, kernel: KernelId, ctx0: u8, ctx1: u8 },

    /// Destructively observe (measure) a quantum register.
    /// H[dst_h] = measure(Q[src_q])
    /// The quantum register Q[src_q] is consumed (set to None).
    /// mode: 0=DIST (full distribution), 1=PROB (single probability), 3=SAMPLE (projective)
    /// ctx0, ctx1: integer register indices providing classical context for PROB mode
    QObserve { dst_h: u8, src_q: u8, mode: ObserveMode, ctx0: u8, ctx1: u8 },

    /// Load quantum distribution from QMEM into quantum register.
    /// Q[dst_q] = QMEM[addr]
    QLoad { dst_q: u8, addr: u8 },

    /// Store quantum register into QMEM slot.
    /// QMEM[addr] = Q[src_q]
    QStore { src_q: u8, addr: u8 },

    /// Apply quantum kernel with float context parameters from F-file.
    /// Q[dst] = kernel(Q[src], F[fctx0], F[fctx1])
    /// kernel: kernel ID (see kernel_id module)
    /// fctx0, fctx1: float register indices providing classical context
    QKernelF { dst: u8, src: u8, kernel: KernelId, fctx0: u8, fctx1: u8 },

    /// Apply quantum kernel with complex context parameters from Z-file.
    /// Q[dst] = kernel(Q[src], Z[zctx0], Z[zctx1])
    /// kernel: kernel ID (see kernel_id module)
    /// zctx0, zctx1: complex register indices providing classical context
    QKernelZ { dst: u8, src: u8, kernel: KernelId, zctx0: u8, zctx1: u8 },

    /// Prepare quantum register with distribution ID from integer register.
    /// Q[dst] = new_qdist(R[dist_reg] as u8)
    /// dist_reg: integer register index whose value is interpreted as a dist_id.
    /// At runtime, the value is cast to u8 and dispatched through the same
    /// dist_id table as QPrep (ZERO=0, UNIFORM=1, BELL=2, GHZ=3).
    QPrepR { dst: u8, dist_reg: u8 },

    /// Encode classical register values as quantum state amplitudes.
    /// Q[dst] = from_statevector(read_regs(file_sel, src_base, count))
    /// src_base: first register index in the selected file (R, F, or Z)
    /// count: number of consecutive registers to read (must be power of 2)
    /// file_sel: register file selector (0=R, 1=F, 2=Z; see file_sel module)
    QEncode { dst: u8, src_base: u8, count: u8, file_sel: FileSel },

    /// Apply Hadamard gate to each qubit selected by a classical bitmask.
    ///
    /// For each bit i of R[mask_reg] that is 1, apply H to qubit i of Q[src].
    /// Store the result in Q[dst]. Bits beyond num_qubits are ignored.
    ///
    /// This is a register-level operation: the mask provides classical control
    /// over which qubits enter superposition.
    QHadM { dst: u8, src: u8, mask_reg: u8 },

    /// Apply Pauli-X (bit flip) to each qubit selected by a classical bitmask.
    ///
    /// For each bit i of R[mask_reg] that is 1, apply X to qubit i of Q[src].
    /// Store the result in Q[dst]. Bits beyond num_qubits are ignored.
    QFlip { dst: u8, src: u8, mask_reg: u8 },

    /// Apply Pauli-Z (phase flip) to each qubit selected by a classical bitmask.
    ///
    /// For each bit i of R[mask_reg] that is 1, apply Z to qubit i of Q[src].
    /// Store the result in Q[dst]. Bits beyond num_qubits are ignored.
    QPhase { dst: u8, src: u8, mask_reg: u8 },

    /// Apply two-qubit CNOT gate to specific qubits within a quantum register.
    ///
    /// Q[dst] = CNOT(Q[src], ctrl=R[ctrl_qubit_reg], tgt=R[tgt_qubit_reg])
    /// The control and target qubit indices are read from integer registers.
    /// Traps if ctrl == tgt or either index >= num_qubits.
    QCnot { dst: u8, src: u8, ctrl_qubit_reg: u8, tgt_qubit_reg: u8 },

    /// Apply parameterized single-qubit rotation to a specific qubit.
    ///
    /// Q[dst] = R_axis(F[angle_freg])(Q[src], qubit=R[qubit_reg])
    /// axis: 0=X, 1=Y, 2=Z (see rot_axis module)
    /// The rotation angle theta is read from F[angle_freg] in radians.
    /// The target qubit index is read from R[qubit_reg].
    QRot { dst: u8, src: u8, qubit_reg: u8, axis: RotAxis, angle_freg: u8 },

    /// Measure a single qubit within a quantum register.
    ///
    /// R[dst_r] = measure_qubit(Q[src_q], qubit=R[qubit_reg])
    /// Stores the measurement outcome (0 or 1) in integer register R[dst_r].
    /// The quantum register Q[src_q] is updated to the post-measurement state
    /// (projected and renormalized, NOT consumed).
    QMeas { dst_r: u8, src_q: u8, qubit_reg: u8 },

    /// Tensor product of two quantum registers.
    ///
    /// Q[dst] = Q[src0] tensor Q[src1]
    /// Both source registers are consumed (set to None).
    /// The resulting register has num_qubits(src0) + num_qubits(src1) qubits.
    /// Traps if the combined qubit count exceeds MAX_QUBITS.
    QTensor { dst: u8, src0: u8, src1: u8 },

    /// Apply a custom unitary matrix read from classical memory.
    ///
    /// Q[dst] = U * Q[src] * U^dagger
    /// where U is read from CMEM[R[base_addr_reg]..] as consecutive (re, im) pairs.
    /// R[dim_reg] specifies the matrix dimension (must equal Q[src].dimension()).
    /// Total cells consumed: 2 * dim * dim.
    QCustom { dst: u8, src: u8, base_addr_reg: u8, dim_reg: u8 },

    /// Apply two-qubit Controlled-Z gate to specific qubits within a quantum register.
    ///
    /// Q[dst] = CZ(Q[src], ctrl=R[ctrl_qubit_reg], tgt=R[tgt_qubit_reg])
    QCz { dst: u8, src: u8, ctrl_qubit_reg: u8, tgt_qubit_reg: u8 },

    /// Apply two-qubit SWAP gate to specific qubits within a quantum register.
    ///
    /// Q[dst] = SWAP(Q[src], qubit_a=R[qubit_a_reg], qubit_b=R[qubit_b_reg])
    QSwap { dst: u8, src: u8, qubit_a_reg: u8, qubit_b_reg: u8 },

    /// Prepare a mixed quantum state from weighted statevectors in CMEM.
    ///
    /// Q[dst] = sum_i w_i * |psi_i><psi_i|
    /// R[base_addr_reg] = base address in CMEM for state data.
    /// R[count_reg] = number of statevector/weight entries.
    ///
    /// CMEM layout per entry: [weight_f64][dim_u64][re_0][im_0][re_1][im_1]...
    /// where weight is f64 bits, dim is the statevector dimension (power of 2),
    /// and (re, im) pairs are f64 bits for each amplitude.
    QMixed { dst: u8, base_addr_reg: u8, count_reg: u8 },

    /// Prepare quantum register with a specified number of qubits.
    ///
    /// Q[dst] = new_qdist(dist, num_qubits=R[qubit_count_reg])
    /// dist: distribution ID (0=zero, 1=uniform, 2=bell, 3=ghz)
    /// The qubit count is read from R[qubit_count_reg] at runtime.
    QPrepN { dst: u8, dist: DistId, qubit_count_reg: u8 },

    /// Float sine: F[dst] = sin(F[src])
    FSin { dst: u8, src: u8 },

    /// Float cosine: F[dst] = cos(F[src])
    FCos { dst: u8, src: u8 },

    /// Float atan2: F[dst] = atan2(F[lhs], F[rhs])
    /// lhs = y, rhs = x (following standard math convention)
    FAtan2 { dst: u8, lhs: u8, rhs: u8 },

    /// Float square root: F[dst] = sqrt(F[src])
    /// Traps if F[src] < 0.
    FSqrt { dst: u8, src: u8 },

    /// Compute partial trace over subsystem B.
    ///
    /// Q[dst] = Tr_B(Q[src]) where subsystem A has R[num_qubits_a_reg] qubits.
    /// The resulting register has fewer qubits than Q[src].
    /// Q[src] is NOT consumed (non-destructive).
    QPtrace { dst: u8, src: u8, num_qubits_a_reg: u8 },

    /// Reset a single qubit to |0> within a quantum register.
    ///
    /// Q[dst] = reset_qubit(Q[src], qubit=R[qubit_reg])
    /// Semantics: measure the target qubit; if outcome is 1, apply X to flip it.
    /// The result is a state where the target qubit is guaranteed to be |0>.
    /// Implemented via `measure_qubit` followed by a conditional X gate.
    QReset { dst: u8, src: u8, qubit_reg: u8 },

    /// Prepare product state from Z-register pairs (register-direct).
    ///
    /// For each qubit i (0..count), reads alpha from Z[z_start + 2*i]
    /// and beta from Z[z_start + 2*i + 1]. Each qubit is independently
    /// prepared in state alpha_i|0> + beta_i|1> via a U3 gate.
    /// Precondition: Qdst must hold a valid handle from QPREP Qdst, ZERO.
    QPreps { dst: u8, z_start: u8, count: u8 },

    /// Prepare product state from CMEM-indirect amplitudes.
    ///
    /// Reads R[r_count] as qubit count, R[r_base] as CMEM base address.
    /// For each qubit i (0..count), reads 4 CMEM cells:
    ///   CMEM[base + 4*i + 0..3] = re(alpha), im(alpha), re(beta), im(beta)
    /// Each qubit is independently prepared via U3.
    /// Precondition: Qdst must hold a valid handle from QPREP Qdst, ZERO.
    QPrepsm { dst: u8, r_base: u8, r_count: u8 },

    /// Swap quantum register handles between Qa and Qb.
    ///
    /// Pure handle-level swap — no quantum gates are emitted. The quantum states
    /// do not move; only the pointers (handles) in the Q-register file are
    /// exchanged. Both Qa and Qb must hold valid (non-None) handles.
    /// PSW is untouched.
    QXch { qa: u8, qb: u8 },

    // -- Hybrid (H-file: HybridValue x 8) ------------------------------------

    /// Fork hybrid execution into parallel threads. Sets PSW fork flags.
    HFork,

    /// Merge hybrid execution branches by joining all forked threads.
    HMerge,

    /// Hybrid Atomic Section Start. Full barrier: all threads must arrive.
    /// One thread is elected to execute the atomic section.
    /// No quantum operations are allowed until HATME.
    HAtmS,

    /// Hybrid Atomic Section End. Commits shared memory, resumes all threads.
    HAtmE,

    /// Conditional execution based on PSW flag.
    /// if PSW.flag[flag] then PC = address_of(target)
    /// flag: flag ID (see flag_id module)
    JmpF { flag: FlagId, target: String },

    /// Reduce hybrid value to classical value.
    /// The output register file depends on the reduction function:
    /// - round/floor/ceil/trunc/abs/negate (0-5): H[src] -> R[dst] (int)
    /// - magnitude/phase/real/imag (6-9): H[src] -> F[dst] (float)
    /// - mean/mode/argmax/variance (10-13): H[src] -> F[dst] or R[dst]
    ///   func: reduction function ID (see reduce_fn module)
    HReduce { src: u8, dst: u8, func: ReduceFn },

    // -- Interrupt handling ---------------------------------------------------

    /// Return from interrupt handler.
    /// Pop saved PC from call stack, clear maskable trap flags, resume.
    Reti,

    /// Set interrupt vector: register a handler address for a trap ID.
    /// trap_id: 0=Arithmetic, 1=QuantumError, 2=SyncFailure
    /// target: label name (resolved to address during encoding)
    SetIV { trap_id: TrapId, target: String },
}

// =============================================================================
// Type-safe ID enums
// =============================================================================

use std::fmt;
use crate::error::CqamError;

/// Trap IDs for SetIV instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TrapId {
    /// Arithmetic fault (division by zero, overflow).
    Arithmetic = 0,
    /// Quantum fidelity dropped below threshold.
    QuantumError = 1,
    /// Hybrid branch synchronization failure.
    SyncFailure = 2,
}

impl TryFrom<u8> for TrapId {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(TrapId::Arithmetic),
            1 => Ok(TrapId::QuantumError),
            2 => Ok(TrapId::SyncFailure),
            _ => Err(CqamError::InvalidId { domain: "TrapId", value: v }),
        }
    }
}

impl From<TrapId> for u8 {
    fn from(v: TrapId) -> u8 { v as u8 }
}

impl fmt::Display for TrapId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl TrapId {
    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            TrapId::Arithmetic => "arithmetic",
            TrapId::QuantumError => "quantum_error",
            TrapId::SyncFailure => "sync_failure",
        }
    }

    /// Parse from name string (case-insensitive) or numeric.
    pub fn from_token(token: &str) -> Option<Self> {
        match token.to_lowercase().as_str() {
            "arithmetic" => Some(TrapId::Arithmetic),
            "quantum_error" => Some(TrapId::QuantumError),
            "sync_failure" => Some(TrapId::SyncFailure),
            _ => token.parse::<u8>().ok().and_then(|v| Self::try_from(v).ok()),
        }
    }
}

/// Distribution IDs for QPrep / QPrepN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DistId {
    /// Zero state: delta distribution at |0>.
    Zero = 0,
    /// Uniform distribution: equal probability over all basis states.
    Uniform = 1,
    /// Bell state: correlated pair distribution.
    Bell = 2,
    /// GHZ state: multi-register correlation.
    Ghz = 3,
}

impl TryFrom<u8> for DistId {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(DistId::Zero),
            1 => Ok(DistId::Uniform),
            2 => Ok(DistId::Bell),
            3 => Ok(DistId::Ghz),
            _ => Err(CqamError::InvalidId { domain: "DistId", value: v }),
        }
    }
}

impl From<DistId> for u8 {
    fn from(v: DistId) -> u8 { v as u8 }
}

impl fmt::Display for DistId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl DistId {
    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            DistId::Uniform => "uniform",
            DistId::Zero => "zero",
            DistId::Bell => "bell",
            DistId::Ghz => "ghz",
        }
    }

    /// Parse from token: accepts "UNIFORM", "ZERO", "BELL", "GHZ" or numeric.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "UNIFORM" | "uniform" => Some(DistId::Uniform),
            "ZERO" | "zero" => Some(DistId::Zero),
            "BELL" | "bell" => Some(DistId::Bell),
            "GHZ" | "ghz" => Some(DistId::Ghz),
            _ => token.parse::<u8>().ok().and_then(|v| Self::try_from(v).ok()),
        }
    }
}

/// Kernel IDs for QKernel / QKernelF / QKernelZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum KernelId {
    /// Initialization kernel.
    Init = 0,
    /// Entanglement kernel.
    Entangle = 1,
    /// Quantum Fourier Transform.
    Fourier = 2,
    /// Grover diffusion operator.
    Diffuse = 3,
    /// Grover iteration (oracle + diffusion).
    GroverIter = 4,
    /// Diagonal rotation kernel.
    Rotate = 5,
    /// Phase shift kernel.
    PhaseShift = 6,
    /// Inverse Quantum Fourier Transform.
    FourierInv = 7,
    /// Controlled-U kernel.
    ControlledU = 8,
    /// Diagonal unitary kernel.
    DiagonalUnitary = 9,
    /// Permutation kernel.
    Permutation = 10,
}

impl TryFrom<u8> for KernelId {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(KernelId::Init),
            1 => Ok(KernelId::Entangle),
            2 => Ok(KernelId::Fourier),
            3 => Ok(KernelId::Diffuse),
            4 => Ok(KernelId::GroverIter),
            5 => Ok(KernelId::Rotate),
            6 => Ok(KernelId::PhaseShift),
            7 => Ok(KernelId::FourierInv),
            8 => Ok(KernelId::ControlledU),
            9 => Ok(KernelId::DiagonalUnitary),
            10 => Ok(KernelId::Permutation),
            _ => Err(CqamError::InvalidId { domain: "KernelId", value: v }),
        }
    }
}

impl From<KernelId> for u8 {
    fn from(v: KernelId) -> u8 { v as u8 }
}

impl fmt::Display for KernelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl KernelId {
    /// Four-letter assembly mnemonic.
    pub fn mnemonic(self) -> &'static str {
        match self {
            KernelId::Init => "UNIT",
            KernelId::Entangle => "ENTG",
            KernelId::Fourier => "QFFT",
            KernelId::Diffuse => "DIFF",
            KernelId::GroverIter => "GROV",
            KernelId::Rotate => "DROT",
            KernelId::PhaseShift => "PHSH",
            KernelId::FourierInv => "QIFT",
            KernelId::ControlledU => "CTLU",
            KernelId::DiagonalUnitary => "DIAG",
            KernelId::Permutation => "PERM",
        }
    }

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            KernelId::Init => "init",
            KernelId::Entangle => "entangle",
            KernelId::Fourier => "fourier",
            KernelId::Diffuse => "diffuse",
            KernelId::GroverIter => "grover_iter",
            KernelId::Rotate => "rotate",
            KernelId::PhaseShift => "phase_shift",
            KernelId::FourierInv => "fourier_inv",
            KernelId::ControlledU => "controlled_u",
            KernelId::DiagonalUnitary => "diagonal_unitary",
            KernelId::Permutation => "permutation",
        }
    }

    /// Parse from mnemonic string.
    pub fn from_mnemonic(name: &str) -> Option<Self> {
        match name {
            "UNIT" => Some(KernelId::Init),
            "ENTG" => Some(KernelId::Entangle),
            "QFFT" => Some(KernelId::Fourier),
            "DIFF" => Some(KernelId::Diffuse),
            "GROV" => Some(KernelId::GroverIter),
            "DROT" => Some(KernelId::Rotate),
            "PHSH" => Some(KernelId::PhaseShift),
            "QIFT" => Some(KernelId::FourierInv),
            "CTLU" => Some(KernelId::ControlledU),
            "DIAG" => Some(KernelId::DiagonalUnitary),
            "PERM" => Some(KernelId::Permutation),
            _ => None,
        }
    }
}

/// PSW flag IDs for JmpF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FlagId {
    /// Zero flag.
    Zf = 0,
    /// Negative flag.
    Nf = 1,
    /// Overflow flag.
    Of = 2,
    /// Predicate flag.
    Pf = 3,
    /// Quantum active flag.
    Qf = 4,
    /// Superposition flag.
    Sf = 5,
    /// Entanglement flag.
    Ef = 6,
    /// Hybrid mode flag.
    Hf = 7,
    /// Decoherence flag (sticky).
    Df = 8,
    /// Collapsed flag (transient).
    Cf = 9,
    /// Forked flag.
    Fk = 10,
    /// Merged flag.
    Mg = 11,
    /// Interference flag.
    If = 12,
    /// Atomic section flag.
    Af = 13,
}

impl TryFrom<u8> for FlagId {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(FlagId::Zf),
            1 => Ok(FlagId::Nf),
            2 => Ok(FlagId::Of),
            3 => Ok(FlagId::Pf),
            4 => Ok(FlagId::Qf),
            5 => Ok(FlagId::Sf),
            6 => Ok(FlagId::Ef),
            7 => Ok(FlagId::Hf),
            8 => Ok(FlagId::Df),
            9 => Ok(FlagId::Cf),
            10 => Ok(FlagId::Fk),
            11 => Ok(FlagId::Mg),
            12 => Ok(FlagId::If),
            13 => Ok(FlagId::Af),
            _ => Err(CqamError::InvalidId { domain: "FlagId", value: v }),
        }
    }
}

impl From<FlagId> for u8 {
    fn from(v: FlagId) -> u8 { v as u8 }
}

impl fmt::Display for FlagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mnemonic())
    }
}

impl FlagId {
    /// Two-letter mnemonic.
    pub fn mnemonic(self) -> &'static str {
        match self {
            FlagId::Zf => "ZF",
            FlagId::Nf => "NF",
            FlagId::Of => "OF",
            FlagId::Pf => "PF",
            FlagId::Qf => "QF",
            FlagId::Sf => "SF",
            FlagId::Ef => "EF",
            FlagId::Hf => "HF",
            FlagId::Df => "DF",
            FlagId::Cf => "CF",
            FlagId::Fk => "FK",
            FlagId::Mg => "MG",
            FlagId::If => "IF",
            FlagId::Af => "AF",
        }
    }

    /// Parse from mnemonic string.
    pub fn from_mnemonic(name: &str) -> Option<Self> {
        match name {
            "ZF" => Some(FlagId::Zf),
            "NF" => Some(FlagId::Nf),
            "OF" => Some(FlagId::Of),
            "PF" => Some(FlagId::Pf),
            "QF" => Some(FlagId::Qf),
            "SF" => Some(FlagId::Sf),
            "EF" => Some(FlagId::Ef),
            "HF" => Some(FlagId::Hf),
            "DF" => Some(FlagId::Df),
            "CF" => Some(FlagId::Cf),
            "FK" => Some(FlagId::Fk),
            "MG" => Some(FlagId::Mg),
            "IF" => Some(FlagId::If),
            "AF" => Some(FlagId::Af),
            _ => None,
        }
    }
}

/// Observation mode IDs for QObserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObserveMode {
    /// Full diagonal distribution.
    Dist = 0,
    /// Probability of basis state at index R[ctx0].
    Prob = 1,
    // 2 is reserved (was AMP — removed: density matrix element access is not
    // physically realizable on hardware).
    /// Projective measurement sample.
    Sample = 3,
}

impl TryFrom<u8> for ObserveMode {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(ObserveMode::Dist),
            1 => Ok(ObserveMode::Prob),
            2 => Err(CqamError::InvalidId { domain: "ObserveMode", value: v }),  // reserved (was AMP, removed)
            3 => Ok(ObserveMode::Sample),
            _ => Err(CqamError::InvalidId { domain: "ObserveMode", value: v }),
        }
    }
}

impl From<ObserveMode> for u8 {
    fn from(v: ObserveMode) -> u8 { v as u8 }
}

impl fmt::Display for ObserveMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl ObserveMode {
    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            ObserveMode::Dist => "dist",
            ObserveMode::Prob => "prob",
            ObserveMode::Sample => "sample",
        }
    }
}

/// Which output register file a reduction function targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceOutput {
    /// Result goes to integer register file (R).
    IntReg,
    /// Result goes to float register file (F).
    FloatReg,
    /// Result goes to complex register file (Z).
    ComplexReg,
}

/// Reduction function IDs for HReduce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ReduceFn {
    Round = 0,
    Floor = 1,
    Ceil = 2,
    Trunc = 3,
    Abs = 4,
    Negate = 5,
    Magnitude = 6,
    Phase = 7,
    Real = 8,
    Imag = 9,
    Mean = 10,
    Mode = 11,
    Argmax = 12,
    Variance = 13,
    ConjZ = 14,
    NegateZ = 15,
    Expect = 16,
}

impl TryFrom<u8> for ReduceFn {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(ReduceFn::Round),
            1 => Ok(ReduceFn::Floor),
            2 => Ok(ReduceFn::Ceil),
            3 => Ok(ReduceFn::Trunc),
            4 => Ok(ReduceFn::Abs),
            5 => Ok(ReduceFn::Negate),
            6 => Ok(ReduceFn::Magnitude),
            7 => Ok(ReduceFn::Phase),
            8 => Ok(ReduceFn::Real),
            9 => Ok(ReduceFn::Imag),
            10 => Ok(ReduceFn::Mean),
            11 => Ok(ReduceFn::Mode),
            12 => Ok(ReduceFn::Argmax),
            13 => Ok(ReduceFn::Variance),
            14 => Ok(ReduceFn::ConjZ),
            15 => Ok(ReduceFn::NegateZ),
            16 => Ok(ReduceFn::Expect),
            _ => Err(CqamError::InvalidId { domain: "ReduceFn", value: v }),
        }
    }
}

impl From<ReduceFn> for u8 {
    fn from(v: ReduceFn) -> u8 { v as u8 }
}

impl fmt::Display for ReduceFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl ReduceFn {
    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            ReduceFn::Round => "round",
            ReduceFn::Floor => "floor",
            ReduceFn::Ceil => "ceil",
            ReduceFn::Trunc => "trunc",
            ReduceFn::Abs => "abs",
            ReduceFn::Negate => "negate",
            ReduceFn::Magnitude => "magnitude",
            ReduceFn::Phase => "phase",
            ReduceFn::Real => "real",
            ReduceFn::Imag => "imag",
            ReduceFn::Mean => "mean",
            ReduceFn::Mode => "mode",
            ReduceFn::Argmax => "argmax",
            ReduceFn::Variance => "variance",
            ReduceFn::ConjZ => "conj_z",
            ReduceFn::NegateZ => "negate_z",
            ReduceFn::Expect => "expect",
        }
    }

    /// Five-letter assembly mnemonic.
    pub fn mnemonic(self) -> &'static str {
        match self {
            ReduceFn::Round => "ROUND",
            ReduceFn::Floor => "FLOOR",
            ReduceFn::Ceil => "CEILI",
            ReduceFn::Trunc => "TRUNC",
            ReduceFn::Abs => "ABSOL",
            ReduceFn::Negate => "NEGAT",
            ReduceFn::Magnitude => "MAGNI",
            ReduceFn::Phase => "PHASE",
            ReduceFn::Real => "REALP",
            ReduceFn::Imag => "IMAGP",
            ReduceFn::Mean => "MEANT",
            ReduceFn::Mode => "MODEV",
            ReduceFn::Argmax => "ARGMX",
            ReduceFn::Variance => "VARNC",
            ReduceFn::ConjZ => "CONJZ",
            ReduceFn::NegateZ => "NEGTZ",
            ReduceFn::Expect => "EXPCT",
        }
    }

    /// Parse from mnemonic string.
    pub fn from_mnemonic(name: &str) -> Option<Self> {
        match name {
            "ROUND" => Some(ReduceFn::Round),
            "FLOOR" => Some(ReduceFn::Floor),
            "CEILI" => Some(ReduceFn::Ceil),
            "TRUNC" => Some(ReduceFn::Trunc),
            "ABSOL" => Some(ReduceFn::Abs),
            "NEGAT" => Some(ReduceFn::Negate),
            "MAGNI" => Some(ReduceFn::Magnitude),
            "PHASE" => Some(ReduceFn::Phase),
            "REALP" => Some(ReduceFn::Real),
            "IMAGP" => Some(ReduceFn::Imag),
            "MEANT" => Some(ReduceFn::Mean),
            "MODEV" => Some(ReduceFn::Mode),
            "ARGMX" => Some(ReduceFn::Argmax),
            "VARNC" => Some(ReduceFn::Variance),
            "CONJZ" => Some(ReduceFn::ConjZ),
            "NEGTZ" => Some(ReduceFn::NegateZ),
            "EXPCT" => Some(ReduceFn::Expect),
            _ => None,
        }
    }

    /// Which register file the output goes to.
    pub fn output_file(self) -> ReduceOutput {
        match self {
            ReduceFn::Round | ReduceFn::Floor | ReduceFn::Ceil
            | ReduceFn::Trunc | ReduceFn::Abs | ReduceFn::Negate
            | ReduceFn::Mode | ReduceFn::Argmax => ReduceOutput::IntReg,
            ReduceFn::Magnitude | ReduceFn::Phase | ReduceFn::Real
            | ReduceFn::Imag | ReduceFn::Mean | ReduceFn::Variance
            | ReduceFn::Expect => ReduceOutput::FloatReg,
            ReduceFn::ConjZ | ReduceFn::NegateZ => ReduceOutput::ComplexReg,
        }
    }
}

/// Rotation axis for QROT instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RotAxis {
    /// Rotation about X axis.
    X = 0,
    /// Rotation about Y axis.
    Y = 1,
    /// Rotation about Z axis.
    Z = 2,
}

impl TryFrom<u8> for RotAxis {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(RotAxis::X),
            1 => Ok(RotAxis::Y),
            2 => Ok(RotAxis::Z),
            _ => Err(CqamError::InvalidId { domain: "RotAxis", value: v }),
        }
    }
}

impl From<RotAxis> for u8 {
    fn from(v: RotAxis) -> u8 { v as u8 }
}

impl fmt::Display for RotAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl RotAxis {
    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            RotAxis::X => "X",
            RotAxis::Y => "Y",
            RotAxis::Z => "Z",
        }
    }
}

/// Register file selector for QEncode instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FileSel {
    /// Integer register file.
    RFile = 0,
    /// Float register file.
    FFile = 1,
    /// Complex register file.
    ZFile = 2,
}

impl TryFrom<u8> for FileSel {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(FileSel::RFile),
            1 => Ok(FileSel::FFile),
            2 => Ok(FileSel::ZFile),
            _ => Err(CqamError::InvalidId { domain: "FileSel", value: v }),
        }
    }
}

impl From<FileSel> for u8 {
    fn from(v: FileSel) -> u8 { v as u8 }
}

impl fmt::Display for FileSel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FileSel {
    /// Short name.
    pub fn name(self) -> &'static str {
        match self {
            FileSel::RFile => "R",
            FileSel::FFile => "F",
            FileSel::ZFile => "Z",
        }
    }
}

/// Built-in procedure IDs for ECALL instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProcId {
    /// Print R[0] as a signed decimal integer followed by newline.
    PrintInt = 0,
    /// Print F[0] as a floating-point number followed by newline.
    PrintFloat = 1,
    /// Print formatted string from CMEM.
    PrintStr = 2,
    /// Print R[0] as a single ASCII character (no newline).
    PrintChar = 3,
    /// Debug dump: print all non-zero registers to stderr.
    DumpRegs = 4,
    /// Print H register histogram to stdout.
    PrintHist = 5,
    /// Print Z[R[0]] as `a + ib` or `a - ib` to stdout.
    PrintCmpx = 6,
}

impl TryFrom<u8> for ProcId {
    type Error = CqamError;
    fn try_from(v: u8) -> Result<Self, CqamError> {
        match v {
            0 => Ok(ProcId::PrintInt),
            1 => Ok(ProcId::PrintFloat),
            2 => Ok(ProcId::PrintStr),
            3 => Ok(ProcId::PrintChar),
            4 => Ok(ProcId::DumpRegs),
            5 => Ok(ProcId::PrintHist),
            6 => Ok(ProcId::PrintCmpx),
            _ => Err(CqamError::InvalidId { domain: "ProcId", value: v }),
        }
    }
}

impl From<ProcId> for u8 {
    fn from(v: ProcId) -> u8 { v as u8 }
}

impl fmt::Display for ProcId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl ProcId {
    /// Canonical name string.
    pub fn name(self) -> &'static str {
        match self {
            ProcId::PrintInt => "PRINT_INT",
            ProcId::PrintFloat => "PRINT_FLOAT",
            ProcId::PrintStr => "PRINT_STR",
            ProcId::PrintChar => "PRINT_CHAR",
            ProcId::DumpRegs => "DUMP_REGS",
            ProcId::PrintHist => "PRINT_HIST",
            ProcId::PrintCmpx => "PRINT_CMPX",
        }
    }

    /// Parse from name string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "PRINT_INT" => Some(ProcId::PrintInt),
            "PRINT_FLOAT" => Some(ProcId::PrintFloat),
            "PRINT_STR" => Some(ProcId::PrintStr),
            "PRINT_CHAR" => Some(ProcId::PrintChar),
            "DUMP_REGS" => Some(ProcId::DumpRegs),
            "PRINT_HIST" => Some(ProcId::PrintHist),
            "PRINT_CMPX" => Some(ProcId::PrintCmpx),
            _ => None,
        }
    }
}
