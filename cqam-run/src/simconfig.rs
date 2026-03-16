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
use cqam_core::config::VmConfig;
use cqam_core::error::CqamError;
use cqam_core::parser::ProgramMetadata;

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

    /// Force use of the density-matrix backend for all quantum registers.
    ///
    /// When `true`, quantum registers always use the full 2^n x 2^n density
    /// matrix representation, even when a statevector backend would be more
    /// memory-efficient. Useful for debugging mixed-state behavior or when
    /// decoherence modeling is required.
    /// Default: `false`.
    #[serde(default)]
    pub force_density_matrix: bool,

    /// Default thread count for HFORK parallelism (1-256).
    /// Overrides the `#! threads N` pragma when set.
    /// Default: None (use pragma or 1).
    pub default_threads: Option<u16>,

    /// RNG seed for reproducible quantum measurements.
    /// Default: None (use entropy-based RNG).
    pub rng_seed: Option<u64>,

    /// Number of shots for QPU-realistic sampling mode.
    /// When set, measurement distributions are resampled N times
    /// to produce shot histograms instead of exact distributions.
    /// Default: None (exact simulation).
    #[serde(default)]
    pub shots: Option<u32>,

    /// Noise model name: "none", "superconducting", "trapped-ion",
    /// "neutral-atom", "photonic", "spin".
    #[serde(default)]
    pub noise_model: Option<String>,

    /// Noise simulation method override.
    #[serde(default)]
    pub noise_method: Option<String>,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            fidelity_threshold: Some(0.95),
            max_cycles: Some(1000),
            enable_interrupts: Some(true),
            default_qubits: None, // use VM default (2 qubits)
            force_density_matrix: false,
            default_threads: None,
            rng_seed: None,
            shots: None,
            noise_model: None,
            noise_method: None,
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

    /// Resolve the noise method based on config and qubit count.
    /// Returns None if noise is disabled.
    pub fn resolve_noise_method(&self, num_qubits: u8) -> Option<cqam_sim::noise::NoiseMethod> {
        use cqam_sim::noise::NoiseMethod;
        if self.noise_model.is_none()
            || self.noise_model.as_deref() == Some("none")
        {
            return None;
        }
        match self.noise_method.as_deref() {
            Some("density-matrix") => Some(NoiseMethod::DensityMatrix),
            Some("trajectory") => Some(NoiseMethod::Trajectory),
            Some(other) => {
                eprintln!("warning: unknown --noise-method '{}', auto-selecting. \
                           Valid: density-matrix, trajectory", other);
                self.auto_select_noise_method(num_qubits)
            }
            None => self.auto_select_noise_method(num_qubits),
        }
    }

    /// Auto-select noise method based on qubit count and shots mode.
    fn auto_select_noise_method(&self, num_qubits: u8) -> Option<cqam_sim::noise::NoiseMethod> {
        use cqam_sim::noise::NoiseMethod;
        // Trajectory for large qubit counts to avoid O(4^n) density matrix memory.
        // 16 qubits density matrix = 2^32 entries * 16 bytes ≈ 64 GB.
        if num_qubits > 12 || (self.shots.is_some() && num_qubits > 10) {
            Some(NoiseMethod::Trajectory)
        } else {
            Some(NoiseMethod::DensityMatrix)
        }
    }

    /// Convert this runner config into a [`VmConfig`], applying metadata
    /// overrides with the standard precedence: CLI > pragma > default.
    pub fn to_vm_config(&self, metadata: &ProgramMetadata) -> VmConfig {
        let mut vm = VmConfig::default();

        if let Some(threshold) = self.fidelity_threshold {
            vm.min_purity = threshold;
        }

        // Qubit count precedence: CLI (SimConfig) > pragma > VmConfig default
        if let Some(qubits) = self.default_qubits {
            vm.default_qubits = qubits;
        } else if let Some(pragma_qubits) = metadata.qubits {
            vm.default_qubits = pragma_qubits;
        }

        vm.force_density_matrix = self.force_density_matrix;

        // Thread count precedence: CLI (SimConfig) > pragma > VmConfig default
        if let Some(threads) = self.default_threads {
            vm.default_threads = threads;
        } else if let Some(pragma_threads) = metadata.threads {
            vm.default_threads = pragma_threads;
        }

        vm
    }
}

/// TOML envelope used to extract the `modality` field before full deserialization.
#[derive(serde::Deserialize)]
struct NoiseTomlEnvelope {
    modality: String,
}

/// Construct a noise model from a model name string or a `.toml` file path.
///
/// If `name` ends with `.toml`, the file is read, its `modality` field selects
/// the noise struct, and all remaining fields are deserialized (missing fields
/// fall back to `Default::default()`).  Otherwise the argument is treated as a
/// built-in modality name and the default parameters are used.
pub fn build_noise_model(
    name: &str,
) -> Result<std::sync::Arc<dyn cqam_sim::noise::NoiseModel>, CqamError> {
    use cqam_sim::noise::*;
    use std::sync::Arc;

    if name.ends_with(".toml") {
        let content = fs::read_to_string(name).map_err(|e| {
            CqamError::ConfigError(format!("Cannot read noise TOML '{}': {}", name, e))
        })?;
        let envelope: NoiseTomlEnvelope = toml::from_str(&content).map_err(|e| {
            CqamError::ConfigError(format!("Failed to parse noise TOML '{}': {}", name, e))
        })?;
        match envelope.modality.as_str() {
            "superconducting" => {
                let model: SuperconductingNoise = toml::from_str(&content).map_err(|e| {
                    CqamError::ConfigError(format!("Failed to parse superconducting noise config: {}", e))
                })?;
                Ok(Arc::new(model))
            }
            "trapped-ion" => {
                let model: TrappedIonNoise = toml::from_str(&content).map_err(|e| {
                    CqamError::ConfigError(format!("Failed to parse trapped-ion noise config: {}", e))
                })?;
                Ok(Arc::new(model))
            }
            "neutral-atom" => {
                let model: NeutralAtomNoise = toml::from_str(&content).map_err(|e| {
                    CqamError::ConfigError(format!("Failed to parse neutral-atom noise config: {}", e))
                })?;
                Ok(Arc::new(model))
            }
            "photonic" => {
                let model: PhotonicNoise = toml::from_str(&content).map_err(|e| {
                    CqamError::ConfigError(format!("Failed to parse photonic noise config: {}", e))
                })?;
                Ok(Arc::new(model))
            }
            "spin" => {
                let model: SpinQubitNoise = toml::from_str(&content).map_err(|e| {
                    CqamError::ConfigError(format!("Failed to parse spin noise config: {}", e))
                })?;
                Ok(Arc::new(model))
            }
            other => Err(CqamError::ConfigError(
                format!("unknown modality '{}' in noise TOML '{}'. Valid: superconducting, \
                         trapped-ion, neutral-atom, photonic, spin", other, name))),
        }
    } else {
        match name {
            "superconducting" => Ok(Arc::new(SuperconductingNoise::default())),
            "trapped-ion"     => Ok(Arc::new(TrappedIonNoise::default())),
            "neutral-atom"    => Ok(Arc::new(NeutralAtomNoise::default())),
            "photonic"        => Ok(Arc::new(PhotonicNoise::default())),
            "spin"            => Ok(Arc::new(SpinQubitNoise::default())),
            other => Err(CqamError::ConfigError(
                format!("unknown noise model: '{}'. Valid: superconducting, \
                         trapped-ion, neutral-atom, photonic, spin", other))),
        }
    }
}
