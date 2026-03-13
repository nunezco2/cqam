//! Shared state types for HFORK/HMERGE parallel execution.
//!
//! Three core abstractions enable multi-threaded quantum-classical execution:
//!
//! - [`SharedQuantumFile`]: per-register mutex for concurrent quantum access.
//! - [`SharedMemory`]: snapshot-commit model for `.shared` classical memory.
//! - [`ThreadBarrier`]: reusable barrier for HATMS/HATME synchronization.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock};

use cqam_core::quantum_backend::QRegHandle;

// =============================================================================
// SharedQuantumFile
// =============================================================================

/// Shared quantum register file for multi-threaded execution.
///
/// Each Q register is independently locked. Threads block on contention
/// for the same register but can operate on different registers concurrently.
pub struct SharedQuantumFile {
    registers: [Arc<Mutex<Option<QRegHandle>>>; 8],
}

impl SharedQuantumFile {
    /// Create from an existing quantum register array (at HFORK time).
    pub fn from_qregs(mut qregs: [Option<QRegHandle>; 8]) -> Self {
        Self {
            registers: std::array::from_fn(|i| {
                Arc::new(Mutex::new(qregs[i].take()))
            }),
        }
    }

    /// Acquire exclusive access to Q[idx]. Blocks if another thread holds it.
    pub fn lock(&self, idx: u8) -> MutexGuard<'_, Option<QRegHandle>> {
        self.registers[idx as usize].lock().unwrap()
    }

    /// Get an Arc clone for sharing with worker threads.
    pub fn arc_register(&self, idx: usize) -> Arc<Mutex<Option<QRegHandle>>> {
        Arc::clone(&self.registers[idx])
    }

    /// Extract registers back into a plain array (at HMERGE time).
    /// Should only be called after all threads have joined.
    pub fn into_qregs(self) -> [Option<QRegHandle>; 8] {
        std::array::from_fn(|i| {
            Arc::try_unwrap(self.registers[i].clone())
                .unwrap_or_else(|arc| {
                    // Fallback: lock and clone if unwrap fails
                    Mutex::new(*arc.lock().unwrap())
                })
                .into_inner()
                .unwrap()
        })
    }
}

// =============================================================================
// SharedMemory
// =============================================================================

/// Configuration for the shared memory region.
#[derive(Debug, Clone)]
pub struct SharedRegionConfig {
    /// Base address in CMEM.
    pub base: u16,
    /// Size in cells.
    pub size: u16,
}

/// Shared memory region with snapshot-commit consistency.
///
/// Writes only permitted inside HATMS/HATME atomic sections.
/// Reads outside atomic sections return the last-committed snapshot.
pub struct SharedMemory {
    /// Live data (written during atomic sections).
    data: RwLock<Vec<i64>>,
    /// Region bounds.
    config: SharedRegionConfig,
    /// Snapshot from most recent HATME (or HFORK for initial).
    snapshot: RwLock<Vec<i64>>,
}

impl SharedMemory {
    pub fn new(config: SharedRegionConfig, initial_data: &[i64]) -> Self {
        let data = initial_data.to_vec();
        let snapshot = data.clone();
        Self {
            data: RwLock::new(data),
            config,
            snapshot: RwLock::new(snapshot),
        }
    }

    /// Check if an address falls within the shared region.
    pub fn contains(&self, addr: u16) -> bool {
        addr >= self.config.base && addr < self.config.base + self.config.size
    }

    /// Read a cell. Uses snapshot outside atomic sections, live data inside.
    pub fn read(&self, addr: u16, in_atomic: bool) -> Option<i64> {
        let offset = self.addr_to_offset(addr)?;
        if in_atomic {
            let data = self.data.read().unwrap();
            Some(data[offset])
        } else {
            let snap = self.snapshot.read().unwrap();
            Some(snap[offset])
        }
    }

    /// Write a cell. Only valid inside atomic sections.
    /// Returns `None` if the address is outside the shared region.
    pub fn write(&self, addr: u16, value: i64) -> Option<()> {
        let offset = self.addr_to_offset(addr)?;
        let mut data = self.data.write().unwrap();
        data[offset] = value;
        Some(())
    }

    /// Commit current data as the new snapshot (called at HATME).
    pub fn commit_snapshot(&self) {
        let data = self.data.read().unwrap();
        let mut snapshot = self.snapshot.write().unwrap();
        snapshot.copy_from_slice(&data);
    }

    /// Copy final shared state back into a CMEM slice (at HMERGE).
    pub fn write_back(&self, cmem: &mut cqam_core::memory::CMem) {
        let data = self.data.read().unwrap();
        for (i, &val) in data.iter().enumerate() {
            let addr = self.config.base.wrapping_add(i as u16);
            cmem.store(addr, val);
        }
    }

    /// Get the region config.
    pub fn config(&self) -> &SharedRegionConfig {
        &self.config
    }

    fn addr_to_offset(&self, addr: u16) -> Option<usize> {
        if addr >= self.config.base && addr < self.config.base + self.config.size {
            Some((addr - self.config.base) as usize)
        } else {
            None
        }
    }
}

// =============================================================================
// ThreadBarrier
// =============================================================================

/// Result of arriving at a barrier.
pub struct BarrierWaitResult {
    /// True if this thread was elected to execute the atomic section.
    pub is_leader: bool,
}

/// Reusable barrier for HATMS/HATME synchronization.
///
/// All N threads must arrive before any can proceed.
pub struct ThreadBarrier {
    inner: Mutex<BarrierState>,
    cvar: Condvar,
    thread_count: u16,
}

struct BarrierState {
    arrived: u16,
    generation: u64,
    /// The thread elected leader during the current gathering phase.
    elected: Option<u16>,
    /// The resolved leader ID, set by the last arriver for woken threads.
    /// Each generation writes this before notify_all, and woken threads
    /// read it before any new generation can overwrite `elected`.
    resolved_leader: u16,
}

impl ThreadBarrier {
    pub fn new(thread_count: u16) -> Self {
        Self {
            inner: Mutex::new(BarrierState {
                arrived: 0,
                generation: 0,
                elected: None,
                resolved_leader: 0,
            }),
            cvar: Condvar::new(),
            thread_count,
        }
    }

    /// Wait at the barrier. Returns whether this thread is the elected leader.
    ///
    /// The first thread to arrive is elected leader. All threads block until
    /// the last thread arrives. Each thread then checks whether it was the
    /// elected leader.
    pub fn wait(&self, thread_id: u16) -> BarrierWaitResult {
        let mut state = self.inner.lock().unwrap();
        let my_generation = state.generation;

        state.arrived += 1;
        if state.arrived == 1 {
            state.elected = Some(thread_id);
        }

        if state.arrived == self.thread_count {
            // Last to arrive: resolve leader, reset for next generation
            let elected = state.elected.take().unwrap();
            state.resolved_leader = elected;
            state.arrived = 0;
            state.generation += 1;
            self.cvar.notify_all();
            BarrierWaitResult {
                is_leader: thread_id == elected,
            }
        } else {
            // Wait for all threads
            while state.generation == my_generation {
                state = self.cvar.wait(state).unwrap();
            }
            // Read the resolved leader (safe: set before generation changed)
            BarrierWaitResult {
                is_leader: thread_id == state.resolved_leader,
            }
        }
    }
}
