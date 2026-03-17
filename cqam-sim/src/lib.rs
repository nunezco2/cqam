//! Quantum simulation backend for the CQAM virtual machine.
//!
//! `cqam-sim` implements the density-matrix formalism for n-qubit systems.
//! A density matrix rho is a 2^n x 2^n Hermitian, trace-1, positive
//! semi-definite matrix stored as a flat row-major `Vec<C64>`. It is the
//! natural representation for both pure states (|psi><psi|) and mixed states
//! arising from decoherence or partial observation.
//!
//! # Key types
//!
//! | Module | Key type | Purpose |
//! |--------|----------|---------|
//! | [`density_matrix`] | [`DensityMatrix`](density_matrix::DensityMatrix) | n-qubit quantum state |
//! | [`kernel`] | [`Kernel`](kernel::Kernel) | Unitary transformation trait |
//! | [`kernels::init`] | `Init` | Uniform superposition |
//! | [`kernels::entangle`] | `Entangle` | CNOT between qubits 0 and 1 |
//! | [`kernels::fourier`] | `Fourier` | Quantum Fourier Transform |
//! | [`kernels::diffuse`] | `Diffuse` | Grover diffusion operator |
//! | [`kernels::grover`] | `GroverIter` | One Grover iteration (oracle + diffusion) |
//! | [`qdist`] | [`QDist`](qdist::QDist), [`Measurable`](qdist::Measurable) | Measurement outcome distributions |
//! | [`complex`] | `C64` | Complex number type (re: f64, im: f64) |
//!
//! # Quantum model
//!
//! All quantum register slots in the CQAM VM hold a `DensityMatrix`. Unitary
//! kernels evolve the state as rho' = U rho U†. Measurement extracts the
//! diagonal probabilities (Born rule) and collapses rho to |outcome><outcome|.
//! The [`QDist`](qdist::QDist) type carries measurement results from `QOBSERVE`
//! back into the hybrid register file.
//!
//! # Usage
//!
//! ```
//! use cqam_sim::density_matrix::DensityMatrix;
//! use cqam_sim::kernels::fourier::Fourier;
//! use cqam_sim::kernel::Kernel;
//!
//! // Create a 2-qubit uniform superposition and apply QFT
//! let state = DensityMatrix::new_uniform(2);
//! let qft = Fourier;
//! let evolved = qft.apply(&state).unwrap();
//! assert!((evolved.purity() - 1.0).abs() < 1e-9);
//! ```

pub mod backend;
pub mod circuit_backend;
pub mod complex;
pub mod constants;
pub mod density_matrix;
pub mod statevector;
pub mod noise;
pub mod quantum_register;
pub mod qdist;
pub mod kernel;
pub mod kernels {
    pub mod init;
    pub mod entangle;
    pub mod fourier;
    pub mod diffuse;
    pub mod grover;
    pub mod rotate;
    pub mod phase;
    pub mod fourier_inv;
    pub mod controlled_u;
    pub mod diagonal;
    pub mod permutation;
}
