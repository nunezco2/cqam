// cqam-core/src/memory.rs
//
// Phase 2: Numeric-addressed memory with fixed sizes.
// Replaces the old HashMap-based CMEM/QMEM and removes dead HybridReg.

use cqam_sim::qdist::QDist;

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

/// Quantum memory: 256 slots of QDist<u16>, addressed by u8.
///
/// Each slot is initially unoccupied (None). Slots are populated by QStore
/// and read by QLoad. This is separate from the quantum register file
/// (Q0-Q7 in ExecutionContext).
///
/// QMEM slots hold 8-qubit distributions (domain 0..255), while the quantum
/// register file holds 16-qubit distributions (domain 0..65535). The
/// distinction allows QMEM to serve as a quantum "swap space" for saving
/// and restoring partial quantum states.
#[derive(Debug, Clone)]
pub struct QMem {
    slots: Vec<Option<QDist<u16>>>,
}

impl Default for QMem {
    fn default() -> Self {
        Self {
            slots: (0..256).map(|_| None).collect(),
        }
    }
}

impl QMem {
    /// Create a new quantum memory with 256 empty slots.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the quantum distribution at QMEM[addr].
    ///
    /// Returns None if the slot is unoccupied.
    pub fn load(&self, addr: u8) -> Option<&QDist<u16>> {
        self.slots[addr as usize].as_ref()
    }

    /// Store a quantum distribution at QMEM[addr].
    ///
    /// Overwrites any existing distribution in that slot.
    pub fn store(&mut self, addr: u8, dist: QDist<u16>) {
        self.slots[addr as usize] = Some(dist);
    }

    /// Take (remove and return) the quantum distribution at QMEM[addr].
    ///
    /// Leaves the slot empty (None). Useful for destructive operations.
    pub fn take(&mut self, addr: u8) -> Option<QDist<u16>> {
        self.slots[addr as usize].take()
    }

    /// Check if a slot is occupied.
    pub fn is_occupied(&self, addr: u8) -> bool {
        self.slots[addr as usize].is_some()
    }
}
