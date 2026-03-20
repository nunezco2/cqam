//! Backend-agnostic quantum execution interface.
//!
//! Defines the `QuantumBackend` trait and associated types (`QRegHandle`,
//! `QOpResult`, `ObserveResult`, `MeasResult`, `KernelParams`) that allow
//! the CQAM VM to execute quantum instructions without depending on a
//! specific simulation implementation.
//!
//! The concrete `SimulationBackend` lives in `cqam-sim`; future backends
//! (QPU, cloud, etc.) will also implement this trait.

use crate::complex::C64;
use crate::error::CqamError;
use crate::instruction::{DistId, KernelId, ObserveMode};

// =============================================================================
// Handle type
// =============================================================================

/// Opaque handle to a quantum register managed by the backend.
///
/// The VM stores these in Q0-Q7 and QMEM slots. The backend maps
/// handles to its internal state representation (DensityMatrix,
/// circuit, cloud job, etc.).
///
/// Handles are Copy + Clone + Eq so they can be stored in arrays
/// and compared. The backend is responsible for reference counting
/// or copy-on-write semantics when a handle is cloned (QStore copies
/// a Q register into QMEM; both the register and the QMEM slot
/// must hold independent copies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QRegHandle(pub u64);

// =============================================================================
// Result types
// =============================================================================

/// Metadata returned by backend operations that produce a new quantum state.
///
/// The VM uses `purity` to drive PSW decoherence flags and fidelity
/// interrupts. `num_qubits` is informational.
#[derive(Debug, Clone)]
pub struct QOpResult {
    /// Purity Tr(rho^2) of the resulting state.
    pub purity: f64,
    /// Number of qubits in the resulting state.
    pub num_qubits: u8,
}

/// Result of a quantum observation (QOBSERVE).
///
/// The VM converts these into `HybridValue` for the hybrid register file.
#[derive(Debug, Clone)]
pub enum ObserveResult {
    /// Full probability distribution: Vec of (basis_state_index, probability)
    /// with near-zero entries filtered out.
    Dist(Vec<(u32, f64)>),
    /// Single probability value (for PROB mode).
    Prob(f64),
    /// Complex amplitude (for AMP mode).
    Amp(C64),
    /// Sampled measurement outcome (for SAMPLE mode).
    Sample(i64),
}

/// Result of a single-qubit measurement (QMEAS).
#[derive(Debug, Clone)]
pub struct MeasResult {
    /// Measurement outcome: 0 or 1.
    pub outcome: u8,
    /// Purity of the post-measurement state.
    pub purity: f64,
}

// =============================================================================
// Kernel parameters
// =============================================================================

/// Classical parameters for kernel application.
///
/// Each variant carries the context values extracted from the instruction's
/// register operands. The backend interprets these based on the KernelId.
#[derive(Debug, Clone)]
pub enum KernelParams {
    /// Integer context from R-file (QKERNEL).
    /// Also carries CMEM data that was pre-read by the VM for kernels
    /// that need memory access (GroverIter multi-target, ControlledU,
    /// DiagonalUnitary, Permutation).
    Int {
        param0: i64,
        param1: i64,
        /// Pre-read CMEM data for kernels that need it.
        /// Empty for kernels that don't access CMEM.
        cmem_data: Vec<i64>,
    },
    /// Float context from F-file (QKERNELF).
    Float {
        param0: f64,
        param1: f64,
    },
    /// Complex context from Z-file (QKERNELZ).
    Complex {
        param0: C64,
        param1: C64,
    },
}

// =============================================================================
// Trait definition
// =============================================================================

/// Backend-agnostic quantum execution interface.
///
/// The VM calls these methods to execute quantum instructions. The backend
/// owns all quantum state internally and exposes it through opaque handles.
///
/// # Lifetime and ownership
///
/// The backend is created once at VM startup and passed by `&mut` to the
/// executor. It is NOT stored inside ExecutionContext -- it is a separate
/// parameter to execute_qop, which avoids borrow conflicts with ctx fields.
///
/// # Error convention
///
/// All fallible operations return `Result<_, CqamError>`. The backend should
/// use CqamError::TypeMismatch for validation failures and
/// CqamError::QubitLimitExceeded for resource limits.
pub trait QuantumBackend: Send {
    // =========================================================================
    // State preparation
    // =========================================================================

    /// Prepare a new quantum register with the given distribution and qubit count.
    ///
    /// Maps to: QPREP, QPREPR, QPREPN
    fn prep(
        &mut self,
        dist: DistId,
        num_qubits: u8,
        force_mixed: bool,
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    /// Prepare a quantum register from explicit amplitude data.
    ///
    /// Maps to: QENCODE
    /// The amplitudes are (re, im) pairs. The backend must normalize.
    fn prep_from_amplitudes(
        &mut self,
        amplitudes: &[C64],
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    /// Prepare a mixed state from a weighted ensemble of statevectors.
    ///
    /// Maps to: QMIXED
    /// Each entry is (weight, amplitudes). The backend constructs
    /// rho = sum_i w_i |psi_i><psi_i|.
    fn prep_mixed(
        &mut self,
        ensemble: &[(f64, &[C64])],
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    // =========================================================================
    // Kernel / gate application
    // =========================================================================

    /// Apply a named kernel to a quantum register.
    ///
    /// Maps to: QKERNEL, QKERNELF, QKERNELZ
    ///
    /// `params` carries the classical context that parameterizes the kernel.
    /// The interpretation depends on the kernel ID (see KernelParams).
    ///
    /// Returns a NEW handle (the source handle remains valid and unchanged --
    /// the VM is responsible for overwriting the destination slot).
    fn apply_kernel(
        &mut self,
        handle: QRegHandle,
        kernel: KernelId,
        params: &KernelParams,
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    /// Apply a single-qubit gate (2x2 unitary) to a specific qubit.
    ///
    /// Maps to: QHADM, QFLIP, QPHASE, QROT (called per-qubit in a mask loop)
    /// The gate is a flat [(f64,f64); 4] in row-major order.
    fn apply_single_gate(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
        gate: &[C64; 4],
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    /// Apply a two-qubit gate (4x4 unitary) to specific qubit pair.
    ///
    /// Maps to: QCNOT, QCZ, QSWAP
    fn apply_two_qubit_gate(
        &mut self,
        handle: QRegHandle,
        qubit_a: u8,
        qubit_b: u8,
        gate: &[C64; 16],
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    /// Apply a full-register custom unitary matrix.
    ///
    /// Maps to: QCUSTOM
    /// The unitary is a dim x dim matrix in row-major order as (re, im) pairs.
    /// The backend should validate unitarity.
    fn apply_custom_unitary(
        &mut self,
        handle: QRegHandle,
        unitary: &[C64],
        dim: usize,
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    // =========================================================================
    // Observation / measurement
    // =========================================================================

    /// Destructively observe a quantum register.
    ///
    /// Maps to: QOBSERVE
    /// After observation, the handle is invalidated (the backend may free
    /// the underlying state). The VM must not use it again.
    fn observe(
        &mut self,
        handle: QRegHandle,
        mode: ObserveMode,
        ctx0: usize,
        ctx1: usize,
    ) -> Result<ObserveResult, CqamError>;

    /// Measure a single qubit (projective measurement with collapse).
    ///
    /// Maps to: QMEAS
    /// Returns the measurement outcome and a new handle for the
    /// post-measurement state. The old handle is invalidated.
    fn measure_qubit(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
    ) -> Result<(QRegHandle, MeasResult), CqamError>;

    // =========================================================================
    // Composite operations
    // =========================================================================

    /// Tensor product of two quantum registers.
    ///
    /// Maps to: QTENSOR
    /// Both source handles are consumed (invalidated).
    /// Returns a new handle for the combined register.
    fn tensor_product(
        &mut self,
        handle_a: QRegHandle,
        handle_b: QRegHandle,
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    /// Partial trace over subsystem B.
    ///
    /// Maps to: QPTRACE
    /// The source handle remains valid (non-destructive).
    /// Returns a new handle for the reduced state of subsystem A.
    fn partial_trace(
        &mut self,
        handle: QRegHandle,
        num_qubits_a: u8,
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    /// Reset a single qubit to |0> (measure + conditional X).
    ///
    /// Maps to: QRESET
    /// The source handle is consumed. Returns a new handle.
    fn reset_qubit(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
    ) -> Result<(QRegHandle, QOpResult), CqamError>;

    /// Prepare a product state on an existing zero-state register.
    ///
    /// Maps to: QPREPS, QPREPSM
    /// Each qubit i is independently rotated from |0> to alpha_i|0> + beta_i|1>.
    /// The amplitudes MUST be pre-normalized by the caller (the VM handles
    /// normalization and zero-checks before calling this method).
    ///
    /// Returns a NEW handle; the source handle is consumed.
    fn prep_product_state(
        &mut self,
        handle: QRegHandle,
        amplitudes: &[(C64, C64)],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let _ = (handle, amplitudes);
        Err(CqamError::QpuUnsupportedOperation {
            operation: "QPREPS/QPREPSM".to_string(),
            detail: "prep_product_state not implemented for this backend".to_string(),
        })
    }

    // =========================================================================
    // Handle lifecycle
    // =========================================================================

    /// Apply teleportation noise to a quantum state after QSTORE/QLOAD transfer.
    /// Default: no-op (perfect teleportation with ideal Bell pairs).
    fn apply_teleportation_noise(&mut self, handle: QRegHandle) -> Result<(), CqamError> {
        let _ = handle;
        Ok(())
    }

    /// Clone a handle's quantum state, producing an independent copy.
    ///
    /// Maps to: QLOAD (clone QMEM slot into Q register),
    ///          QSTORE (clone Q register into QMEM slot)
    ///
    /// The original handle remains valid. The new handle refers to an
    /// independent copy of the quantum state.
    fn clone_state(
        &mut self,
        handle: QRegHandle,
    ) -> Result<QRegHandle, CqamError>;

    /// Release a handle, freeing the associated quantum state.
    ///
    /// Called when QOBSERVE consumes a register, or when a register
    /// is overwritten. The backend may defer cleanup (e.g., arena GC).
    fn release(&mut self, handle: QRegHandle);

    /// Query the number of qubits for a handle.
    fn num_qubits(&self, handle: QRegHandle) -> Result<u8, CqamError>;

    /// Query the Hilbert space dimension for a handle.
    fn dimension(&self, handle: QRegHandle) -> Result<usize, CqamError>;

    // =========================================================================
    // Backend capabilities / limits
    // =========================================================================

    /// Maximum number of qubits this backend supports.
    ///
    /// Used by IQCfg, QPrepN, QTensor for limit checks.
    fn max_qubits(&self) -> u8;

    /// Set the RNG seed for reproducible measurements.
    fn set_rng_seed(&mut self, seed: u64);

    // =========================================================================
    // State inspection (for debugger / reporting)
    // =========================================================================

    /// Query the purity Tr(rho^2) of a quantum register.
    fn purity(&self, handle: QRegHandle) -> Result<f64, CqamError>;

    /// Query whether the state is pure (statevector) or mixed (density matrix).
    fn is_pure(&self, handle: QRegHandle) -> Result<bool, CqamError>;

    /// Get the diagonal probabilities (all basis states) for a quantum register.
    fn diagonal_probabilities(&self, handle: QRegHandle) -> Result<Vec<f64>, CqamError>;

    /// Get a single matrix element rho[i][j] as (re, im).
    fn get_element(&self, handle: QRegHandle, row: usize, col: usize) -> Result<C64, CqamError>;

    /// Get the amplitude of basis state `index` (only meaningful for pure states).
    /// For mixed states, returns (sqrt(p), 0.0).
    fn amplitude(&self, handle: QRegHandle, index: usize) -> Result<C64, CqamError>;
}
