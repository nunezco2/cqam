//! Breakpoint management for the debugger engine.
//!
//! Provides `BreakpointTable`, `Breakpoint`, and `BreakpointKind` for
//! address-based, label-based, class-based, and conditional breakpoints.

use cqam_core::instruction::*;
use crate::engine::condition::Condition;
use crate::format::instruction::instruction_class;

/// Instruction class categories for `break class` commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrClass {
    Quantum,
    Hybrid,
    Branch,
    Memory,
    Ecall,
    Float,
    Complex,
}

impl InstrClass {
    /// Parse a class name string (case-insensitive) into an InstrClass.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "quantum" => Some(Self::Quantum),
            "hybrid" => Some(Self::Hybrid),
            "branch" => Some(Self::Branch),
            "memory" => Some(Self::Memory),
            "ecall" => Some(Self::Ecall),
            "float" => Some(Self::Float),
            "complex" => Some(Self::Complex),
            _ => None,
        }
    }

    /// Return the string name for this class.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Quantum => "quantum",
            Self::Hybrid => "hybrid",
            Self::Branch => "branch",
            Self::Memory => "memory",
            Self::Ecall => "ecall",
            Self::Float => "float",
            Self::Complex => "complex",
        }
    }

    /// Check if an instruction matches this class.
    pub fn matches(&self, instr: &Instruction) -> bool {
        instruction_class(instr) == Some(self.name())
    }
}

/// The kind of breakpoint trigger.
#[derive(Debug, Clone)]
pub enum BreakpointKind {
    /// Break at a specific PC address.
    Address(usize),
    /// Break at a labeled address. Stores (label_name, resolved_address).
    Label(String, usize),
    /// Break on any instruction of the given class.
    InstructionClass(InstrClass),
}

/// A single breakpoint entry.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Unique monotonically increasing breakpoint ID.
    pub id: usize,
    /// What triggers this breakpoint.
    pub kind: BreakpointKind,
    /// Whether this breakpoint is currently enabled.
    pub enabled: bool,
    /// Number of times this breakpoint has been hit.
    pub hit_count: usize,
    /// Optional condition: breakpoint only fires when this evaluates to true.
    pub condition: Option<Condition>,
}

impl Breakpoint {
    /// Create a new enabled breakpoint with no condition.
    fn new(id: usize, kind: BreakpointKind) -> Self {
        Self {
            id,
            kind,
            enabled: true,
            hit_count: 0,
            condition: None,
        }
    }

    /// Create a new enabled breakpoint with a condition.
    fn new_conditional(id: usize, kind: BreakpointKind, condition: Condition) -> Self {
        Self {
            id,
            kind,
            enabled: true,
            hit_count: 0,
            condition: Some(condition),
        }
    }

    /// Check if this breakpoint should fire at the given PC for the given instruction.
    ///
    /// Returns `true` if the breakpoint is enabled, matches the location/class,
    /// and any condition is satisfied. The condition is NOT evaluated here --
    /// the caller must check `self.condition` separately against the VM state.
    pub fn matches_location(&self, pc: usize, instr: &Instruction) -> bool {
        if !self.enabled {
            return false;
        }
        match &self.kind {
            BreakpointKind::Address(addr) => pc == *addr,
            BreakpointKind::Label(_, addr) => pc == *addr,
            BreakpointKind::InstructionClass(class) => class.matches(instr),
        }
    }

    /// Return a human-readable description of this breakpoint.
    pub fn describe(&self) -> String {
        let kind_str = match &self.kind {
            BreakpointKind::Address(addr) => format!("at 0x{:04X}", addr),
            BreakpointKind::Label(name, addr) => format!("at {} (0x{:04X})", name, addr),
            BreakpointKind::InstructionClass(class) => format!("class {}", class.name()),
        };
        let state = if self.enabled { "enabled" } else { "disabled" };
        let cond_str = match &self.condition {
            Some(c) => format!(" if {}", c.describe()),
            None => String::new(),
        };
        format!("#{}: {} [{}] hits={}{}", self.id, kind_str, state, self.hit_count, cond_str)
    }
}

/// Table of all breakpoints, indexed by monotonically increasing IDs.
#[derive(Debug, Clone)]
pub struct BreakpointTable {
    breakpoints: Vec<Breakpoint>,
    next_id: usize,
}

impl BreakpointTable {
    /// Create a new empty breakpoint table.
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            next_id: 1,
        }
    }

    /// Add an address breakpoint. Returns the new breakpoint ID.
    pub fn add_address(&mut self, addr: usize) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.breakpoints.push(Breakpoint::new(id, BreakpointKind::Address(addr)));
        id
    }

    /// Add a label breakpoint. Returns the new breakpoint ID.
    pub fn add_label(&mut self, name: String, addr: usize) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.breakpoints.push(Breakpoint::new(id, BreakpointKind::Label(name, addr)));
        id
    }

    /// Add an instruction class breakpoint. Returns the new breakpoint ID.
    pub fn add_class(&mut self, class: InstrClass) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.breakpoints
            .push(Breakpoint::new(id, BreakpointKind::InstructionClass(class)));
        id
    }

    /// Add a conditional address breakpoint. Returns the new breakpoint ID.
    pub fn add_conditional(&mut self, addr: usize, condition: Condition) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.breakpoints
            .push(Breakpoint::new_conditional(id, BreakpointKind::Address(addr), condition));
        id
    }

    /// Remove a breakpoint by ID. Returns `true` if found and removed.
    pub fn remove(&mut self, id: usize) -> bool {
        let len_before = self.breakpoints.len();
        self.breakpoints.retain(|bp| bp.id != id);
        self.breakpoints.len() < len_before
    }

    /// Remove all breakpoints.
    pub fn remove_all(&mut self) {
        self.breakpoints.clear();
    }

    /// Enable a breakpoint by ID. Returns `true` if found.
    pub fn enable(&mut self, id: usize) -> bool {
        if let Some(bp) = self.breakpoints.iter_mut().find(|bp| bp.id == id) {
            bp.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable a breakpoint by ID. Returns `true` if found.
    pub fn disable(&mut self, id: usize) -> bool {
        if let Some(bp) = self.breakpoints.iter_mut().find(|bp| bp.id == id) {
            bp.enabled = false;
            true
        } else {
            false
        }
    }

    /// Check if any breakpoint fires at the given PC for the given instruction.
    ///
    /// Returns the list of matching breakpoint IDs. The caller is responsible
    /// for evaluating conditions and incrementing hit counts.
    pub fn check(&self, pc: usize, instr: &Instruction) -> Vec<usize> {
        self.breakpoints
            .iter()
            .filter(|bp| bp.matches_location(pc, instr))
            .map(|bp| bp.id)
            .collect()
    }

    /// Record a hit on a breakpoint by ID.
    pub fn record_hit(&mut self, id: usize) {
        if let Some(bp) = self.breakpoints.iter_mut().find(|bp| bp.id == id) {
            bp.hit_count += 1;
        }
    }

    /// Get a breakpoint by ID.
    pub fn get(&self, id: usize) -> Option<&Breakpoint> {
        self.breakpoints.iter().find(|bp| bp.id == id)
    }

    /// Return an iterator over all breakpoints.
    pub fn iter(&self) -> impl Iterator<Item = &Breakpoint> {
        self.breakpoints.iter()
    }

    /// Return the number of breakpoints.
    pub fn len(&self) -> usize {
        self.breakpoints.len()
    }

    /// Return true if there are no breakpoints.
    pub fn is_empty(&self) -> bool {
        self.breakpoints.is_empty()
    }

    /// Return the number of enabled breakpoints.
    pub fn enabled_count(&self) -> usize {
        self.breakpoints.iter().filter(|bp| bp.enabled).count()
    }

    /// Check if a specific PC address has a breakpoint set (enabled or disabled).
    pub fn has_breakpoint_at(&self, pc: usize) -> Option<&Breakpoint> {
        self.breakpoints.iter().find(|bp| match &bp.kind {
            BreakpointKind::Address(addr) | BreakpointKind::Label(_, addr) => *addr == pc,
            _ => false,
        })
    }
}

impl Default for BreakpointTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_check_address() {
        let mut table = BreakpointTable::new();
        let id = table.add_address(0x0010);
        assert_eq!(id, 1);
        assert_eq!(table.len(), 1);

        let instr = Instruction::Halt;
        let hits = table.check(0x0010, &instr);
        assert_eq!(hits, vec![1]);

        let no_hits = table.check(0x0020, &instr);
        assert!(no_hits.is_empty());
    }

    #[test]
    fn test_add_and_check_class() {
        let mut table = BreakpointTable::new();
        table.add_class(InstrClass::Quantum);

        let quantum_instr = Instruction::QPrep { dst: 0, dist: DistId::Uniform };
        let hits = table.check(0, &quantum_instr);
        assert_eq!(hits.len(), 1);

        let classical_instr = Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 };
        let no_hits = table.check(0, &classical_instr);
        assert!(no_hits.is_empty());
    }

    #[test]
    fn test_disable_enable() {
        let mut table = BreakpointTable::new();
        let id = table.add_address(0x0010);

        table.disable(id);
        let hits = table.check(0x0010, &Instruction::Halt);
        assert!(hits.is_empty());

        table.enable(id);
        let hits = table.check(0x0010, &Instruction::Halt);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut table = BreakpointTable::new();
        let id = table.add_address(0x0010);
        assert_eq!(table.len(), 1);

        assert!(table.remove(id));
        assert_eq!(table.len(), 0);

        assert!(!table.remove(id)); // Already removed.
    }

    #[test]
    fn test_remove_all() {
        let mut table = BreakpointTable::new();
        table.add_address(0x0010);
        table.add_address(0x0020);
        table.add_class(InstrClass::Branch);
        assert_eq!(table.len(), 3);

        table.remove_all();
        assert!(table.is_empty());
    }

    #[test]
    fn test_hit_count() {
        let mut table = BreakpointTable::new();
        let id = table.add_address(0x0010);

        table.record_hit(id);
        table.record_hit(id);
        assert_eq!(table.get(id).unwrap().hit_count, 2);
    }

    #[test]
    fn test_instr_class_from_name() {
        assert_eq!(InstrClass::from_name("quantum"), Some(InstrClass::Quantum));
        assert_eq!(InstrClass::from_name("BRANCH"), Some(InstrClass::Branch));
        assert_eq!(InstrClass::from_name("unknown"), None);
    }

    #[test]
    fn test_has_breakpoint_at() {
        let mut table = BreakpointTable::new();
        table.add_address(0x0010);
        assert!(table.has_breakpoint_at(0x0010).is_some());
        assert!(table.has_breakpoint_at(0x0020).is_none());
    }
}
