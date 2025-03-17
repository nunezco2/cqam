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
    HybReduce { dst: String, src: String, function: String },
    HybCondExec { condition: String, then_block: String },

    // Control instructions
    Label(String),
    NoOp,
}
