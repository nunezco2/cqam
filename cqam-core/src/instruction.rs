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
    /// mode: 0=DIST (full distribution), 1=PROB (single probability), 2=AMP (amplitude)
    /// ctx0, ctx1: integer register indices providing classical context for PROB/AMP modes
    QObserve { dst_h: u8, src_q: u8, mode: u8, ctx0: u8, ctx1: u8 },

    /// Load quantum distribution from QMEM into quantum register.
    /// Q[dst_q] = QMEM[addr]
    QLoad { dst_q: u8, addr: u8 },

    /// Store quantum register into QMEM slot.
    /// QMEM[addr] = Q[src_q]
    QStore { src_q: u8, addr: u8 },

    /// Non-destructively sample the probability distribution of a quantum register.
    /// H[dst_h] = diagonal_probabilities(Q[src_q])
    /// The quantum register Q[src_q] is NOT consumed (non-destructive read).
    /// mode: 0=DIST (full distribution), 1=PROB (single probability), 2=AMP (amplitude)
    /// ctx0, ctx1: integer register indices providing classical context for PROB/AMP modes
    QSample { dst_h: u8, src_q: u8, mode: u8, ctx0: u8, ctx1: u8 },

    /// Apply quantum kernel with float context parameters from F-file.
    /// Q[dst] = kernel(Q[src], F[fctx0], F[fctx1])
    /// kernel: kernel ID (see kernel_id module)
    /// fctx0, fctx1: float register indices providing classical context
    QKernelF { dst: u8, src: u8, kernel: u8, fctx0: u8, fctx1: u8 },

    /// Apply quantum kernel with complex context parameters from Z-file.
    /// Q[dst] = kernel(Q[src], Z[zctx0], Z[zctx1])
    /// kernel: kernel ID (see kernel_id module)
    /// zctx0, zctx1: complex register indices providing classical context
    QKernelZ { dst: u8, src: u8, kernel: u8, zctx0: u8, zctx1: u8 },

    /// Prepare quantum register with distribution ID from integer register.
    /// Q[dst] = new_qdist(R[dist_reg] as u8)
    /// dist_reg: integer register index whose value is interpreted as a dist_id.
    /// At runtime, the value is cast to u8 and dispatched through the same
    /// dist_id table as QPrep (UNIFORM=0, ZERO=1, BELL=2, GHZ=3).
    QPrepR { dst: u8, dist_reg: u8 },

    /// Encode classical register values as quantum state amplitudes.
    /// Q[dst] = from_statevector(read_regs(file_sel, src_base, count))
    /// src_base: first register index in the selected file (R, F, or Z)
    /// count: number of consecutive registers to read (must be power of 2)
    /// file_sel: register file selector (0=R, 1=F, 2=Z; see file_sel module)
    QEncode { dst: u8, src_base: u8, count: u8, file_sel: u8 },

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
    QRot { dst: u8, src: u8, qubit_reg: u8, axis: u8, angle_freg: u8 },

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
    /// dist: distribution ID (0=uniform, 1=zero, 2=bell, 3=ghz)
    /// The qubit count is read from R[qubit_count_reg] at runtime.
    QPrepN { dst: u8, dist: u8, qubit_count_reg: u8 },

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
    /// Depends on P0.3 measure_qubit and existing apply_single_qubit_gate.
    QReset { dst: u8, src: u8, qubit_reg: u8 },

    // -- Hybrid (H-file: HybridValue x 8) ------------------------------------

    /// Fork hybrid execution into parallel threads. Sets PSW fork flags.
    HFork,

    /// Merge hybrid execution branches by joining all forked threads.
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

    // -- Interrupt handling ---------------------------------------------------

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
    /// Diagonal rotation kernel: U[k][k] = exp(i * theta * k).
    pub const ROTATE: u8 = 5;
    /// Phase shift kernel: U[k][k] = exp(i * |z| * k).
    pub const PHASE_SHIFT: u8 = 6;
    /// Inverse Quantum Fourier Transform.
    pub const FOURIER_INV: u8 = 7;
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

/// Observation mode IDs for QObserve/QSample.
pub mod observe_mode {
    /// Full diagonal distribution (default, Phase 1 behavior).
    pub const DIST: u8 = 0;
    /// Probability of basis state at index R[ctx0].
    pub const PROB: u8 = 1;
    /// Amplitude dm.get(row, col) where row=R[ctx0], col=R[ctx1].
    pub const AMP: u8 = 2;
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

    // Complex -> Z-file reductions
    /// Conjugate: Z[dst] = (re, -im).
    pub const CONJ_Z: u8 = 14;
    /// Negate: Z[dst] = (-re, -im).
    pub const NEGATE_Z: u8 = 15;

    /// Expectation value: sum_k eigenvalue_k * p_k.
    /// Reads eigenvalues from CMEM[R[ctx]..R[ctx]+n] where n is distribution size.
    /// Output: F[dst] = expectation value.
    pub const EXPECT: u8 = 16;
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
        kernel_id::ROTATE => "rotate",
        kernel_id::PHASE_SHIFT => "phase_shift",
        kernel_id::FOURIER_INV => "fourier_inv",
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

/// Helper: name string for an observation mode (for display/debug).
pub fn observe_mode_name(mode: u8) -> &'static str {
    match mode {
        observe_mode::DIST => "dist",
        observe_mode::PROB => "prob",
        observe_mode::AMP => "amp",
        _ => "unknown",
    }
}

/// Register file selector constants for QEncode instruction.
///
/// Determines which classical register file provides the source amplitudes:
/// - R_FILE (0): Read from integer registers (R-file), values cast i64 -> f64
/// - F_FILE (1): Read from float registers (F-file), values used as real part
/// - Z_FILE (2): Read from complex registers (Z-file), values used directly
pub mod file_sel {
    /// Integer register file: R[src_base]..R[src_base+count-1] as i64.
    /// Each value is converted to a complex amplitude: (val as f64, 0.0).
    pub const R_FILE: u8 = 0;

    /// Float register file: F[src_base]..F[src_base+count-1] as f64.
    /// Each value is converted to a complex amplitude: (val, 0.0).
    pub const F_FILE: u8 = 1;

    /// Complex register file: Z[src_base]..Z[src_base+count-1] as (f64, f64).
    /// Values are used directly as complex amplitudes.
    pub const Z_FILE: u8 = 2;
}

/// Helper: name string for a file selector ID (for display/debug).
pub fn file_sel_name(id: u8) -> &'static str {
    match id {
        0 => "R",
        1 => "F",
        2 => "Z",
        _ => "unknown",
    }
}

/// Rotation axis constants for QROT instruction.
pub mod rot_axis {
    /// Rotation about X axis: Rx(theta).
    pub const X: u8 = 0;
    /// Rotation about Y axis: Ry(theta).
    pub const Y: u8 = 1;
    /// Rotation about Z axis: Rz(theta).
    pub const Z: u8 = 2;
}

/// Helper: name string for a rotation axis (for display/debug).
pub fn rot_axis_name(axis: u8) -> &'static str {
    match axis {
        rot_axis::X => "X",
        rot_axis::Y => "Y",
        rot_axis::Z => "Z",
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
        reduce_fn::CONJ_Z => "conj_z",
        reduce_fn::NEGATE_Z => "negate_z",
        reduce_fn::EXPECT => "expect",
        _ => "unknown",
    }
}
