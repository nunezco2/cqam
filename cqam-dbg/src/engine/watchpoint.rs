//! Watchpoint management for the debugger engine.
//!
//! Watchpoints trigger when a watched register's value changes between steps.
//! Only classical registers are supported: R0--R15, F0--F15, Z0--Z15.

use crate::engine::snapshot::RegisterSnapshot;
use cqam_vm::context::ExecutionContext;

/// Which register file a watchpoint targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchedRegFile {
    /// Integer register file (R0--R15).
    Int,
    /// Float register file (F0--F15).
    Float,
    /// Complex register file (Z0--Z15).
    Complex,
}

/// A single watchpoint on a classical register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watchpoint {
    /// The register file.
    pub file: WatchedRegFile,
    /// The register index (0--15).
    pub index: u8,
}

impl Watchpoint {
    /// Return a human-readable name for this watchpoint, e.g. "R3", "F0", "Z12".
    pub fn name(&self) -> String {
        let prefix = match self.file {
            WatchedRegFile::Int => "R",
            WatchedRegFile::Float => "F",
            WatchedRegFile::Complex => "Z",
        };
        format!("{}{}", prefix, self.index)
    }

    /// Parse a register name like "R3", "F0", "Z12" into a Watchpoint.
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        if name.len() < 2 {
            return None;
        }
        let prefix = &name[..1];
        let idx_str = &name[1..];
        let idx: u8 = idx_str.parse().ok()?;

        let file = match prefix.to_uppercase().as_str() {
            "R" => {
                if idx > 15 { return None; }
                WatchedRegFile::Int
            }
            "F" => {
                if idx > 15 { return None; }
                WatchedRegFile::Float
            }
            "Z" => {
                if idx > 15 { return None; }
                WatchedRegFile::Complex
            }
            _ => return None,
        };

        Some(Self { file, index: idx })
    }

    /// Check if this watchpoint's register changed between the snapshot and current state.
    pub fn triggered(&self, snapshot: &RegisterSnapshot, ctx: &ExecutionContext) -> bool {
        match self.file {
            WatchedRegFile::Int => snapshot.ireg_changed(ctx, self.index as usize),
            WatchedRegFile::Float => snapshot.freg_changed(ctx, self.index as usize),
            WatchedRegFile::Complex => snapshot.zreg_changed(ctx, self.index as usize),
        }
    }
}

/// Table of active watchpoints.
#[derive(Debug, Clone)]
pub struct WatchpointTable {
    watchpoints: Vec<Watchpoint>,
}

impl WatchpointTable {
    /// Create a new empty watchpoint table.
    pub fn new() -> Self {
        Self {
            watchpoints: Vec::new(),
        }
    }

    /// Add a watchpoint. Returns `true` if it was added (not already present).
    pub fn add(&mut self, wp: Watchpoint) -> bool {
        if self.watchpoints.contains(&wp) {
            false
        } else {
            self.watchpoints.push(wp);
            true
        }
    }

    /// Remove a watchpoint by register name. Returns `true` if found and removed.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(wp) = Watchpoint::parse(name) {
            let len_before = self.watchpoints.len();
            self.watchpoints.retain(|w| w != &wp);
            self.watchpoints.len() < len_before
        } else {
            false
        }
    }

    /// Remove all watchpoints.
    pub fn remove_all(&mut self) {
        self.watchpoints.clear();
    }

    /// Check all watchpoints and return the names of those that triggered.
    pub fn check(&self, snapshot: &RegisterSnapshot, ctx: &ExecutionContext) -> Vec<String> {
        self.watchpoints
            .iter()
            .filter(|wp| wp.triggered(snapshot, ctx))
            .map(|wp| wp.name())
            .collect()
    }

    /// Return an iterator over all watchpoints.
    pub fn iter(&self) -> impl Iterator<Item = &Watchpoint> {
        self.watchpoints.iter()
    }

    /// Return the number of watchpoints.
    pub fn len(&self) -> usize {
        self.watchpoints.len()
    }

    /// Return true if there are no watchpoints.
    pub fn is_empty(&self) -> bool {
        self.watchpoints.is_empty()
    }

}

impl Default for WatchpointTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::instruction::Instruction;

    fn make_ctx() -> ExecutionContext {
        ExecutionContext::new(vec![Instruction::Halt])
    }

    #[test]
    fn test_parse_watchpoint() {
        let wp = Watchpoint::parse("R3").unwrap();
        assert_eq!(wp.file, WatchedRegFile::Int);
        assert_eq!(wp.index, 3);

        let wp = Watchpoint::parse("F15").unwrap();
        assert_eq!(wp.file, WatchedRegFile::Float);
        assert_eq!(wp.index, 15);

        let wp = Watchpoint::parse("Z0").unwrap();
        assert_eq!(wp.file, WatchedRegFile::Complex);
        assert_eq!(wp.index, 0);

        assert!(Watchpoint::parse("R16").is_none());
        assert!(Watchpoint::parse("Q0").is_none());
        assert!(Watchpoint::parse("X").is_none());
    }

    #[test]
    fn test_watchpoint_name() {
        let wp = Watchpoint { file: WatchedRegFile::Int, index: 7 };
        assert_eq!(wp.name(), "R7");
    }

    #[test]
    fn test_add_duplicate() {
        let mut table = WatchpointTable::new();
        let wp = Watchpoint::parse("R3").unwrap();
        assert!(table.add(wp.clone()));
        assert!(!table.add(wp)); // duplicate
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut table = WatchpointTable::new();
        table.add(Watchpoint::parse("R3").unwrap());
        assert!(table.remove("R3"));
        assert!(table.is_empty());
        assert!(!table.remove("R3")); // already removed
    }

    #[test]
    fn test_check_triggered() {
        let mut ctx = make_ctx();
        let snap = RegisterSnapshot::capture(&ctx);
        ctx.iregs.regs[3] = 42;

        let mut table = WatchpointTable::new();
        table.add(Watchpoint::parse("R3").unwrap());
        table.add(Watchpoint::parse("R0").unwrap());

        let triggered = table.check(&snap, &ctx);
        assert_eq!(triggered, vec!["R3"]);
    }

    #[test]
    fn test_remove_all() {
        let mut table = WatchpointTable::new();
        table.add(Watchpoint::parse("R0").unwrap());
        table.add(Watchpoint::parse("F1").unwrap());
        table.remove_all();
        assert!(table.is_empty());
    }
}
