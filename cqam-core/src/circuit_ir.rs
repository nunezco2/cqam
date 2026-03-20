//! High-level circuit IR for the QPU pipeline.
//!
//! Types in this module represent quantum operations before decomposition
//! into hardware-native gates. They are produced by `CircuitBackend` during
//! eager buffering and consumed by `cqam-micro`'s synthesis pipeline.

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

use crate::complex::C64;
use crate::instruction::{DistId, KernelId, ObserveMode};
use crate::quantum_backend::{KernelParams, QRegHandle};

// =============================================================================
// Wire and parameter types
// =============================================================================

/// A logical qubit wire in the circuit IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QWire(pub u32);

/// When a symbolic parameter is resolved during compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingTier {
    /// Resolved during decomposition (before routing).
    Early,
    /// Resolved after routing but before optimization.
    Semi,
    /// Resolved at submission time (latest possible).
    Late,
}

/// A parameter that may be resolved immediately or bound later.
#[derive(Debug, Clone)]
pub enum Param {
    /// Resolved at buffer time.
    Resolved(f64),
    /// Symbolic placeholder resolved at a later pipeline stage.
    Symbolic {
        name: String,
        tier: Option<BindingTier>,
    },
}

impl Param {
    /// Returns true if this parameter is resolved.
    pub fn is_resolved(&self) -> bool {
        matches!(self, Param::Resolved(_))
    }

    /// Returns the resolved value, or None if symbolic.
    pub fn value(&self) -> Option<f64> {
        match self {
            Param::Resolved(v) => Some(*v),
            Param::Symbolic { .. } => None,
        }
    }

    /// Returns the binding tier, or None if resolved or unspecified.
    pub fn tier(&self) -> Option<BindingTier> {
        match self {
            Param::Resolved(_) => None,
            Param::Symbolic { tier, .. } => *tier,
        }
    }
}

// =============================================================================
// Gate enums
// =============================================================================

/// Symbolic single-qubit gate types.
/// Terse names for well-known gates, descriptive for uncommon ones.
#[derive(Debug, Clone)]
pub enum Gate1q {
    H,
    X,
    Y,
    Z,
    S,
    Sdg,
    T,
    Tdg,
    Rx(Param),
    Ry(Param),
    Rz(Param),
    U3(Param, Param, Param),
    /// Explicit 2x2 matrix (fallback for non-standard gates).
    Custom(Box<[C64; 4]>),
}

/// Symbolic two-qubit gate types.
#[derive(Debug, Clone)]
pub enum Gate2q {
    Cx,
    Cz,
    Swap,
    EchoCrossResonance,
    /// Explicit 4x4 matrix (fallback for non-standard gates).
    Custom(Box<[C64; 16]>),
}

// =============================================================================
// Operation wrapper structs (verb-oriented)
// =============================================================================

/// Prepare wire(s) in a known state distribution.
#[derive(Debug, Clone)]
pub struct Prepare {
    pub wires: Vec<QWire>,
    pub dist: DistId,
}

/// Apply a named kernel to a contiguous wire group.
#[derive(Debug, Clone)]
pub struct ApplyKernel {
    pub wires: Vec<QWire>,
    pub kernel: KernelId,
    pub params: KernelParams,
}

/// Apply a single-qubit gate.
#[derive(Debug, Clone)]
pub struct ApplyGate1q {
    pub wire: QWire,
    pub gate: Gate1q,
}

/// Apply a two-qubit gate.
#[derive(Debug, Clone)]
pub struct ApplyGate2q {
    pub wire_a: QWire,
    pub wire_b: QWire,
    pub gate: Gate2q,
}

/// Observe (measure) wires.
#[derive(Debug, Clone)]
pub struct Observe {
    pub wires: Vec<QWire>,
    pub mode: ObserveMode,
    pub ctx0: usize,
    pub ctx1: usize,
}

/// Synchronization barrier.
#[derive(Debug, Clone)]
pub struct Barrier {
    pub wires: Vec<QWire>,
}

/// Conditional qubit reset.
#[derive(Debug, Clone)]
pub struct Reset {
    pub wire: QWire,
}

/// Product-state preparation: each qubit is independently rotated from |0>.
#[derive(Debug, Clone)]
pub struct PrepProduct {
    /// Wires to prepare (one per qubit).
    pub wires: Vec<QWire>,
    /// Per-qubit (alpha, beta) pairs. Already normalized by caller.
    pub amplitudes: Vec<(C64, C64)>,
}

// =============================================================================
// Op enum
// =============================================================================

/// A high-level operation in the circuit IR.
/// Each variant wraps a verb-oriented struct for ergonomic access.
#[derive(Debug, Clone)]
pub enum Op {
    Prep(Prepare),
    Kernel(ApplyKernel),
    Gate1q(ApplyGate1q),
    Gate2q(ApplyGate2q),
    CustomUnitary {
        wires: Vec<QWire>,
        matrix: Vec<C64>,
    },
    Measure(Observe),
    Barrier(Barrier),
    MeasQubit { wire: QWire },
    Reset(Reset),
    PrepProduct(PrepProduct),
}

// =============================================================================
// MicroProgram container
// =============================================================================

/// A quantum micro-program at any abstraction level.
/// Buffered by CircuitBackend from QPREP through QOBSERVE.
#[derive(Debug, Clone)]
pub struct MicroProgram {
    /// Number of logical qubits (wires).
    pub num_wires: u32,
    /// Wire allocation table: QWire -> (QRegHandle, qubit_index).
    pub wire_map: Vec<(QRegHandle, u8)>,
    /// Ordered sequence of operations.
    pub ops: Vec<Op>,
    /// Structure hash for template caching. Two circuits with the same
    /// structure_key differ only in Param::Resolved values.
    pub structure_key: Option<u64>,
}

impl MicroProgram {
    /// Create a new empty micro-program.
    pub fn new(num_wires: u32) -> Self {
        Self {
            num_wires,
            wire_map: Vec::new(),
            ops: Vec::new(),
            structure_key: None,
        }
    }

    /// Push an operation onto the program.
    pub fn push(&mut self, op: Op) {
        self.ops.push(op);
        self.structure_key = None; // invalidate cache
    }

    /// Compute (or retrieve cached) structure key.
    /// The key captures op sequence and structure but normalizes
    /// all Param::Resolved values so that circuits differing only
    /// in parameter values share the same key.
    pub fn compute_structure_key(&mut self) -> u64 {
        if let Some(key) = self.structure_key {
            return key;
        }
        let mut hasher = DefaultHasher::new();
        self.num_wires.hash(&mut hasher);
        for op in &self.ops {
            structural_hash_op(op, &mut hasher);
        }
        let key = hasher.finish();
        self.structure_key = Some(key);
        key
    }
}

// =============================================================================
// Structural hashing (normalizes parameter values)
// =============================================================================

fn structural_hash_op<H: Hasher>(op: &Op, state: &mut H) {
    std::mem::discriminant(op).hash(state);
    match op {
        Op::Prep(p) => {
            p.wires.hash(state);
            (p.dist as u8).hash(state);
        }
        Op::Kernel(k) => {
            k.wires.hash(state);
            (k.kernel as u8).hash(state);
            std::mem::discriminant(&k.params).hash(state);
        }
        Op::Gate1q(g) => {
            g.wire.hash(state);
            structural_hash_gate1q(&g.gate, state);
        }
        Op::Gate2q(g) => {
            g.wire_a.hash(state);
            g.wire_b.hash(state);
            structural_hash_gate2q(&g.gate, state);
        }
        Op::CustomUnitary { wires, matrix } => {
            wires.hash(state);
            matrix.len().hash(state);
        }
        Op::Measure(o) => {
            o.wires.hash(state);
            (o.mode as u8).hash(state);
        }
        Op::Barrier(b) => {
            b.wires.hash(state);
        }
        Op::MeasQubit { wire } => {
            wire.hash(state);
        }
        Op::Reset(r) => {
            r.wire.hash(state);
        }
        Op::PrepProduct(pp) => {
            pp.wires.hash(state);
            pp.amplitudes.len().hash(state);
            // Amplitude values are "parameter-like" -- hash them for uniqueness
            for (a, b) in &pp.amplitudes {
                a.0.to_bits().hash(state);
                a.1.to_bits().hash(state);
                b.0.to_bits().hash(state);
                b.1.to_bits().hash(state);
            }
        }
    }
}

fn structural_hash_gate1q<H: Hasher>(gate: &Gate1q, state: &mut H) {
    std::mem::discriminant(gate).hash(state);
    // Only hash Custom matrix data -- parameter VALUES are normalized away
    if let Gate1q::Custom(m) = gate {
        for c in m.iter() {
            c.0.to_bits().hash(state);
            c.1.to_bits().hash(state);
        }
    }
}

fn structural_hash_gate2q<H: Hasher>(gate: &Gate2q, state: &mut H) {
    std::mem::discriminant(gate).hash(state);
    if let Gate2q::Custom(m) = gate {
        for c in m.iter() {
            c.0.to_bits().hash(state);
            c.1.to_bits().hash(state);
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_program_construction() {
        let mp = MicroProgram::new(4);
        assert_eq!(mp.num_wires, 4);
        assert!(mp.ops.is_empty());
        assert!(mp.structure_key.is_none());
    }

    #[test]
    fn test_push_ops() {
        let mut mp = MicroProgram::new(3);
        mp.push(Op::Prep(Prepare {
            wires: vec![QWire(0), QWire(1), QWire(2)],
            dist: DistId::Zero,
        }));
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(0),
            wire_b: QWire(1),
            gate: Gate2q::Cx,
        }));
        mp.push(Op::Measure(Observe {
            wires: vec![QWire(0), QWire(1), QWire(2)],
            mode: ObserveMode::Dist,
            ctx0: 0,
            ctx1: 0,
        }));
        assert_eq!(mp.ops.len(), 4);
    }

    #[test]
    fn test_structure_key_same_for_different_param_values() {
        let mut mp1 = MicroProgram::new(1);
        mp1.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::Rx(Param::Resolved(1.0)),
        }));

        let mut mp2 = MicroProgram::new(1);
        mp2.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::Rx(Param::Resolved(2.0)),
        }));

        assert_eq!(mp1.compute_structure_key(), mp2.compute_structure_key());
    }

    #[test]
    fn test_structure_key_differs_for_different_op_sequences() {
        let mut mp1 = MicroProgram::new(1);
        mp1.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));

        let mut mp2 = MicroProgram::new(1);
        mp2.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::X,
        }));

        assert_ne!(mp1.compute_structure_key(), mp2.compute_structure_key());
    }

    #[test]
    fn test_structure_key_invalidated_on_push() {
        let mut mp = MicroProgram::new(2);
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));
        let _key = mp.compute_structure_key();
        assert!(mp.structure_key.is_some());
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(1),
            gate: Gate1q::X,
        }));
        assert!(mp.structure_key.is_none());
    }

    #[test]
    fn test_structure_key_differs_for_different_wires() {
        let mut mp1 = MicroProgram::new(2);
        mp1.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));

        let mut mp2 = MicroProgram::new(2);
        mp2.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(1),
            gate: Gate1q::H,
        }));

        assert_ne!(mp1.compute_structure_key(), mp2.compute_structure_key());
    }

    #[test]
    fn test_structure_key_cached() {
        let mut mp = MicroProgram::new(1);
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));
        let key1 = mp.compute_structure_key();
        let key2 = mp.compute_structure_key();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_param_helpers() {
        let resolved = Param::Resolved(3.14);
        assert!(resolved.is_resolved());
        assert_eq!(resolved.value(), Some(3.14));
        assert_eq!(resolved.tier(), None);

        let symbolic = Param::Symbolic {
            name: "theta".to_string(),
            tier: Some(BindingTier::Late),
        };
        assert!(!symbolic.is_resolved());
        assert_eq!(symbolic.value(), None);
        assert_eq!(symbolic.tier(), Some(BindingTier::Late));

        let untiered = Param::Symbolic {
            name: "phi".to_string(),
            tier: None,
        };
        assert_eq!(untiered.tier(), None);
    }

    #[test]
    fn test_qwire_equality() {
        assert_eq!(QWire(0), QWire(0));
        assert_ne!(QWire(0), QWire(1));
    }

    #[test]
    fn test_binding_tier_equality() {
        assert_eq!(BindingTier::Early, BindingTier::Early);
        assert_ne!(BindingTier::Early, BindingTier::Late);
    }

    #[test]
    fn test_gate2q_custom_boxed() {
        let matrix = Box::new([C64::ZERO; 16]);
        let gate = Gate2q::Custom(matrix);
        let _cloned = gate.clone();
    }

    #[test]
    fn test_structure_key_symbolic_same_as_resolved() {
        // Structural hash normalizes params: Resolved and Symbolic with same
        // gate discriminant should produce the same key
        let mut mp1 = MicroProgram::new(1);
        mp1.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::Rx(Param::Resolved(1.0)),
        }));

        let mut mp2 = MicroProgram::new(1);
        mp2.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::Rx(Param::Symbolic {
                name: "theta".to_string(),
                tier: Some(BindingTier::Late),
            }),
        }));

        assert_eq!(mp1.compute_structure_key(), mp2.compute_structure_key());
    }

    #[test]
    fn test_complex_microprogram() {
        let mut mp = MicroProgram::new(3);
        mp.push(Op::Prep(Prepare {
            wires: vec![QWire(0), QWire(1), QWire(2)],
            dist: DistId::Zero,
        }));
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(0),
            wire_b: QWire(1),
            gate: Gate2q::Cx,
        }));
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(1),
            wire_b: QWire(2),
            gate: Gate2q::Cx,
        }));
        mp.push(Op::Measure(Observe {
            wires: vec![QWire(0), QWire(1), QWire(2)],
            mode: ObserveMode::Dist,
            ctx0: 0,
            ctx1: 0,
        }));
        assert_eq!(mp.ops.len(), 5);
        let key = mp.compute_structure_key();
        assert!(key != 0);
    }
}
