//! Thread lifecycle coordinator for HFORK/HMERGE parallelism.
//!
//! `ForkManager` spawns Rust threads at HFORK and joins them at HMERGE,
//! enforcing a configurable nesting-depth limit to prevent exponential
//! thread growth. It is intentionally not `Clone` because it owns
//! `JoinHandle` values.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use crate::context::ExecutionContext;
use crate::executor::run_program;
use crate::thread_pool::{SharedQuantumFile, SharedMemory, ThreadBarrier};
use cqam_core::error::CqamError;

/// Maximum fork nesting depth. Prevents exponential thread spawning.
pub const DEFAULT_MAX_FORK_DEPTH: u8 = 4;

/// Manages the lifecycle of forked execution threads.
///
/// Created by the runner at the top level. Passed by mutable reference
/// through the execution chain: runner -> executor -> hybrid.
///
/// Each fork thread creates its own ForkManager with incremented depth.
pub struct ForkManager {
    /// Currently active fork threads awaiting HMERGE.
    pub(crate) active_forks: Vec<JoinHandle<Result<ExecutionContext, CqamError>>>,

    /// Completed fork contexts collected after HMERGE join.
    pub completed_forks: Vec<ExecutionContext>,

    /// Current fork nesting depth. Top-level is 0.
    depth: u8,

    /// Maximum allowed fork nesting depth.
    max_depth: u8,

    /// Shared quantum file for the current HFORK/HMERGE block.
    shared_qfile: Option<Arc<SharedQuantumFile>>,

    /// Shared memory for the current HFORK/HMERGE block.
    shared_mem: Option<Arc<SharedMemory>>,

    /// Thread barrier for HATMS/HATME synchronization.
    barrier: Option<Arc<ThreadBarrier>>,
}

impl ForkManager {
    /// Create a new top-level ForkManager with default depth limit.
    pub fn new() -> Self {
        Self {
            active_forks: Vec::new(),
            completed_forks: Vec::new(),
            depth: 0,
            max_depth: DEFAULT_MAX_FORK_DEPTH,
            shared_qfile: None,
            shared_mem: None,
            barrier: None,
        }
    }

    /// Create a ForkManager for a nested fork context.
    ///
    /// Used inside the fork thread's execution loop.
    pub fn nested(parent_depth: u8, max_depth: u8) -> Self {
        Self {
            active_forks: Vec::new(),
            completed_forks: Vec::new(),
            depth: parent_depth.saturating_add(1),
            max_depth,
            shared_qfile: None,
            shared_mem: None,
            barrier: None,
        }
    }

    /// Check whether another fork can be spawned (depth < max_depth).
    pub fn can_fork(&self) -> bool {
        self.depth < self.max_depth
    }

    /// Current nesting depth.
    pub fn depth(&self) -> u8 {
        self.depth
    }

    /// Maximum allowed depth.
    pub fn max_depth(&self) -> u8 {
        self.max_depth
    }

    /// Spawn a fork thread running the given context to completion.
    ///
    /// The context is moved into the spawned thread. The thread creates
    /// its own nested ForkManager and calls `run_program`.
    pub fn spawn_fork(&mut self, fork_ctx: ExecutionContext) -> Result<(), CqamError> {
        if !self.can_fork() {
            return Err(CqamError::ForkError(format!(
                "Fork depth limit exceeded (max: {})",
                self.max_depth
            )));
        }

        let depth = self.depth;
        let max_depth = self.max_depth;

        let handle = thread::Builder::new()
            .name(format!("cqam-fork-d{}", depth + 1))
            .spawn(move || {
                let mut ctx = fork_ctx;
                let mut fm = ForkManager::nested(depth, max_depth);
                run_program(&mut ctx, &mut fm)?;
                Ok(ctx)
            })
            .map_err(CqamError::IoError)?;

        self.active_forks.push(handle);
        Ok(())
    }

    /// Join all active fork threads and collect their final contexts.
    ///
    /// On success, moves all results into `self.completed_forks`.
    /// On first error, remaining threads are still joined to avoid leaks.
    pub fn join_all(&mut self) -> Result<(), CqamError> {
        let handles: Vec<_> = self.active_forks.drain(..).collect();
        let mut first_error: Option<CqamError> = None;

        for handle in handles {
            match handle.join() {
                Ok(Ok(ctx)) => {
                    if first_error.is_none() {
                        self.completed_forks.push(ctx);
                    }
                }
                Ok(Err(e)) => {
                    if first_error.is_none() {
                        first_error = Some(CqamError::ForkError(format!(
                            "Fork thread returned error: {}",
                            e
                        )));
                    }
                }
                Err(_panic) => {
                    if first_error.is_none() {
                        first_error = Some(CqamError::ForkError(
                            "Fork thread panicked".to_string(),
                        ));
                    }
                }
            }
        }

        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Number of currently active (unjoined) fork threads.
    pub fn active_count(&self) -> usize {
        self.active_forks.len()
    }

    /// Drain and return all completed fork contexts.
    pub fn take_completed(&mut self) -> Vec<ExecutionContext> {
        std::mem::take(&mut self.completed_forks)
    }

    /// Store shared resources for thread 0 during an HFORK/HMERGE block.
    pub fn set_shared_resources(
        &mut self,
        qfile: Arc<SharedQuantumFile>,
        mem: Arc<SharedMemory>,
        barrier: Arc<ThreadBarrier>,
    ) {
        self.shared_qfile = Some(qfile);
        self.shared_mem = Some(mem);
        self.barrier = Some(barrier);
    }

    /// Take the shared quantum file (consumes the stored reference).
    pub fn take_shared_qfile(&mut self) -> Option<Arc<SharedQuantumFile>> {
        self.shared_qfile.take()
    }

    /// Take the shared memory (consumes the stored reference).
    pub fn take_shared_mem(&mut self) -> Option<Arc<SharedMemory>> {
        self.shared_mem.take()
    }

    /// Get a reference to the shared memory (non-consuming).
    pub fn get_shared_mem(&self) -> Option<&Arc<SharedMemory>> {
        self.shared_mem.as_ref()
    }

    /// Get a reference to the barrier.
    pub fn get_barrier(&self) -> Option<&Arc<ThreadBarrier>> {
        self.barrier.as_ref()
    }
}

impl Default for ForkManager {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time assertions: ExecutionContext and CqamError must be Send.
const _: () = {
    fn assert_send<T: Send>() {}
    fn _assert_bounds() {
        assert_send::<ExecutionContext>();
        assert_send::<CqamError>();
    }
};
