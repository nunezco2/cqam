//! Entanglement kernel: applies a CNOT gate between qubit 0 and qubit 1.

use cqam_core::error::CqamError;
use crate::complex::{C64, ZERO, ONE};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;

/// Entanglement kernel: applies CNOT between qubit 0 (control) and qubit 1
/// (target) within the register.
///
/// For n qubits, the unitary is CNOT_{0,1} tensor I_{2^(n-2)}.
/// Requires at least 2 qubits.
pub struct Entangle;

impl Kernel for Entangle {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let n = input.num_qubits();
        if n < 2 {
            return Err(CqamError::TypeMismatch {
                instruction: "QKERNEL/ENTANGLE".to_string(),
                detail: format!(
                    "Entangle requires >= 2 qubits, got {}",
                    n
                ),
            });
        }

        // CNOT 4x4 matrix: |00>->|00>, |01>->|01>, |10>->|11>, |11>->|10>
        let cnot: [C64; 16] = [
            ONE,  ZERO, ZERO, ZERO,
            ZERO, ONE,  ZERO, ZERO,
            ZERO, ZERO, ZERO, ONE,
            ZERO, ZERO, ONE,  ZERO,
        ];

        let mut result = input.clone();
        result.apply_two_qubit_gate(0, 1, &cnot);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, CqamError> {
        let n = input.num_qubits();
        if n < 2 {
            return Err(CqamError::TypeMismatch {
                instruction: "QKERNEL/ENTANGLE".to_string(),
                detail: format!("Entangle requires >= 2 qubits, got {}", n),
            });
        }

        // CNOT is a permutation: swap amplitudes where control qubit (q0) is 1
        // and target qubit (q1) differs. This is O(2^n) with no matrix needed.
        let dim = input.dimension();
        let mut amps = input.amplitudes().to_vec();
        let target_bit = 1 << (n - 2); // qubit 1 bit position

        for basis_state in 0..dim {
            let q0 = (basis_state >> (n - 1)) & 1; // control qubit
            if q0 == 1 && (basis_state & target_bit) == 0 {
                // Swap |...1,0,...> <-> |...1,1,...>
                let partner = basis_state ^ target_bit;
                amps.swap(basis_state, partner);
            }
        }

        Ok(Statevector::from_amplitudes(amps)
            .expect("Entangle apply_sv produced invalid amplitudes"))
    }
}
