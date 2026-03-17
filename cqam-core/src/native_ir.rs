//! Native (hardware-resolved) circuit IR for the QPU pipeline.
//!
//! Types in this module represent quantum operations after decomposition
//! and routing -- they reference physical qubits and modality-specific gates.
//! Produced by `cqam-micro`'s synthesis pipeline, consumed by `QpuBackend`.

// =============================================================================
// Physical qubit
// =============================================================================

/// A physical qubit index on the target device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalQubit(pub u32);

// =============================================================================
// Native gate enums
// =============================================================================

/// Native single-qubit gate on a specific hardware modality.
#[derive(Debug, Clone)]
pub enum NativeGate1 {
    /// sqrt(X) gate -- native on superconducting.
    Sx,
    /// Pauli-X gate -- native on superconducting.
    X,
    /// Virtual Z-rotation -- native on superconducting.
    Rz(f64),
    /// Identity (delay/idle).
    Id,
}

/// Native two-qubit gate on a specific hardware modality.
#[derive(Debug, Clone)]
pub enum NativeGate2 {
    /// CNOT / CX gate -- native on superconducting.
    Cx,
}

/// The native gate set supported by a specific modality/device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeGateSet {
    /// IBM superconducting: { SX, X, Rz, CX }
    Superconducting,
    /// Trapped-ion: { Rz, Ry, MS }
    TrappedIon,
    /// Neutral-atom: { Rz, Ry, CZ }
    NeutralAtom,
    /// Photonic: { Rz, BS, PS }
    Photonic,
    /// Spin qubit: { Rz, Rx, SWAP }
    Spin,
}

// =============================================================================
// Operation wrapper structs (verb-oriented, mirror circuit_ir convention)
// =============================================================================

/// Apply a native single-qubit gate to a physical qubit.
#[derive(Debug, Clone)]
pub struct ApplyGate1q {
    pub qubit: PhysicalQubit,
    pub gate: NativeGate1,
}

/// Apply a native two-qubit gate to a physical qubit pair.
#[derive(Debug, Clone)]
pub struct ApplyGate2q {
    pub qubit_a: PhysicalQubit,
    pub qubit_b: PhysicalQubit,
    pub gate: NativeGate2,
}

/// Measure a physical qubit into a classical bit.
#[derive(Debug, Clone)]
pub struct Observe {
    pub qubit: PhysicalQubit,
    pub clbit: u32,
}

/// Reset a physical qubit to |0>.
#[derive(Debug, Clone)]
pub struct QubitReset {
    pub qubit: PhysicalQubit,
}

/// Synchronization barrier on a set of physical qubits.
#[derive(Debug, Clone)]
pub struct Barrier {
    pub qubits: Vec<PhysicalQubit>,
}

// =============================================================================
// Op enum
// =============================================================================

/// A native operation on physical qubits.
#[derive(Debug, Clone)]
pub enum Op {
    Gate1q(ApplyGate1q),
    Gate2q(ApplyGate2q),
    Measure(Observe),
    Reset(QubitReset),
    Barrier(Barrier),
}

// =============================================================================
// Circuit container
// =============================================================================

/// A fully resolved circuit ready for hardware submission.
#[derive(Debug, Clone)]
pub struct Circuit {
    /// Physical qubit count.
    pub num_physical_qubits: u32,
    /// Virtual -> physical qubit mapping.
    pub qubit_map: Vec<PhysicalQubit>,
    /// Ops in the native gate set, fully parameterized.
    pub ops: Vec<Op>,
    /// Circuit depth (for metrics).
    pub depth: u32,
    /// SWAP overhead (number of inserted routing SWAPs).
    pub swap_count: u32,
}

impl Circuit {
    /// Create a new empty native circuit.
    pub fn new(num_physical_qubits: u32) -> Self {
        Self {
            num_physical_qubits,
            qubit_map: Vec::new(),
            ops: Vec::new(),
            depth: 0,
            swap_count: 0,
        }
    }

    /// Number of operations in the circuit.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the circuit is empty.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Count of single-qubit gates.
    pub fn gate1q_count(&self) -> usize {
        self.ops.iter().filter(|op| matches!(op, Op::Gate1q(_))).count()
    }

    /// Count of two-qubit gates.
    pub fn gate2q_count(&self) -> usize {
        self.ops.iter().filter(|op| matches!(op, Op::Gate2q(_))).count()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_construction() {
        let c = Circuit::new(5);
        assert_eq!(c.num_physical_qubits, 5);
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
        assert_eq!(c.depth, 0);
        assert_eq!(c.swap_count, 0);
    }

    #[test]
    fn test_physical_qubit_equality() {
        assert_eq!(PhysicalQubit(3), PhysicalQubit(3));
        assert_ne!(PhysicalQubit(3), PhysicalQubit(4));
    }

    #[test]
    fn test_circuit_gate_counts() {
        let mut c = Circuit::new(3);
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(0),
            gate: NativeGate1::Sx,
        }));
        c.ops.push(Op::Gate1q(ApplyGate1q {
            qubit: PhysicalQubit(1),
            gate: NativeGate1::Rz(1.57),
        }));
        c.ops.push(Op::Gate2q(ApplyGate2q {
            qubit_a: PhysicalQubit(0),
            qubit_b: PhysicalQubit(1),
            gate: NativeGate2::Cx,
        }));
        c.ops.push(Op::Measure(Observe {
            qubit: PhysicalQubit(0),
            clbit: 0,
        }));
        assert_eq!(c.len(), 4);
        assert_eq!(c.gate1q_count(), 2);
        assert_eq!(c.gate2q_count(), 1);
    }

    #[test]
    fn test_native_gate_set_equality() {
        assert_eq!(NativeGateSet::Superconducting, NativeGateSet::Superconducting);
        assert_ne!(NativeGateSet::Superconducting, NativeGateSet::TrappedIon);
    }
}
