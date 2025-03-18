// cqam-core/src/instruction.rs

/// Enum representing CQAM instruction types
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // Classical instructions
    ClLoad { dst: String, src: String },
    ClAdd { dst: String, lhs: String, rhs: String },
    ClSub { dst: String, lhs: String, rhs: String },
    ClStore { addr: String, src: String },
    ClJump { label: String },
    ClIf { pred: String, label: String },

    // Quantum operations
    QPrep { dst: String, dist_src: String },
    QKernel { dst: String, src: String, kernel: String, ctx: Option<String> },
    QMeas { dst: String, src: String },
    QObserve { dst: String, src: String },

    // Hybrid instructions
    HybFork,
    HybMerge,
    HybReduce { dst: String, src: String, function: String },
    HybCondExec { flag: String, then_label: String },

    // Control instructions
    Label(String),
    Halt,
    NoOp,
}
