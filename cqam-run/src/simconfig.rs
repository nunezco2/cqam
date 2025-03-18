use std::fs;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SimConfig {
    pub fidelity_threshold: Option<f64>,
    pub max_cycles: Option<usize>,
    pub enable_interrupts: Option<bool>,
}

impl SimConfig {
    pub fn load(path: &str) -> Self {
        let content = fs::read_to_string(path)
            .expect("Failed to read config file.");
        toml::from_str(&content).expect("Failed to parse config TOML.")
    }

    pub fn default() -> Self {
        SimConfig {
            fidelity_threshold: Some(0.95),
            max_cycles: Some(1000),
            enable_interrupts: Some(true),
        }
    }
}
