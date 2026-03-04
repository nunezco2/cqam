use std::fs;
use serde::Deserialize;
use cqam_core::error::CqamError;

#[derive(Debug, Deserialize)]
pub struct SimConfig {
    pub fidelity_threshold: Option<f64>,
    pub max_cycles: Option<usize>,
    pub enable_interrupts: Option<bool>,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            fidelity_threshold: Some(0.95),
            max_cycles: Some(1000),
            enable_interrupts: Some(true),
        }
    }
}

impl SimConfig {
    pub fn load(path: &str) -> Result<Self, CqamError> {
        let content = fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| CqamError::ConfigError(
            format!("Failed to parse config TOML: {}", e)
        ))
    }
}
