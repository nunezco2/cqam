// cqam-sim/src/density_matrix.rs
//
// Phase 2: Density matrix representation for n-qubit quantum states.
//
// The density matrix rho is a 2^n x 2^n Hermitian, positive semi-definite
// matrix with Tr(rho) = 1. Stored as a flat row-major Vec<C64>.

use crate::complex::{self, C64, cx_add, cx_mul, cx_conj, cx_scale, cx_norm_sq};
use rand::Rng;

/// Maximum number of qubits supported by the full density matrix.
pub const MAX_QUBITS: u8 = 12;

/// A density matrix representing an n-qubit quantum state.
///
/// Invariants (maintained by all public constructors and operations):
/// - `data.len() == dim * dim` where `dim = 2^num_qubits`
/// - `Tr(rho) = 1.0` (within floating-point tolerance)
/// - `rho` is Hermitian: `rho[i][j] = conj(rho[j][i])`
/// - `rho` is positive semi-definite
#[derive(Debug, Clone)]
pub struct DensityMatrix {
    num_qubits: u8,
    data: Vec<C64>,
}

// =============================================================================
// Construction
// =============================================================================

impl DensityMatrix {
    /// Create the computational zero state |0...0><0...0|.
    ///
    /// # Panics
    /// Panics if `num_qubits == 0` or `num_qubits > MAX_QUBITS`.
    pub fn new_zero_state(num_qubits: u8) -> Self {
        assert!((1..=MAX_QUBITS).contains(&num_qubits),
            "num_qubits must be 1..={}, got {}", MAX_QUBITS, num_qubits);
        let dim = 1usize << num_qubits;
        let mut data = vec![complex::ZERO; dim * dim];
        data[0] = complex::ONE; // rho[0][0] = 1
        Self { num_qubits, data }
    }

    /// Create the equal-superposition pure state H^n|0><0|(H^n)^dagger.
    ///
    /// Every entry is (1/dim, 0). This is a pure state (rank 1).
    ///
    /// # Panics
    /// Panics if `num_qubits == 0` or `num_qubits > MAX_QUBITS`.
    pub fn new_uniform(num_qubits: u8) -> Self {
        assert!((1..=MAX_QUBITS).contains(&num_qubits),
            "num_qubits must be 1..={}, got {}", MAX_QUBITS, num_qubits);
        let dim = 1usize << num_qubits;
        let val = 1.0 / dim as f64;
        let data = vec![(val, 0.0); dim * dim];
        Self { num_qubits, data }
    }

    /// Create the Bell state |Phi+> = (|00> + |11>) / sqrt(2).
    ///
    /// Produces a 4x4 density matrix (2 qubits).
    pub fn new_bell() -> Self {
        let num_qubits = 2u8;
        let dim = 4usize;
        let mut data = vec![complex::ZERO; dim * dim];
        // rho[0][0] = rho[0][3] = rho[3][0] = rho[3][3] = 0.5
        //   row 0, col 0        row 0, col 3
        data[0]     = (0.5, 0.0);
        data[3]     = (0.5, 0.0);
        //   row 3, col 0        row 3, col 3
        data[3 * dim]     = (0.5, 0.0);
        data[3 * dim + 3] = (0.5, 0.0);
        Self { num_qubits, data }
    }

    /// Create the GHZ state (|0...0> + |1...1>) / sqrt(2) for n qubits.
    ///
    /// # Panics
    /// Panics if `num_qubits < 2` or `num_qubits > MAX_QUBITS`.
    pub fn new_ghz(num_qubits: u8) -> Self {
        assert!((2..=MAX_QUBITS).contains(&num_qubits),
            "num_qubits must be 2..={}, got {}", MAX_QUBITS, num_qubits);
        let dim = 1usize << num_qubits;
        let mut data = vec![complex::ZERO; dim * dim];
        //   row 0, col 0                row 0, col dim-1
        data[0]                     = (0.5, 0.0);
        data[dim - 1]               = (0.5, 0.0);
        //   row dim-1, col 0            row dim-1, col dim-1
        data[(dim - 1) * dim]       = (0.5, 0.0);
        data[(dim - 1) * dim + (dim - 1)] = (0.5, 0.0);
        Self { num_qubits, data }
    }

    /// Construct a density matrix from a pure state vector: rho = |psi><psi|.
    ///
    /// The statevector length must be a power of 2 (2^n for some n >= 1).
    pub fn from_statevector(psi: &[C64]) -> Result<Self, String> {
        let len = psi.len();
        if len == 0 || (len & (len - 1)) != 0 {
            return Err(format!("Statevector length {} is not a power of 2", len));
        }
        let num_qubits = len.trailing_zeros() as u8;
        if num_qubits > MAX_QUBITS {
            return Err(format!("Statevector implies {} qubits, max is {}", num_qubits, MAX_QUBITS));
        }

        // Normalize the statevector
        let norm_sq: f64 = psi.iter().map(|z| cx_norm_sq(*z)).sum();
        let norm = norm_sq.sqrt();
        let psi_norm: Vec<C64> = if (norm - 1.0).abs() > 1e-12 {
            psi.iter().map(|z| cx_scale(1.0 / norm, *z)).collect()
        } else {
            psi.to_vec()
        };

        // rho[i][j] = psi[i] * conj(psi[j])
        let dim = len;
        let mut data = vec![complex::ZERO; dim * dim];
        for i in 0..dim {
            for j in 0..dim {
                data[i * dim + j] = cx_mul(psi_norm[i], cx_conj(psi_norm[j]));
            }
        }

        Ok(Self { num_qubits, data })
    }
}

// =============================================================================
// Element Access
// =============================================================================

impl DensityMatrix {
    /// Get the element at row i, column j.
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> C64 {
        let dim = self.dimension();
        self.data[i * dim + j]
    }

    /// Set the element at row i, column j.
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, val: C64) {
        let dim = self.dimension();
        self.data[i * dim + j] = val;
    }

    /// Number of qubits.
    #[inline]
    pub fn num_qubits(&self) -> u8 {
        self.num_qubits
    }

    /// Hilbert space dimension: 2^num_qubits.
    #[inline]
    pub fn dimension(&self) -> usize {
        1 << self.num_qubits
    }
}

// =============================================================================
// Unitary Application
// =============================================================================

impl DensityMatrix {
    /// Apply a unitary transformation in-place: rho <- U * rho * U^dagger.
    ///
    /// The `unitary` slice must contain dim*dim elements in row-major order.
    ///
    /// # Panics
    /// Panics if `unitary.len() != dim * dim`.
    pub fn apply_unitary(&mut self, unitary: &[C64]) {
        let dim = self.dimension();
        assert_eq!(unitary.len(), dim * dim,
            "Unitary size mismatch: expected {}, got {}", dim * dim, unitary.len());

        // Step 1: temp = U * rho
        let mut temp = vec![complex::ZERO; dim * dim];
        for i in 0..dim {
            for j in 0..dim {
                let mut sum = complex::ZERO;
                for k in 0..dim {
                    sum = cx_add(sum, cx_mul(unitary[i * dim + k], self.data[k * dim + j]));
                }
                temp[i * dim + j] = sum;
            }
        }

        // Step 2: result = temp * U^dagger
        // U^dagger[k][j] = conj(U[j][k])
        for i in 0..dim {
            for j in 0..dim {
                let mut sum = complex::ZERO;
                for k in 0..dim {
                    // U^dagger[k][j] = conj(U[j][k])
                    sum = cx_add(sum, cx_mul(temp[i * dim + k], cx_conj(unitary[j * dim + k])));
                }
                self.data[i * dim + j] = sum;
            }
        }
    }
}

// =============================================================================
// Measurement
// =============================================================================

impl DensityMatrix {
    /// Stochastic measurement of all qubits using the Born rule.
    pub fn measure_all(&self) -> (u16, DensityMatrix) {
        let dim = self.dimension();
        let probs = self.diagonal_probabilities();

        let mut rng = rand::thread_rng();
        let r: f64 = rng.r#gen();

        let mut cumulative = 0.0;
        let mut outcome = dim - 1; // fallback
        for (k, &p) in probs.iter().enumerate() {
            cumulative += p;
            if r < cumulative {
                outcome = k;
                break;
            }
        }

        // Collapsed state: |outcome><outcome|
        let mut collapsed = DensityMatrix::new_zero_state(self.num_qubits);
        // Clear the default zero state and set the outcome
        for entry in collapsed.data.iter_mut() {
            *entry = complex::ZERO;
        }
        collapsed.data[outcome * dim + outcome] = complex::ONE;

        (outcome as u16, collapsed)
    }

    /// Deterministic measurement: return the basis state with the highest
    /// diagonal probability (argmax of Re(rho[k][k])).
    pub fn measure_deterministic(&self) -> u16 {
        let dim = self.dimension();
        let mut max_idx = 0;
        let mut max_prob = f64::NEG_INFINITY;
        for k in 0..dim {
            let p = self.data[k * dim + k].0;
            if p > max_prob {
                max_prob = p;
                max_idx = k;
            }
        }
        max_idx as u16
    }

    /// Extract the diagonal probabilities as a Vec.
    pub fn diagonal_probabilities(&self) -> Vec<f64> {
        let dim = self.dimension();
        (0..dim).map(|k| self.data[k * dim + k].0).collect()
    }
}

// =============================================================================
// Metrics
// =============================================================================

impl DensityMatrix {
    /// Purity: Tr(rho^2).
    ///
    /// Computed as sum_{i,j} |rho[i][j]|^2 (which equals Tr(rho^2) for
    /// Hermitian rho).
    pub fn purity(&self) -> f64 {
        self.data.iter().map(|z| cx_norm_sq(*z)).sum()
    }

    /// Von Neumann entropy of the measurement outcome distribution,
    /// normalized to [0, 1].
    ///
    /// S = -sum_k p_k * log2(p_k) / log2(dim)
    pub fn von_neumann_entropy(&self) -> f64 {
        let dim = self.dimension();
        if dim <= 1 {
            return 0.0;
        }
        let probs = self.diagonal_probabilities();
        let entropy: f64 = probs.iter()
            .filter(|&&p| p > 0.0)
            .map(|&p| -p * p.log2())
            .sum();
        let max_entropy = (dim as f64).log2();
        if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        }
    }

    /// Trace of the density matrix: sum of diagonal elements.
    pub fn trace(&self) -> C64 {
        let dim = self.dimension();
        let mut sum = complex::ZERO;
        for k in 0..dim {
            sum = cx_add(sum, self.data[k * dim + k]);
        }
        sum
    }

    /// Check if the density matrix satisfies basic validity constraints.
    pub fn is_valid(&self, tolerance: f64) -> bool {
        let dim = self.dimension();

        // Check trace ~= 1
        let tr = self.trace();
        if (tr.0 - 1.0).abs() > tolerance || tr.1.abs() > tolerance {
            return false;
        }

        // Check Hermitian: rho[i][j] ~= conj(rho[j][i])
        for i in 0..dim {
            for j in 0..dim {
                let rij = self.data[i * dim + j];
                let rji = self.data[j * dim + i];
                let conj_rji = cx_conj(rji);
                if (rij.0 - conj_rji.0).abs() > tolerance || (rij.1 - conj_rji.1).abs() > tolerance {
                    return false;
                }
            }
        }

        // Check diagonal entries are real and non-negative
        for k in 0..dim {
            let diag = self.data[k * dim + k];
            if diag.1.abs() > tolerance || diag.0 < -tolerance {
                return false;
            }
        }

        true
    }
}

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
                    let (re, im) = self.data[i * dim + j];
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
