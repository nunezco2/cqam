// cqam-core/src/error.rs
//
// Phase 4: Unified error type for the CQAM project.
// Replaces panic!() and expect() throughout library code with Result<T, CqamError>.

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
