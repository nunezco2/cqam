//! Resource cost accounting for CQAM instruction execution.
//!
//! Defines `ResourceDelta` (per-instruction cost) and `ResourceTracker`
//! (cumulative totals). The `resource_cost` function maps each instruction
//! variant to its associated time, space, superposition, entanglement, and
//! interference costs.

use cqam_core::instruction::Instruction;

/// Per-instruction or per-kernel resource cost delta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResourceDelta {
    pub time: usize,
    pub space: usize,
    pub superposition: f64,
    pub entanglement: f64,
    pub interference: f64,
}

/// Tracks cumulative resource usage across execution.
#[derive(Debug, Default, Clone)]
pub struct ResourceTracker {
    pub total_time: usize,
    pub total_space: usize,
    pub total_superposition: f64,
    pub total_entanglement: f64,
    pub total_interference: f64,
}

impl ResourceTracker {
    pub fn new() -> Self {
        Self::default()
    }

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
        Instruction::QPrep { .. } => ResourceDelta {
            time: 2,
            space: 2,
            superposition: 1.0,
            ..Default::default()
        },

        // Quantum kernel: 3 cycles, may create entanglement
        Instruction::QKernel { .. } => ResourceDelta {
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

        // Quantum load/store: 1 cycle
        Instruction::QLoad { .. }
        | Instruction::QStore { .. } => ResourceDelta {
            time: 1,
            space: 1,
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
        Instruction::HCExec { .. } => ResourceDelta {
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
