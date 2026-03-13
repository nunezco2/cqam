//! Display and QuantumState trait implementations for `DensityMatrix`.

use super::DensityMatrix;
use cqam_core::quantum_state::QuantumState;

// =============================================================================
// Display
// =============================================================================

impl std::fmt::Display for DensityMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dim = self.dimension();
        writeln!(f, "DensityMatrix({} qubits, dim={})", self.num_qubits, dim)?;

        if dim <= 8 {
            // Print full matrix
            for i in 0..dim {
                write!(f, "  [")?;
                for j in 0..dim {
                    let entry = self.data[i * dim + j];
                    let re = entry.0;
                    let im = entry.1;
                    if j > 0 { write!(f, ", ")?; }
                    if im.abs() < 1e-10 {
                        write!(f, "{:7.4}", re)?;
                    } else {
                        write!(f, "({:.4},{:.4}i)", re, im)?;
                    }
                }
                writeln!(f, "]")?;
            }
        } else {
            // Print only diagonal probabilities
            writeln!(f, "  Diagonal probabilities:")?;
            let probs = self.diagonal_probabilities();
            for (k, p) in probs.iter().enumerate() {
                if *p > 1e-10 {
                    writeln!(f, "    |{:0width$b}> : {:.6}", k, p, width = self.num_qubits as usize)?;
                }
            }
        }
        write!(f, "  Purity: {:.6}, Trace: ({:.6}, {:.6})",
            self.purity(), self.trace().0, self.trace().1)
    }
}

// --- QuantumState trait implementation ---------------------------------------

impl QuantumState for DensityMatrix {
    fn num_qubits(&self) -> u8 {
        DensityMatrix::num_qubits(self)
    }

    fn dimension(&self) -> usize {
        DensityMatrix::dimension(self)
    }

    fn diagonal_probabilities(&self) -> Vec<f64> {
        DensityMatrix::diagonal_probabilities(self)
    }

    fn purity(&self) -> f64 {
        DensityMatrix::purity(self)
    }
}
