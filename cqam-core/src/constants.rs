//! Centralized constants for the cqam-core crate.
//!
//! Register file sizes, memory capacities, and other architectural constants
//! that define the CQAM virtual machine's resource limits.

/// Number of integer registers (R0-R15).
pub const INT_REG_COUNT: usize = 16;

/// Number of floating-point registers (F0-F15).
pub const FLOAT_REG_COUNT: usize = 16;

/// Number of complex registers (Z0-Z15).
pub const COMPLEX_REG_COUNT: usize = 16;

/// Number of hybrid registers (H0-H7).
pub const H_REG_COUNT: usize = 8;

/// Number of classical memory cells (addressed by u16).
pub const CMEM_SIZE: usize = 65536;

/// Number of quantum memory slots (addressed by u8).
pub const QMEM_SLOTS: usize = 256;

/// Minimum collection size to use parallel (Rayon) iteration.
/// Below this threshold, sequential iteration is faster due to thread-pool overhead.
pub const PAR_THRESHOLD: usize = 256;
