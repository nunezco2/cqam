//! Quantum simulation backend for the CQAM virtual machine.
//!
//! Provides the `DensityMatrix` quantum state representation, the `Kernel`
//! trait for unitary transformations, concrete kernel implementations
//! (Init, Entangle, Fourier, Diffuse, GroverIter), and the `QDist` probability
//! distribution type. Complex arithmetic is supplied by `complex::C64`.

pub mod complex;
pub mod density_matrix;
pub mod qdist;
pub mod kernel;
pub mod kernels {
    pub mod init;
    pub mod entangle;
    pub mod fourier;
    pub mod diffuse;
    pub mod grover;
}
