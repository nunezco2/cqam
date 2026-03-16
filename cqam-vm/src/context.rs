//! Execution context for the CQAM virtual machine.
//!
//! `ExecutionContext` holds the complete machine state: program counter, all
//! register files (integer, float, complex, hybrid, quantum), classical and
//! quantum memory, call stack, program status word, ISR table, and resource
//! tracker. Label resolution is performed once at construction and cached in
//! a `HashMap` for O(1) lookups during execution.

use std::collections::HashMap;
use std::sync::Arc;

use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;
use cqam_core::memory::{CMem, QMem};
use cqam_core::quantum_backend::{QRegHandle, QuantumBackend};
use cqam_core::register::{IntRegFile, FloatRegFile, ComplexRegFile, HybridRegFile};
use crate::isr::IsrTable;
use crate::resource::ResourceTracker;
use crate::psw::ProgramStateWord;
use cqam_core::config::VmConfig;
use crate::thread_pool::SharedMemory;

/// The complete execution state of the CQAM virtual machine.
///
/// Contains the program counter, all register files, memory banks,
/// call stack, PSW, and the program being executed.
#[derive(Clone)]
pub struct ExecutionContext {
    /// Program counter: index into `self.program`.
    pub pc: usize,

    /// Integer register file: R0-R15 (16 x i64).
    pub iregs: IntRegFile,

    /// Float register file: F0-F15 (16 x f64).
    pub fregs: FloatRegFile,

    /// Complex register file: Z0-Z15 (16 x (f64, f64)).
    pub zregs: ComplexRegFile,

    /// Hybrid register file: H0-H7 (8 x HybridValue).
    pub hregs: HybridRegFile,

    /// Quantum register file: Q0-Q7 (8 x `Option<QRegHandle>`).
    /// Separate from QMEM. These are the "live" quantum registers
    /// that QPREP, QKERNEL, and QOBSERVE operate on.
    pub qregs: [Option<QRegHandle>; 8],

    /// Classical memory: 65536 cells of i64.
    pub cmem: CMem,

    /// Quantum memory: 256 slots of QRegHandle.
    pub qmem: QMem<QRegHandle>,

    /// Call stack for CALL/RET instructions.
    /// Each entry is the return address (PC value to resume at).
    pub call_stack: Vec<usize>,

    /// Program status word (condition flags, trap flags).
    pub psw: ProgramStateWord,

    /// VM configuration: fidelity thresholds, qubit defaults, backend flags.
    pub config: VmConfig,

    /// ISR vector table for trap handler dispatch.
    pub isr_table: IsrTable,

    /// Cumulative resource usage tracker.
    pub resource_tracker: ResourceTracker,

    /// The program being executed (IR form: one `Instruction` per word).
    /// Stored in `Arc` so the execution loop can hold an immutable reference
    /// to the program while mutating the rest of the context, avoiding
    /// per-cycle instruction clones.
    pub program: Arc<[Instruction]>,

    /// Label resolution cache: label name -> instruction index.
    /// Populated once during construction by `resolve_labels()`.
    pub labels: HashMap<String, usize>,

    /// Thread identity (0 = primary/single-threaded, 1..N-1 = workers).
    pub thread_id: u16,

    /// Configured thread count (from pragma/CLI, default 1).
    pub thread_count: u16,

    /// Whether this thread is inside a HATMS/HATME atomic section.
    pub in_atomic_section: bool,

    /// Shared memory region bounds (base, size). None if no .shared section.
    pub shared_region: Option<(u16, u16)>,

    /// Whether this thread should skip instructions until HATME (non-leader in SPMD).
    pub(crate) skip_to_hatme: bool,

    /// Reference to shared memory (set during HFORK, None otherwise).
    /// All threads in a parallel region share the same SharedMemory instance.
    pub shared_memory: Option<Arc<SharedMemory>>,

    /// Remaining Bell pairs for QSTORE/QLOAD teleportation.
    pub bell_pair_budget: u32,
}

impl ExecutionContext {
    /// Create a new execution context for the given program.
    ///
    /// Initializes all register files to zero/empty, allocates memory,
    /// and resolves all labels in the program.
    pub fn new(program: Vec<Instruction>) -> Self {
        let program: Arc<[Instruction]> = program.into();
        let mut ctx = Self {
            pc: 0,
            iregs: IntRegFile::new(),
            fregs: FloatRegFile::new(),
            zregs: ComplexRegFile::new(),
            hregs: HybridRegFile::new(),
            qregs: Default::default(), // [None; 8]
            cmem: CMem::new(),
            qmem: QMem::new(),
            call_stack: Vec::new(),
            psw: ProgramStateWord::new(),
            config: VmConfig::default(),
            isr_table: IsrTable::new(),
            resource_tracker: ResourceTracker::new(),
            labels: HashMap::new(),
            thread_id: 0,
            thread_count: 1,
            in_atomic_section: false,
            shared_region: None,
            skip_to_hatme: false,
            shared_memory: None,
            bell_pair_budget: VmConfig::default().bell_pair_budget,
            program,
        };
        ctx.resolve_labels();
        ctx
    }

    /// Store a new handle in a Q register, releasing any previous handle via the backend.
    pub fn set_qreg<B: QuantumBackend + ?Sized>(
        &mut self,
        idx: u8,
        handle: QRegHandle,
        backend: &mut B,
    ) {
        if let Some(old) = self.qregs[idx as usize].take() {
            backend.release(old);
        }
        self.qregs[idx as usize] = Some(handle);
    }

    /// Take a handle from a Q register (for destructive operations).
    pub fn take_qreg(&mut self, idx: u8) -> Option<QRegHandle> {
        self.qregs[idx as usize].take()
    }

    /// Advance the program counter by one.
    pub fn advance_pc(&mut self) {
        self.pc += 1;
    }

    /// Reset the program counter to zero.
    pub fn reset_pc(&mut self) {
        self.pc = 0;
    }

    /// Get the instruction at the current PC, or None if past end-of-program.
    pub fn current_line(&self) -> Option<&Instruction> {
        self.program.get(self.pc)
    }

    /// Jump to the instruction at the given label or numeric address.
    ///
    /// Accepts either a label name (resolved via the label cache) or a
    /// numeric address in `@N` format (used by decoded binary programs).
    /// Returns `Err(CqamError::UnresolvedLabel)` if the label is not found.
    pub fn jump_to_label(&mut self, label: &str) -> Result<(), CqamError> {
        if let Some(&addr) = self.labels.get(label) {
            self.pc = addr;
            Ok(())
        } else if let Some(addr_str) = label.strip_prefix('@') {
            if let Ok(addr) = addr_str.parse::<usize>() {
                self.pc = addr;
                Ok(())
            } else {
                Err(CqamError::UnresolvedLabel(label.to_string()))
            }
        } else {
            Err(CqamError::UnresolvedLabel(label.to_string()))
        }
    }

    /// Push the current PC+1 onto the call stack (for CALL instruction).
    pub fn push_call(&mut self) {
        self.call_stack.push(self.pc + 1);
    }

    /// Pop the top of the call stack (for RET instruction).
    ///
    /// Returns `None` if the call stack is empty (indicating a RET from
    /// the top-level, which should be treated as HALT).
    pub fn pop_call(&mut self) -> Option<usize> {
        self.call_stack.pop()
    }

    // =========================================================================
    // Shared-memory-aware classical memory access
    // =========================================================================

    /// Load an i64 from classical memory, checking shared memory first.
    ///
    /// If `shared_memory` is set and the address falls within the shared region,
    /// the value is read from shared memory (snapshot outside atomic sections,
    /// live data inside). Otherwise falls back to local CMEM.
    pub fn cmem_load(&self, addr: u16) -> i64 {
        if let Some(ref sm) = self.shared_memory {
            if let Some(val) = sm.read(addr, self.in_atomic_section) {
                return val;
            }
        }
        self.cmem.load(addr)
    }

    /// Store an i64 to classical memory, respecting shared-memory constraints.
    ///
    /// If `shared_memory` is set and the address falls within the shared region,
    /// writes are only permitted inside an atomic section (HATMS/HATME).
    /// Returns `Err(CqamError::SharedMemoryViolation)` on illegal writes.
    pub fn cmem_store(&mut self, addr: u16, val: i64) -> Result<(), CqamError> {
        if let Some(ref sm) = self.shared_memory {
            if sm.contains(addr) {
                if self.in_atomic_section {
                    sm.write(addr, val).ok_or(CqamError::SharedMemoryViolation {
                        address: addr,
                        thread_id: self.thread_id,
                    })?;
                } else {
                    return Err(CqamError::SharedMemoryViolation {
                        address: addr,
                        thread_id: self.thread_id,
                    });
                }
                return Ok(());
            }
        }
        self.cmem.store(addr, val);
        Ok(())
    }

    /// Load an f64 from classical memory (stored as bit-cast i64).
    pub fn cmem_load_f64(&self, addr: u16) -> f64 {
        f64::from_bits(self.cmem_load(addr) as u64)
    }

    /// Store an f64 to classical memory (stored as bit-cast i64).
    pub fn cmem_store_f64(&mut self, addr: u16, val: f64) -> Result<(), CqamError> {
        self.cmem_store(addr, val.to_bits() as i64)
    }

    /// Load a complex (f64, f64) from two consecutive classical memory cells.
    pub fn cmem_load_c64(&self, addr: u16) -> (f64, f64) {
        let re = self.cmem_load_f64(addr);
        let im = self.cmem_load_f64(addr.wrapping_add(1));
        (re, im)
    }

    /// Store a complex (f64, f64) to two consecutive classical memory cells.
    pub fn cmem_store_c64(&mut self, addr: u16, val: (f64, f64)) -> Result<(), CqamError> {
        self.cmem_store_f64(addr, val.0)?;
        self.cmem_store_f64(addr.wrapping_add(1), val.1)
    }

    /// Scan the program for Label instructions and populate the label cache.
    ///
    /// Called once during construction. Subsequent label lookups are O(1)
    /// via the HashMap, replacing the previous O(n) linear scan per jump.
    fn resolve_labels(&mut self) {
        self.labels.clear();
        for (idx, instr) in self.program.iter().enumerate() {
            if let Instruction::Label(name) = instr {
                self.labels.insert(name.clone(), idx);
            }
        }
    }
}
