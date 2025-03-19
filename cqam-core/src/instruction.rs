/// Instruction represents a classical, hybrid, or quantum operation in the CQAM architecture.
/// Each variant may have associated operands tagged as register, literal, label, or function.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// No operation (placeholder or alignment)
    /// Operand: None
    NoOp,

    /// Label definition: used as a jump target
    /// Operand: label (String)
    /// Example: `LABEL: LOOP`
    Label(String),

    /// Classical load: load a literal or memory address into a register
    /// Operands: dst (register), src (literal or memory address)
    /// Example: `CL:LOAD R1, 42`
    ClLoad { dst: String, src: String },

    /// Classical store: store register value into memory address
    /// Operands: addr (memory address), src (register)
    /// Example: `CL:STORE result, R1`
    ClStore { addr: String, src: String },

    /// Classical addition: dst = lhs + rhs
    /// Operands: dst (register), lhs (register), rhs (register or literal)
    /// Example: `CL:ADD R3, R1, R2`
    ClAdd { dst: String, lhs: String, rhs: String },

    /// Classical subtraction: dst = lhs - rhs
    /// Operands: dst (register), lhs (register), rhs (register or literal)
    /// Example: `CL:SUB R4, R3, R1`
    ClSub { dst: String, lhs: String, rhs: String },

    /// Classical unconditional jump to a label
    /// Operand: label (String)
    /// Example: `CL:JMP LOOP`
    ClJump { label: String },

    /// Classical conditional jump if predicate is true
    /// Operands: pred (register), label (String)
    /// Example: `CL:IF cond, THEN`
    ClIf { pred: String, label: String },

    /// Hybrid fork: initiate a quantum-classical control fork
    /// Operand: None
    /// Example: `HYB:FORK`
    HybFork,

    /// Hybrid merge: join previously forked hybrid branches
    /// Operand: None
    /// Example: `HYB:MERGE`
    HybMerge,

    /// Hybrid conditional execution based on control flag
    /// Operands: flag (status flag name), then_label (label)
    /// Example: `HYB:COND_EXEC QF, THEN`
    HybCondExec { flag: String, then_label: String },

    /// Hybrid reduction on quantum-classical value
    /// Operands: src (register), dst (register), function (reduction function name)
    /// Example: `HYB:REDUCE x, y, round`
    HybReduce { src: String, dst: String, function: String },

    /// Quantum preparation of a distribution
    /// Operands: dst (quantum register), dist_src (named distribution source)
    /// Example: `QPREP q1, dist_uniform`
    QPrep { dst: String, dist_src: String },

    /// Quantum kernel transformation
    /// Operands: dst (quantum register), src (quantum register), kernel (function name), ctx (optional register context)
    /// Example: `QKERNEL q2, q1, modexp`
    QKernel { dst: String, src: String, kernel: String, ctx: Option<String> },

    /// Quantum measurement
    /// Operands: dst (classical register), src (quantum register)
    /// Example: `QMEAS m1, q2`
    QMeas { dst: String, src: String },

    /// Quantum observation (non-destructive)
    /// Operands: dst (classical register), src (quantum register)
    /// Example: `QOBSERVE m2, q3`
    QObserve { dst: String, src: String },

    /// Program halt instruction (explicit termination)
    /// Operand: None
    /// Example: `HALT`
    Halt
}
