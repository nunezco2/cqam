// cqam-sim/src/kernels/entangle.rs
//
// Phase 2: Entanglement kernel using CNOT gate on DensityMatrix.

use crate::complex;
use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;

/// Entanglement kernel: applies CNOT between qubit 0 (control) and qubit 1
/// (target) within the register.
///
/// For n qubits, the unitary is CNOT_{0,1} tensor I_{2^(n-2)}.
/// Requires at least 2 qubits.
pub struct Entangle;

impl Kernel for Entangle {
    fn apply(&self, input: &DensityMatrix) -> DensityMatrix {
        let n = input.num_qubits();
        assert!(n >= 2, "Entangle kernel requires at least 2 qubits");

        let dim = input.dimension();
        let mut unitary = vec![complex::ZERO; dim * dim];

        // CNOT on qubits 0,1 (big-endian ordering):
        // qubit 0 = control (MSB), qubit 1 = target
        // If control qubit is 1: flip target qubit
        for basis_state in 0..dim {
            let q0 = (basis_state >> (n - 1)) & 1; // control (MSB)
            let output_state = if q0 == 1 {
                basis_state ^ (1 << (n - 2)) // flip qubit 1
            } else {
                basis_state // identity
            };
            unitary[output_state * dim + basis_state] = complex::ONE;
        }

        let mut result = input.clone();
        result.apply_unitary(&unitary);
        result
    }
}
