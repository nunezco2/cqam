//! Density matrix representation for n-qubit quantum states.
//!
//! The density matrix rho is a 2^n x 2^n Hermitian, positive semi-definite
//! matrix with Tr(rho) = 1. It is stored as a flat row-major `Vec<C64>`,
//! where dim = 2^n. Supports construction of standard states (zero, uniform,
//! Bell, GHZ), unitary evolution, measurement, and fidelity metrics.

mod core;
mod construction;
mod gates;
mod measurement;
mod metrics;
mod display;
mod jacobi;
mod noise;

pub use self::core::DensityMatrix;
