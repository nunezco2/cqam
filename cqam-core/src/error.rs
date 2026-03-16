//! Unified error type for all CQAM operations.
//!
//! All library functions return `Result<T, CqamError>` rather than panicking,
//! allowing callers to propagate or handle errors explicitly.

use std::fmt;

/// Unified error type for all CQAM operations.
///
/// Covers parsing, execution, I/O, and configuration errors.
/// All library functions return `Result<T, CqamError>` instead of panicking.
#[derive(Debug)]
pub enum CqamError {
    /// Type mismatch during instruction execution (e.g., wrong register file,
    /// wrong HybridValue variant for a reduction).
    TypeMismatch { instruction: String, detail: String },

    /// Unknown quantum kernel ID.
    UnknownKernel(String),

    /// Missing operand during parsing.
    MissingOperand(String),

    /// Parse error with line number and message.
    ParseError { line: usize, message: String },

    /// I/O error (file read/write).
    IoError(std::io::Error),

    /// Configuration file error.
    ConfigError(String),

    /// Invalid opcode in binary program.
    InvalidOpcode(u8),

    /// Decode error at a specific PC.
    DecodeError { pc: usize, message: String },

    /// Program counter out of bounds.
    IllegalPC(usize),

    /// Halt trap (normal or error-induced halt).
    TrapHalt(String),

    /// Division by zero.
    DivisionByZero { instruction: String },

    /// Register index out of bounds.
    RegisterOutOfBounds { file: String, index: u8 },

    /// Attempted to read an uninitialized register (e.g., empty quantum register).
    UninitializedRegister { file: String, index: u8 },

    /// Label not found during assembly / encoding.
    UnresolvedLabel(String),

    /// Duplicate label definition during assembly.
    DuplicateLabel { name: String, first: u32, second: u32 },

    /// Address does not fit in the required bit width.
    AddressOverflow { label: String, address: u32, max_bits: u8 },

    /// Operand value out of range for its bit-field width.
    OperandOverflow { field: String, value: u32, max: u32 },

    /// Invalid binary file (bad magic, truncated, etc.).
    InvalidBinary(String),

    /// Memory address out of range during register-indirect access.
    AddressOutOfRange { instruction: String, address: i64 },

    /// Qubit or state index out of range for a quantum register.
    QuantumIndexOutOfRange { instruction: String, index: usize, limit: usize },

    /// Qubit budget exceeded (e.g., tensor product would exceed MAX_SV_QUBITS).
    QubitLimitExceeded { instruction: String, required: u8, max: u8 },

    /// Error during fork/merge thread operations.
    ///
    /// Covers fork depth limit exceeded, thread panic, thread join failure.
    ForkError(String),

    /// Unknown distribution ID in QPREP instruction.
    UnknownDistribution(u8),

    /// Invalid built-in procedure ID for ECALL.
    InvalidProcedure(u8),

    /// Write to .shared memory outside HATMS/HATME atomic section.
    SharedMemoryViolation { address: u16, thread_id: u16 },

    /// Quantum operation attempted inside HATMS/HATME atomic section.
    QuantumInAtomicSection { instruction: String },

    /// Thread synchronization error (thread failed to reach barrier/merge).
    ThreadSyncError { thread_id: u16, detail: String },

    /// Invalid numeric ID for a typed domain (e.g., kernel, distribution, flag).
    InvalidId { domain: &'static str, value: u8 },

    /// Bell pair budget exhausted during teleportation-based QSTORE/QLOAD.
    BellPairExhausted { instruction: String },
}

impl fmt::Display for CqamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CqamError::TypeMismatch { instruction, detail } => {
                write!(f, "Type mismatch in {}: {}", instruction, detail)
            }
            CqamError::UnknownKernel(name) => {
                write!(f, "Unknown kernel: {}", name)
            }
            CqamError::MissingOperand(msg) => {
                write!(f, "Missing operand: {}", msg)
            }
            CqamError::ParseError { line, message } => {
                write!(f, "Parse error at line {}: {}", line, message)
            }
            CqamError::IoError(err) => {
                write!(f, "I/O error: {}", err)
            }
            CqamError::ConfigError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            CqamError::InvalidOpcode(op) => {
                write!(f, "Invalid opcode: 0x{:02X}", op)
            }
            CqamError::DecodeError { pc, message } => {
                write!(f, "Decode error at PC {}: {}", pc, message)
            }
            CqamError::IllegalPC(pc) => {
                write!(f, "Illegal program counter: {}", pc)
            }
            CqamError::TrapHalt(msg) => {
                write!(f, "Halt: {}", msg)
            }
            CqamError::DivisionByZero { instruction } => {
                write!(f, "Division by zero in {}", instruction)
            }
            CqamError::RegisterOutOfBounds { file, index } => {
                write!(f, "Register out of bounds: {}[{}]", file, index)
            }
            CqamError::UninitializedRegister { file, index } => {
                write!(f, "Uninitialized register: {}[{}]", file, index)
            }
            CqamError::UnresolvedLabel(name) => {
                write!(f, "Unresolved label: {}", name)
            }
            CqamError::DuplicateLabel { name, first, second } => {
                write!(
                    f,
                    "Duplicate label '{}': first at {}, second at {}",
                    name, first, second
                )
            }
            CqamError::AddressOverflow {
                label,
                address,
                max_bits,
            } => {
                write!(
                    f,
                    "Address overflow for '{}': address {} exceeds {}-bit limit",
                    label, address, max_bits
                )
            }
            CqamError::OperandOverflow { field, value, max } => {
                write!(
                    f,
                    "Operand overflow: {} = {} exceeds max {}",
                    field, value, max
                )
            }
            CqamError::InvalidBinary(msg) => {
                write!(f, "Invalid binary: {}", msg)
            }
            CqamError::AddressOutOfRange { instruction, address } => {
                write!(
                    f,
                    "Address out of range in {}: {} is not a valid CMEM address (0..65535)",
                    instruction, address
                )
            }
            CqamError::QuantumIndexOutOfRange { instruction, index, limit } => {
                write!(
                    f,
                    "Index out of range in {}: {} exceeds register dimension {}",
                    instruction, index, limit
                )
            }
            CqamError::QubitLimitExceeded { instruction, required, max } => {
                write!(
                    f,
                    "Qubit limit exceeded in {}: requires {} qubits, maximum is {}",
                    instruction, required, max
                )
            }
            CqamError::ForkError(msg) => {
                write!(f, "Fork error: {}", msg)
            }
            CqamError::UnknownDistribution(id) => {
                write!(f, "Unknown distribution ID: {}", id)
            }
            CqamError::InvalidProcedure(id) => {
                write!(f, "Invalid ECALL procedure ID: {}", id)
            }
            CqamError::SharedMemoryViolation { address, thread_id } => {
                write!(f, "Shared memory violation at address {} from thread {}: writes to .shared only allowed inside HATMS/HATME", address, thread_id)
            }
            CqamError::QuantumInAtomicSection { instruction } => {
                write!(f, "Quantum operation {} not allowed inside HATMS/HATME atomic section", instruction)
            }
            CqamError::ThreadSyncError { thread_id, detail } => {
                write!(f, "Thread synchronization error (thread {}): {}", thread_id, detail)
            }
            CqamError::InvalidId { domain, value } => {
                write!(f, "Invalid {} ID: {}", domain, value)
            }
            CqamError::BellPairExhausted { instruction } => {
                write!(f, "Bell pair budget exhausted during {}", instruction)
            }
        }
    }
}

impl std::error::Error for CqamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CqamError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CqamError {
    fn from(err: std::io::Error) -> Self {
        CqamError::IoError(err)
    }
}
