//! Simulator configuration for the CQAM runner.
//!
//! [`SimConfig`] is loaded from a TOML file at startup (via `--config`) or
//! constructed with [`SimConfig::default`]. It controls cycle limits, interrupt
//! masking, and the quantum fidelity threshold that triggers a
//! `QuantumError` interrupt when a register's superposition or entanglement
//! metric drops below the specified value.
//!
//! # TOML format
//!
//! ```toml
//! fidelity_threshold = 0.95
//! max_cycles         = 1000
//! enable_interrupts  = true
//! default_qubits     = 2
//! ```

use std::fs;
use serde::Deserialize;
use cqam_core::error::CqamError;

/// Simulator configuration loaded from a TOML file or built from defaults.
///
/// All fields are `Option<T>` to support partial TOML files; `None` means
/// "use the hardcoded default" and is handled by the runner at startup.
#[derive(Debug, Deserialize)]
pub struct SimConfig {
    /// Minimum acceptable quantum fidelity metric.
    ///
    /// When the superposition or entanglement metric of a quantum register
    /// drops below this value after a `QKERNEL` or `QOBSERVE`, the VM sets
    /// the `int_quantum_err` PSW flag and dispatches a QuantumError interrupt.
    /// Default: `0.95`.
    pub fidelity_threshold: Option<f64>,

    /// Maximum number of instructions to execute before forcing a halt.
    ///
    /// Prevents infinite loops in untrusted programs. Default: `1000`.
    pub max_cycles: Option<usize>,

    /// Whether maskable interrupts (Arithmetic, QuantumError, SyncFailure) are
    /// dispatched through the ISR table.
    ///
    /// When `false`, all maskable traps are silently discarded.
    /// Default: `true`.
    pub enable_interrupts: Option<bool>,

    /// Default number of qubits per quantum register (1-16).
    ///
    /// Controls the dimension of the density matrix allocated by `QPREP`.
    /// A register with `n` qubits has a 2^n x 2^n density matrix.
    /// Default: `2` (4-state).
    pub default_qubits: Option<u8>,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            fidelity_threshold: Some(0.95),
            max_cycles: Some(1000),
            enable_interrupts: Some(true),
            default_qubits: None, // use VM default (2 qubits)
        }
    }
}

impl SimConfig {
    /// Load simulator configuration from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns [`CqamError::IoError`] if the file cannot be read, or
    /// [`CqamError::ConfigError`] if the TOML cannot be parsed.
    pub fn load(path: &str) -> Result<Self, CqamError> {
        let content = fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| CqamError::ConfigError(
            format!("Failed to parse config TOML: {}", e)
        ))
    }
}
