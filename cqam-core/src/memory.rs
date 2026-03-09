//! Classical and quantum memory abstractions for the CQAM virtual machine.
//!
//! Provides `CMem` (64K cells of i64, addressed by u16) and `QMem<Q>` (256
//! slots of generic quantum state, addressed by u8). `QMem` is generic over
//! `Q: QuantumState` so that `cqam-core` has no compile-time dependency on the
//! concrete simulation backend in `cqam-sim`.

use crate::quantum_state::QuantumState;

/// Classical memory: 65536 cells of i64, addressed by u16.
///
/// Each cell is initialized to zero. The memory is heap-allocated via Vec
/// to avoid a 512KB stack allocation.
///
/// Accessed by ILdm, IStr, FLdm, FStr, ZLdm, ZStr instructions.
#[derive(Debug, Clone)]
pub struct CMem {
    cells: Vec<i64>,
}

impl Default for CMem {
    fn default() -> Self {
        Self {
            cells: vec![0i64; 65536],
        }
    }
}

impl CMem {
    /// Create a new zero-initialized classical memory (64K cells).
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the value at CMEM[addr].
    ///
    /// Always succeeds because addr is u16 and the memory has 65536 cells.
    pub fn load(&self, addr: u16) -> i64 {
        self.cells[addr as usize]
    }

    /// Store a value at CMEM[addr].
    ///
    /// Always succeeds because addr is u16 and the memory has 65536 cells.
    pub fn store(&mut self, addr: u16, val: i64) {
        self.cells[addr as usize] = val;
    }

    /// Return the number of cells (always 65536).
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Check if any cell has been written (useful for reporting).
    /// NOTE: This is O(n). For frequent use, consider tracking dirty addresses.
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|&c| c == 0)
    }

    /// Pre-load a slice of values into CMEM starting at address 0.
    ///
    /// Used by the runner to install `.data` section content before execution.
    pub fn load_data(&mut self, data: &[i64]) {
        for (i, &val) in data.iter().enumerate() {
            self.cells[i] = val;
        }
    }

    /// Iterate over all non-zero (addr, value) pairs.
    /// Useful for printing memory dumps without iterating all 64K cells.
    pub fn non_zero_entries(&self) -> impl Iterator<Item = (u16, i64)> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != 0)
            .map(|(i, v)| (i as u16, *v))
    }
}

// --- Quantum memory ----------------------------------------------------------

/// Quantum memory: 256 slots of quantum state, addressed by u8.
///
/// Generic over `Q: QuantumState` so that cqam-core has zero dependency
/// on the concrete simulation backend (cqam-sim).
///
/// Each slot is initially unoccupied (None). Slots are populated by QStore
/// and read by QLoad. This is separate from the quantum register file
/// (Q0-Q7 in ExecutionContext).
#[derive(Debug, Clone)]
pub struct QMem<Q: QuantumState> {
    slots: Vec<Option<Q>>,
}

impl<Q: QuantumState> Default for QMem<Q> {
    fn default() -> Self {
        Self {
            slots: (0..256).map(|_| None).collect(),
        }
    }
}

impl<Q: QuantumState> QMem<Q> {
    /// Create a new quantum memory with 256 empty slots.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the quantum state at QMEM[addr].
    ///
    /// Returns None if the slot is unoccupied.
    pub fn load(&self, addr: u8) -> Option<&Q> {
        self.slots[addr as usize].as_ref()
    }

    /// Store a quantum state at QMEM[addr].
    ///
    /// Overwrites any existing state in that slot.
    pub fn store(&mut self, addr: u8, state: Q) {
        self.slots[addr as usize] = Some(state);
    }

    /// Take (remove and return) the quantum state at QMEM[addr].
    ///
    /// Leaves the slot empty (None). Useful for destructive operations.
    pub fn take(&mut self, addr: u8) -> Option<Q> {
        self.slots[addr as usize].take()
    }

    /// Check if a slot is occupied.
    pub fn is_occupied(&self, addr: u8) -> bool {
        self.slots[addr as usize].is_some()
    }
}
