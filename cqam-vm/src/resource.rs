//! Resource cost accounting for CQAM instruction execution.
//!
//! Defines `ResourceDelta` (per-instruction cost) and `ResourceTracker`
//! (cumulative totals). The `resource_cost` function maps each instruction
//! variant to its associated time, space, superposition, entanglement, and
//! interference costs.

use cqam_core::instruction::Instruction;

/// Per-instruction resource cost contribution.
///
/// Returned by [`resource_cost`] for each instruction and accumulated in a
/// [`ResourceTracker`]. Units are intentionally abstract; they provide a
/// relative measure of computational weight rather than wall-clock time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResourceDelta {
    /// Simulated clock cycles consumed (1 for most classical ops, 2-4 for complex/quantum).
    pub time: usize,
    /// Register or memory slots written (0-2 depending on instruction).
    pub space: usize,
    /// Incremental superposition cost (non-zero for QPREP and QKERNEL).
    pub superposition: f64,
    /// Incremental entanglement cost (non-zero for QKERNEL).
    pub entanglement: f64,
    /// Incremental interference cost (non-zero for QOBSERVE).
    pub interference: f64,
}

/// Cumulative resource usage totals across an entire program execution.
///
/// Populated by summing [`ResourceDelta`] values returned by [`resource_cost`]
/// for each executed instruction. Reported by `cqam-run --resource-usage`.
#[derive(Debug, Default, Clone)]
pub struct ResourceTracker {
    /// Total simulated cycles elapsed.
    pub total_time: usize,
    /// Total register/memory slots written.
    pub total_space: usize,
    /// Cumulative superposition cost.
    pub total_superposition: f64,
    /// Cumulative entanglement cost.
    pub total_entanglement: f64,
    /// Cumulative interference cost.
    pub total_interference: f64,
}

impl ResourceTracker {
    /// Create a new zero-initialised resource tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Accumulate a per-instruction cost delta into the running totals.
    pub fn apply_delta(&mut self, delta: &ResourceDelta) {
        self.total_time += delta.time;
        self.total_space += delta.space;
        self.total_superposition += delta.superposition;
        self.total_entanglement += delta.entanglement;
        self.total_interference += delta.interference;
    }
}

/// Compute the resource cost for a given instruction.
///
/// Moved from `executor.rs` to keep the executor focused on dispatch logic.
/// This function assigns fixed costs per instruction type.
pub fn resource_cost(instr: &Instruction) -> ResourceDelta {
    match instr {
        // Integer arithmetic: 1 cycle, 1 register write
        Instruction::IAdd { .. }
        | Instruction::ISub { .. }
        | Instruction::IMul { .. }
        | Instruction::IDiv { .. }
        | Instruction::IMod { .. } => ResourceDelta {
            time: 1,
            space: 1,
            ..Default::default()
        },

        // Integer bitwise: 1 cycle, 1 register write
        Instruction::IAnd { .. }
        | Instruction::IOr { .. }
        | Instruction::IXor { .. }
        | Instruction::INot { .. }
        | Instruction::IShl { .. }
        | Instruction::IShr { .. } => ResourceDelta {
            time: 1,
            space: 1,
            ..Default::default()
        },

        // Integer memory: 1 cycle, 1 memory access
        Instruction::ILdi { .. }
        | Instruction::ILdm { .. }
        | Instruction::IStr { .. } => ResourceDelta {
            time: 1,
            space: 1,
            ..Default::default()
        },

        // Register-indirect memory: same cost as direct memory
        Instruction::ILdx { .. }
        | Instruction::IStrx { .. }
        | Instruction::FLdx { .. }
        | Instruction::FStrx { .. } => ResourceDelta {
            time: 1,
            space: 1,
            ..Default::default()
        },

        // Register-indirect complex memory: 2 cycles (two cells)
        Instruction::ZLdx { .. }
        | Instruction::ZStrx { .. } => ResourceDelta {
            time: 2,
            space: 1,
            ..Default::default()
        },

        // Integer comparison: 1 cycle, 1 register write
        Instruction::IEq { .. }
        | Instruction::ILt { .. }
        | Instruction::IGt { .. } => ResourceDelta {
            time: 1,
            space: 1,
            ..Default::default()
        },

        // Float arithmetic: 1 cycle, 1 register write
        Instruction::FAdd { .. }
        | Instruction::FSub { .. }
        | Instruction::FMul { .. }
        | Instruction::FDiv { .. } => ResourceDelta {
            time: 1,
            space: 1,
            ..Default::default()
        },

        // Float memory
        Instruction::FLdi { .. }
        | Instruction::FLdm { .. }
        | Instruction::FStr { .. } => ResourceDelta {
            time: 1,
            space: 1,
            ..Default::default()
        },

        // Float comparison
        Instruction::FEq { .. }
        | Instruction::FLt { .. }
        | Instruction::FGt { .. } => ResourceDelta {
            time: 1,
            space: 1,
            ..Default::default()
        },

        // Complex arithmetic: 2 cycles (two f64 operations)
        Instruction::ZAdd { .. }
        | Instruction::ZSub { .. } => ResourceDelta {
            time: 2,
            space: 1,
            ..Default::default()
        },

        // Complex multiply/divide: 4 cycles (four f64 operations)
        Instruction::ZMul { .. }
        | Instruction::ZDiv { .. } => ResourceDelta {
            time: 4,
            space: 1,
            ..Default::default()
        },

        // Complex memory: 2 cycles (two memory accesses)
        Instruction::ZLdi { .. }
        | Instruction::ZLdm { .. }
        | Instruction::ZStr { .. } => ResourceDelta {
            time: 2,
            space: 1,
            ..Default::default()
        },

        // Type conversion: 1 cycle
        Instruction::CvtIF { .. }
        | Instruction::CvtFI { .. }
        | Instruction::CvtFZ { .. }
        | Instruction::CvtZF { .. } => ResourceDelta {
            time: 1,
            space: 0,
            ..Default::default()
        },

        // Thread configuration: 1 cycle
        Instruction::ICCfg { .. }
        | Instruction::ITid { .. } => ResourceDelta {
            time: 1,
            space: 0,
            ..Default::default()
        },

        // Configuration query: 1 cycle (reads config, no memory access)
        Instruction::IQCfg { .. } => ResourceDelta {
            time: 1,
            space: 0,
            ..Default::default()
        },

        // Environment call: 2 cycles (dispatch + I/O), no register writes
        Instruction::Ecall { .. } => ResourceDelta {
            time: 2,
            space: 0,
            ..Default::default()
        },

        // Control flow: 1 cycle, no register/memory effects
        Instruction::Jmp { .. }
        | Instruction::Jif { .. }
        | Instruction::Call { .. }
        | Instruction::Ret
        | Instruction::Halt => ResourceDelta {
            time: 1,
            space: 0,
            ..Default::default()
        },

        // Quantum prep: 2 cycles, creates superposition
        Instruction::QPrep { .. }
        | Instruction::QPrepR { .. } => ResourceDelta {
            time: 2,
            space: 2,
            superposition: 1.0,
            ..Default::default()
        },

        // Quantum encode: 1 cycle, quantum operation
        Instruction::QEncode { .. } => ResourceDelta {
            time: 1,
            space: 1,
            superposition: 1.0,
            ..Default::default()
        },

        // Quantum kernel: KernelId::Diffuse cycles, may create entanglement
        Instruction::QKernel { .. }
        | Instruction::QKernelF { .. }
        | Instruction::QKernelZ { .. } => ResourceDelta {
            time: 3,
            space: 2,
            superposition: 0.5,
            entanglement: 0.7,
            ..Default::default()
        },

        // Quantum observe: 1 cycle, collapses state
        Instruction::QObserve { .. } => ResourceDelta {
            time: 1,
            space: 1,
            interference: 0.3,
            ..Default::default()
        },

        // Masked Hadamard: 2 cycles, creates/destroys superposition
        Instruction::QHadM { .. } => ResourceDelta {
            time: 2,
            space: 1,
            superposition: 0.5,
            entanglement: 0.0,
            interference: 0.0,
        },

        // Masked bit flip: 1 cycle, classical-like operation
        Instruction::QFlip { .. } => ResourceDelta {
            time: 1,
            space: 1,
            superposition: 0.0,
            entanglement: 0.0,
            interference: 0.0,
        },

        // Masked phase flip: 1 cycle, modest superposition impact
        Instruction::QPhase { .. } => ResourceDelta {
            time: 1,
            space: 1,
            superposition: 0.2,
            entanglement: 0.0,
            interference: 0.0,
        },

        // Two-qubit CNOT gate: 4 cycles, creates entanglement
        Instruction::QCnot { .. } => ResourceDelta {
            time: 4,
            space: 2,
            superposition: 0.5,
            entanglement: 1.0,
            interference: 0.0,
        },

        // Parameterized rotation: 3 cycles (trig computation + gate application)
        Instruction::QRot { .. } => ResourceDelta {
            time: 3,
            space: 1,
            superposition: 0.5,
            entanglement: 0.0,
            interference: 0.0,
        },

        // Partial measurement: 2 cycles, collapses one qubit
        Instruction::QMeas { .. } => ResourceDelta {
            time: 2,
            space: 1,
            superposition: 0.0,
            entanglement: 0.0,
            interference: 0.5,
        },

        // Tensor product: 4 cycles, space grows quadratically
        Instruction::QTensor { .. } => ResourceDelta {
            time: 4,
            space: 4,
            superposition: 0.0,
            entanglement: 0.0,
            interference: 0.0,
        },

        // Custom unitary: expensive -- 5 cycles + heavy memory reads
        Instruction::QCustom { .. } => ResourceDelta {
            time: 5,
            space: 2,
            superposition: 0.5,
            entanglement: 0.7,
            interference: 0.0,
        },

        // CZ: same cost as CNOT
        Instruction::QCz { .. } => ResourceDelta {
            time: 4,
            space: 2,
            superposition: 0.5,
            entanglement: 1.0,
            interference: 0.0,
        },

        // SWAP: slightly cheaper (no entanglement creation in isolation)
        Instruction::QSwap { .. } => ResourceDelta {
            time: 4,
            space: 2,
            superposition: 0.0,
            entanglement: 0.5,
            interference: 0.0,
        },

        // QMIXED: mixed state preparation -- expensive
        Instruction::QMixed { .. } => ResourceDelta {
            time: 5,
            space: 4,
            superposition: 1.0,
            entanglement: 0.0,
            interference: 0.0,
        },

        // QPREPN: variable qubit count preparation
        Instruction::QPrepN { .. } => ResourceDelta {
            time: 2,
            space: 2,
            superposition: 1.0,
            ..Default::default()
        },

        // Trig functions: 2 cycles (more expensive than basic float ops)
        Instruction::FSin { .. }
        | Instruction::FCos { .. }
        | Instruction::FAtan2 { .. }
        | Instruction::FSqrt { .. } => ResourceDelta {
            time: 2,
            space: 1,
            ..Default::default()
        },

        // Partial trace: 3 cycles
        Instruction::QPtrace { .. } => ResourceDelta {
            time: 3,
            space: 2,
            superposition: 0.0,
            entanglement: -0.5,
            interference: 0.3,
        },

        // Reset = measure + conditional X: 3 cycles
        Instruction::QReset { .. } => ResourceDelta {
            time: 3,
            space: 1,
            superposition: 0.0,
            entanglement: 0.0,
            interference: 0.5,
        },

        // Quantum load/store: 3 cycles (teleportation), 1 Bell pair consumed
        Instruction::QLoad { .. }
        | Instruction::QStore { .. } => ResourceDelta {
            time: 3,
            space: 1,
            entanglement: 1.0,
            ..Default::default()
        },

        // Atomic section barriers: 2 cycles (barrier synchronization)
        Instruction::HAtmS
        | Instruction::HAtmE => ResourceDelta {
            time: 2,
            space: 0,
            ..Default::default()
        },

        // Hybrid fork/merge: 1 cycle
        Instruction::HFork
        | Instruction::HMerge => ResourceDelta {
            time: 1,
            space: 0,
            ..Default::default()
        },

        // Hybrid conditional exec: 1 cycle
        Instruction::JmpF { .. } => ResourceDelta {
            time: 1,
            space: 0,
            ..Default::default()
        },

        // Hybrid reduce: 2 cycles
        Instruction::HReduce { .. } => ResourceDelta {
            time: 2,
            space: 1,
            ..Default::default()
        },

        // Interrupt handling: 1 cycle
        Instruction::Reti
        | Instruction::SetIV { .. } => ResourceDelta {
            time: 1,
            space: 0,
            ..Default::default()
        },

        // No-op / labels: zero cost
        Instruction::Nop
        | Instruction::Label(_) => ResourceDelta::default(),
    }
}
