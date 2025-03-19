// cqam-core/src/instruction.rs

/// Enum representing CQAM instruction types
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// No operation (placeholder or alignment)
    NoOp,

    /// Label definition: used as a jump target
    /// Example: `LABEL: LOOP`
    Label(String),

    /// Classical load: load a literal or memory address into a register
    /// Example: `CL:LOAD R1, 42`
    ClLoad { dst: String, src: String },

    /// Classical store: store register value into memory address
    /// Example: `CL:STORE result, R1`
    ClStore { addr: String, src: String },

    /// Classical addition: dst = lhs + rhs
    /// Example: `CL:ADD R3, R1, R2`
    ClAdd { dst: String, lhs: String, rhs: String },

    /// Classical subtraction: dst = lhs - rhs
    /// Example: `CL:SUB R4, R3, R1`
    ClSub { dst: String, lhs: String, rhs: String },

    /// Classical unconditional jump to a label
    /// Example: `CL:JMP LOOP`
    ClJump { label: String },

    /// Classical conditional jump if predicate is true
    /// Example: `CL:IF cond, THEN`
    ClIf { pred: String, label: String },

    /// Hybrid fork: initiate a quantum-classical control fork
    /// Example: `HYB:FORK`
    HybFork,

    /// Hybrid merge: join previously forked hybrid branches
    /// Example: `HYB:MERGE`
    HybMerge,

    /// Hybrid conditional execution based on control flag
    /// Example: `HYB:COND_EXEC QF, THEN`
    HybCondExec { flag: String, then_label: String },

    /// Hybrid reduction on quantum-classical value
    /// Example: `HYB:REDUCE x, y, round`
    HybReduce { src: String, dst: String, function: String },

    /// Quantum preparation of a distribution
    /// Example: `QPREP q1, dist_uniform`
    QPrep { dst: String, dist_src: String },

    /// Quantum kernel transformation
    /// Example: `QKERNEL q2, q1, apply_modexp`
    QKernel { dst: String, src: String, kernel: String, ctx: Option<String> },

    /// Quantum measurement
    /// Example: `QMEAS m1, q2`
    QMeas { dst: String, src: String },

    /// Quantum observation (non-destructive)
    /// Example: `QOBSERVE m2, q3`
    QObserve { dst: String, src: String },

    /// Program halt instruction (explicit termination)
    /// Example: `HALT`
    Halt

    // Optional future variants...
    // Extendable: Halts, interrupts, and system-level ops
    // Halt,
    // SysCall(String), etc.
}
