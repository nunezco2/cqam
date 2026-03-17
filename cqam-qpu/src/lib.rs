//! QPU backend infrastructure for CQAM.
//!
//! This crate owns the `QpuBackend` trait, shared QPU infrastructure
//! (`ConnectivityGraph`, `CalibrationData`, `QpuMetrics`, `ConvergenceCriterion`,
//! `RawResults`), and the `MockQpuBackend` (behind the `mock` feature flag
//! in a future phase).
//!
//! Provider-specific backends (e.g., `cqam-qpu-ibm`) implement `QpuBackend`
//! and live in separate crates.

pub mod traits;

pub use traits::{
    QpuBackend, CalibrationData, CircuitQuantumBackend,
    QpuMetrics, ConnectivityGraph, ConvergenceCriterion,
    RawResults, QpuError,
};
