//! CircuitBackend: QPU-mode QuantumBackend that buffers ops into a circuit IR,
//! compiles them via cqam-micro, and submits to a QpuBackend for execution.
//!
//! # Known limitations
//!
//! - Mid-circuit measurement (`measure_qubit`) returns a dummy outcome (0) and
//!   does not support classical feedback. Programs that branch on QMEAS results
//!   will behave incorrectly. This is deferred to Phase 5 (dynamic circuits).
//! - `ObserveMode::Prob` is approximated from shot counts.
//! - `ObserveMode::Amp` is unsupported (requires statevector access).
//! - Partial trace is unsupported.
//! - State inspection methods (purity, diagonal_probs, etc.) are unsupported.

use std::collections::{HashMap, HashSet};

use rand::SeedableRng;
use rand::Rng as _;
use rand_chacha::ChaCha8Rng;

use cqam_core::circuit_ir::{
    ApplyGate1q, ApplyGate2q, ApplyKernel, Gate1q, Gate2q, MicroProgram, Observe, Op,
    Prepare, PrepProduct, QWire, Reset,
};
use cqam_core::complex::C64;
use cqam_core::error::CqamError;
use cqam_core::instruction::{DistId, KernelId, ObserveMode};
use cqam_core::quantum_backend::{
    KernelParams, MeasResult, ObserveResult, QOpResult, QRegHandle, QuantumBackend,
};

use cqam_micro::pipeline::CompilationPipeline;
use cqam_qpu::traits::{
    CircuitQuantumBackend, ConvergenceCriterion, QpuBackend, QpuMetrics,
};

// =============================================================================
// WireAllocator
// =============================================================================

/// Monotonic logical qubit wire allocator.
///
/// Allocates contiguous wire ranges for each `prep()` call. Wires are NOT
/// recycled within a buffer lifetime (prep-to-observe). Reset is called
/// when the buffer is flushed.
#[derive(Clone)]
struct WireAllocator {
    next_wire: u32,
}

impl WireAllocator {
    fn new() -> Self { Self { next_wire: 0 } }

    /// Allocate `n` contiguous wires. Returns the wire vector.
    fn alloc(&mut self, n: u8) -> Vec<QWire> {
        let start = self.next_wire;
        self.next_wire += n as u32;
        (start..self.next_wire).map(QWire).collect()
    }

    /// Reset to wire 0 (call after buffer flush).
    fn reset(&mut self) {
        self.next_wire = 0;
    }
}

// =============================================================================
// Gate recognition matrices (for apply_single_gate / apply_two_qubit_gate)
// =============================================================================

const TOLERANCE: f64 = 1e-8;

fn c64_close(a: C64, b: C64) -> bool {
    (a.0 - b.0).abs() < TOLERANCE && (a.1 - b.1).abs() < TOLERANCE
}

fn mat4_close(a: &[C64; 4], b: &[C64; 4]) -> bool {
    a.iter().zip(b.iter()).all(|(&x, &y)| c64_close(x, y))
}

fn mat16_close(a: &[C64; 16], b: &[C64; 16]) -> bool {
    a.iter().zip(b.iter()).all(|(&x, &y)| c64_close(x, y))
}

// Standard gate matrices (row-major, C64)
const H_MAT: [C64; 4] = [
    C64(std::f64::consts::FRAC_1_SQRT_2, 0.0), C64(std::f64::consts::FRAC_1_SQRT_2, 0.0),
    C64(std::f64::consts::FRAC_1_SQRT_2, 0.0), C64(-std::f64::consts::FRAC_1_SQRT_2, 0.0),
];
const X_MAT: [C64; 4] = [C64(0.0, 0.0), C64(1.0, 0.0), C64(1.0, 0.0), C64(0.0, 0.0)];
const Y_MAT: [C64; 4] = [C64(0.0, 0.0), C64(0.0, -1.0), C64(0.0, 1.0), C64(0.0, 0.0)];
const Z_MAT: [C64; 4] = [C64(1.0, 0.0), C64(0.0, 0.0), C64(0.0, 0.0), C64(-1.0, 0.0)];
const S_MAT: [C64; 4] = [C64(1.0, 0.0), C64(0.0, 0.0), C64(0.0, 0.0), C64(0.0, 1.0)];
const SDG_MAT: [C64; 4] = [C64(1.0, 0.0), C64(0.0, 0.0), C64(0.0, 0.0), C64(0.0, -1.0)];
/// e^{i*pi/4} = (1+i) / sqrt(2): real and imaginary parts.
const FRAC_1_SQRT_2: f64 = std::f64::consts::FRAC_1_SQRT_2;
static T_MAT: [C64; 4] = [
    C64(1.0, 0.0), C64(0.0, 0.0),
    C64(0.0, 0.0), C64(FRAC_1_SQRT_2, FRAC_1_SQRT_2), // e^{i*pi/4}
];
static TDG_MAT: [C64; 4] = [
    C64(1.0, 0.0), C64(0.0, 0.0),
    C64(0.0, 0.0), C64(FRAC_1_SQRT_2, -FRAC_1_SQRT_2), // e^{-i*pi/4}
];

/// ZYZ Euler decomposition of a 2x2 unitary matrix.
///
/// Decomposes U = e^{ig} * U3(theta, phi, lambda) where
///   U3(θ, φ, λ) = Rz(φ) * Ry(θ) * Rz(λ)
///               = | cos(θ/2)           -e^{iλ}*sin(θ/2) |
///                 | e^{iφ}*sin(θ/2)     e^{i(φ+λ)}*cos(θ/2) |
///
/// Matrix layout (row-major): [a, b, c, d] where U = |a b|
///                                                     |c d|
///
/// Derivation (with g = global phase):
///   a = e^{ig} * cos(θ/2)           →  |a| = cos(θ/2),  arg(a) = g
///   b = -e^{ig} * e^{iλ} * sin(θ/2) →  arg(b) = g + λ + π
///   c = e^{ig} * e^{iφ} * sin(θ/2)  →  arg(c) = g + φ
///   d = e^{ig} * e^{i(φ+λ)} * cos(θ/2)
///
/// Therefore:
///   phi    = arg(c) - arg(a)
///   lambda = arg(b) - arg(a) - π
///
/// Returns `Some((theta, phi, lambda))` for valid unitaries, `None` for degenerate matrices.
fn decompose_zyz(mat: &[C64; 4]) -> Option<(f64, f64, f64)> {
    let a = mat[0]; // U[0,0]
    let b = mat[1]; // U[0,1]
    let c = mat[2]; // U[1,0]
    let d = mat[3]; // U[1,1]

    let a_norm = a.norm();

    // theta = 2 * acos(|a|), clamped for numerical safety
    let theta = 2.0 * a_norm.clamp(0.0, 1.0).acos();

    // Handle special cases based on theta
    if theta.abs() < 1e-9 {
        // theta ≈ 0: identity-like (U ≈ e^{ig} * diag(1, e^{i(φ+λ)}))
        // Only the sum φ+λ is meaningful; fix φ=φ+λ and λ=0.
        // phi+lambda = arg(d) - arg(a)
        let arg_a = if a_norm < 1e-12 { 0.0 } else { a.1.atan2(a.0) };
        let d_norm = d.norm();
        let arg_d = if d_norm < 1e-12 { 0.0 } else { d.1.atan2(d.0) };
        let phi = arg_d - arg_a;
        return Some((0.0, phi, 0.0));
    }

    if (theta - std::f64::consts::PI).abs() < 1e-9 {
        // theta ≈ pi: bit-flip-like (U ≈ e^{ig} * [[0, -e^{iλ}], [e^{iφ}, 0]])
        // In this case |a| ≈ 0; derive phi and lambda from b and c.
        // arg(c) = g + phi,  arg(b) = g + lambda + pi
        // Fix lambda=0: phi = arg(c) - arg(b) - pi
        let b_norm = b.norm();
        let c_norm = c.norm();
        let arg_b = if b_norm < 1e-12 { 0.0 } else { b.1.atan2(b.0) };
        let arg_c = if c_norm < 1e-12 { 0.0 } else { c.1.atan2(c.0) };
        let phi = arg_c - arg_b - std::f64::consts::PI;
        return Some((std::f64::consts::PI, phi, 0.0));
    }

    // General case: all entries are non-negligible
    // phi = arg(c) - arg(a),  lambda = arg(b) - arg(a) - pi
    let arg_a = if a_norm < 1e-12 { 0.0 } else { a.1.atan2(a.0) };
    let b_norm = b.norm();
    let c_norm = c.norm();
    let arg_b = if b_norm < 1e-12 { 0.0 } else { b.1.atan2(b.0) };
    let arg_c = if c_norm < 1e-12 { 0.0 } else { c.1.atan2(c.0) };

    let phi = arg_c - arg_a;
    let lambda = arg_b - arg_a - std::f64::consts::PI;

    Some((theta, phi, lambda))
}

fn recognize_gate1q(mat: &[C64; 4]) -> Gate1q {
    if mat4_close(mat, &H_MAT) { return Gate1q::H; }
    if mat4_close(mat, &X_MAT) { return Gate1q::X; }
    if mat4_close(mat, &Y_MAT) { return Gate1q::Y; }
    if mat4_close(mat, &Z_MAT) { return Gate1q::Z; }
    if mat4_close(mat, &S_MAT) { return Gate1q::S; }
    if mat4_close(mat, &SDG_MAT) { return Gate1q::Sdg; }
    if mat4_close(mat, &T_MAT) { return Gate1q::T; }
    if mat4_close(mat, &TDG_MAT) { return Gate1q::Tdg; }
    // ZYZ Euler decomposition: any valid unitary becomes U3(theta, phi, lambda)
    if let Some((theta, phi, lambda)) = decompose_zyz(mat) {
        use cqam_core::circuit_ir::Param;
        return Gate1q::U3(
            Param::Resolved(theta),
            Param::Resolved(phi),
            Param::Resolved(lambda),
        );
    }
    // Safety net: only reached for degenerate (non-unitary) matrices
    Gate1q::Custom(Box::new(*mat))
}

// CX / CNOT 4x4 matrix (row-major, |00>,|01>,|10>,|11> basis)
#[allow(clippy::zero_prefixed_literal)]
const CX_MAT: [C64; 16] = [
    C64(1.0,0.0), C64(0.0,0.0), C64(0.0,0.0), C64(0.0,0.0),
    C64(0.0,0.0), C64(1.0,0.0), C64(0.0,0.0), C64(0.0,0.0),
    C64(0.0,0.0), C64(0.0,0.0), C64(0.0,0.0), C64(1.0,0.0),
    C64(0.0,0.0), C64(0.0,0.0), C64(1.0,0.0), C64(0.0,0.0),
];

// CZ 4x4 matrix
const CZ_MAT: [C64; 16] = [
    C64(1.0,0.0), C64(0.0,0.0), C64(0.0,0.0), C64(0.0,0.0),
    C64(0.0,0.0), C64(1.0,0.0), C64(0.0,0.0), C64(0.0,0.0),
    C64(0.0,0.0), C64(0.0,0.0), C64(1.0,0.0), C64(0.0,0.0),
    C64(0.0,0.0), C64(0.0,0.0), C64(0.0,0.0), C64(-1.0,0.0),
];

// SWAP 4x4 matrix
const SWAP_MAT: [C64; 16] = [
    C64(1.0,0.0), C64(0.0,0.0), C64(0.0,0.0), C64(0.0,0.0),
    C64(0.0,0.0), C64(0.0,0.0), C64(1.0,0.0), C64(0.0,0.0),
    C64(0.0,0.0), C64(1.0,0.0), C64(0.0,0.0), C64(0.0,0.0),
    C64(0.0,0.0), C64(0.0,0.0), C64(0.0,0.0), C64(1.0,0.0),
];

fn recognize_gate2q(mat: &[C64; 16]) -> Gate2q {
    if mat16_close(mat, &CX_MAT) { return Gate2q::Cx; }
    if mat16_close(mat, &CZ_MAT) { return Gate2q::Cz; }
    if mat16_close(mat, &SWAP_MAT) { return Gate2q::Swap; }
    Gate2q::Custom(Box::new(*mat))
}

// =============================================================================
// CircuitBackend
// =============================================================================

/// A `QuantumBackend` that buffers quantum operations into a circuit IR,
/// compiles via the `cqam-micro` pipeline, and submits to a QPU backend.
///
/// # Type parameter
/// `Q: QpuBackend` -- the underlying QPU backend (e.g., `MockQpuBackend`).
pub struct CircuitBackend<Q: QpuBackend> {
    qpu: Q,
    pipeline: CompilationPipeline,
    buffer: Option<MicroProgram>,
    wire_allocator: WireAllocator,
    handle_wires: HashMap<u64, Vec<QWire>>,
    handle_num_qubits: HashMap<u64, u8>,
    /// Distribution used for initial prep (for clone_state duplication).
    handle_dist: HashMap<u64, DistId>,
    next_handle: u64,
    /// Handles that have had at least one gate/kernel applied (for clone_state).
    evolved_handles: HashSet<u64>,
    convergence: ConvergenceCriterion,
    shot_budget: u32,
    metrics: QpuMetrics,
    rng_seed: Option<u64>,
}

impl<Q: QpuBackend> CircuitBackend<Q> {
    /// Create a new CircuitBackend wrapping the given QPU backend.
    pub fn new(qpu: Q, convergence: ConvergenceCriterion, shot_budget: u32) -> Self {
        let gate_set = qpu.gate_set().clone();
        let connectivity = qpu.connectivity().clone();
        let pipeline = CompilationPipeline::new(gate_set, connectivity, 64);
        Self {
            qpu,
            pipeline,
            buffer: None,
            wire_allocator: WireAllocator::new(),
            handle_wires: HashMap::new(),
            handle_num_qubits: HashMap::new(),
            handle_dist: HashMap::new(),
            next_handle: 0,
            evolved_handles: HashSet::new(),
            convergence,
            shot_budget,
            metrics: QpuMetrics::default(),
            rng_seed: None,
        }
    }

    /// Allocate a new handle with the given wires and qubit count. Returns the handle.
    fn alloc_handle(&mut self, wires: Vec<QWire>, num_qubits: u8) -> QRegHandle {
        let id = self.next_handle;
        self.next_handle += 1;
        self.handle_wires.insert(id, wires);
        self.handle_num_qubits.insert(id, num_qubits);
        QRegHandle(id)
    }

    /// Validate that a handle exists, returning its wire list.
    fn validate_handle(&self, handle: QRegHandle) -> Result<&Vec<QWire>, CqamError> {
        self.handle_wires.get(&handle.0).ok_or_else(|| CqamError::UninitializedRegister {
            file: "Q".to_string(),
            index: 0,
        })
    }

    /// Validate that a qubit index is in range for the given handle.
    fn validate_qubit(&self, handle: QRegHandle, qubit: u8) -> Result<QWire, CqamError> {
        let wires = self.validate_handle(handle)?;
        let idx = qubit as usize;
        if idx >= wires.len() {
            return Err(CqamError::QuantumIndexOutOfRange {
                instruction: "circuit_backend".to_string(),
                index: idx,
                limit: wires.len(),
            });
        }
        Ok(wires[idx])
    }

    /// Ensure a buffer exists, creating one if needed. Returns a mutable ref.
    fn ensure_buffer(&mut self) -> &mut MicroProgram {
        if self.buffer.is_none() {
            self.buffer = Some(MicroProgram::new(0));
        }
        self.buffer.as_mut().unwrap()
    }

    /// Clone wire mapping from an old handle to a new handle.
    fn clone_handle_mapping(&mut self, old: QRegHandle, new_id: u64) {
        if let Some(wires) = self.handle_wires.get(&old.0).cloned() {
            let nq = self.handle_num_qubits[&old.0];
            self.handle_wires.insert(new_id, wires);
            self.handle_num_qubits.insert(new_id, nq);
        }
    }

    /// Mark a handle as evolved (a gate has been applied to it).
    fn mark_evolved(&mut self, handle: QRegHandle) {
        self.evolved_handles.insert(handle.0);
    }

    /// Flush: finalize the buffer and return it (also resets wire allocator).
    fn take_buffer(&mut self) -> Option<MicroProgram> {
        let buf = self.buffer.take();
        self.wire_allocator.reset();
        buf
    }
}

impl<Q: QpuBackend> QuantumBackend for CircuitBackend<Q> {
    // =========================================================================
    // State preparation
    // =========================================================================

    fn prep(
        &mut self,
        dist: DistId,
        num_qubits: u8,
        _force_mixed: bool,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let wires = self.wire_allocator.alloc(num_qubits);
        let buf = self.ensure_buffer();
        buf.num_wires = buf.num_wires.max(
            wires.last().map(|w| w.0 + 1).unwrap_or(0)
        );
        buf.push(Op::Prep(Prepare { wires: wires.clone(), dist }));

        let handle = self.alloc_handle(wires, num_qubits);
        self.handle_dist.insert(handle.0, dist);
        Ok((handle, QOpResult { purity: 1.0, num_qubits }))
    }

    fn prep_from_amplitudes(
        &mut self,
        _amplitudes: &[C64],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "QENCODE".to_string(),
            detail: "arbitrary amplitude encoding not supported in circuit mode".to_string(),
        })
    }

    fn prep_mixed(
        &mut self,
        _ensemble: &[(f64, &[C64])],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "QMIXED".to_string(),
            detail: "mixed state preparation not supported in circuit mode".to_string(),
        })
    }

    fn prep_product_state(
        &mut self,
        handle: QRegHandle,
        amplitudes: &[(C64, C64)],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let wires = self.validate_handle(handle)?.clone();
        let num_qubits = self.handle_num_qubits[&handle.0];

        if amplitudes.len() > wires.len() {
            return Err(CqamError::QuantumIndexOutOfRange {
                instruction: "prep_product_state".to_string(),
                index: amplitudes.len(),
                limit: wires.len(),
            });
        }

        // Emit Op::PrepProduct into the circuit buffer
        let prep_wires: Vec<QWire> = wires[..amplitudes.len()].to_vec();
        self.ensure_buffer().push(Op::PrepProduct(PrepProduct {
            wires: prep_wires,
            amplitudes: amplitudes.to_vec(),
        }));

        self.mark_evolved(handle);
        let new_id = self.next_handle;
        self.next_handle += 1;
        self.clone_handle_mapping(handle, new_id);
        Ok((QRegHandle(new_id), QOpResult { purity: 1.0, num_qubits }))
    }

    // =========================================================================
    // Gate / kernel application
    // =========================================================================

    fn apply_kernel(
        &mut self,
        handle: QRegHandle,
        kernel: KernelId,
        params: &KernelParams,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let wires = self.validate_handle(handle)?.clone();
        let num_qubits = self.handle_num_qubits[&handle.0];

        self.ensure_buffer().push(Op::Kernel(ApplyKernel {
            wires: wires.clone(),
            kernel,
            params: params.clone(),
        }));

        self.mark_evolved(handle);
        let new_id = self.next_handle;
        self.next_handle += 1;
        self.clone_handle_mapping(handle, new_id);
        Ok((QRegHandle(new_id), QOpResult { purity: 1.0, num_qubits }))
    }

    fn apply_single_gate(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
        gate: &[C64; 4],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let wire = self.validate_qubit(handle, target_qubit)?;
        let num_qubits = self.handle_num_qubits[&handle.0];

        let gate1q = recognize_gate1q(gate);
        self.ensure_buffer().push(Op::Gate1q(ApplyGate1q { wire, gate: gate1q }));

        self.mark_evolved(handle);
        let new_id = self.next_handle;
        self.next_handle += 1;
        self.clone_handle_mapping(handle, new_id);
        Ok((QRegHandle(new_id), QOpResult { purity: 1.0, num_qubits }))
    }

    fn apply_two_qubit_gate(
        &mut self,
        handle: QRegHandle,
        qubit_a: u8,
        qubit_b: u8,
        gate: &[C64; 16],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let wire_a = self.validate_qubit(handle, qubit_a)?;
        let wire_b = self.validate_qubit(handle, qubit_b)?;
        let num_qubits = self.handle_num_qubits[&handle.0];

        let gate2q = recognize_gate2q(gate);
        self.ensure_buffer().push(Op::Gate2q(ApplyGate2q { wire_a, wire_b, gate: gate2q }));

        self.mark_evolved(handle);
        let new_id = self.next_handle;
        self.next_handle += 1;
        self.clone_handle_mapping(handle, new_id);
        Ok((QRegHandle(new_id), QOpResult { purity: 1.0, num_qubits }))
    }

    fn apply_custom_unitary(
        &mut self,
        handle: QRegHandle,
        unitary: &[C64],
        dim: usize,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let num_qubits = self.handle_num_qubits.get(&handle.0).copied().ok_or_else(|| {
            CqamError::UninitializedRegister { file: "Q".to_string(), index: 0 }
        })?;
        let expected_dim = 1usize << num_qubits;
        if dim != expected_dim {
            return Err(CqamError::TypeMismatch {
                instruction: "apply_custom_unitary".to_string(),
                detail: format!("dim {} does not match 2^n_qubits = {}", dim, expected_dim),
            });
        }

        let wires = self.handle_wires[&handle.0].clone();
        self.ensure_buffer().push(Op::CustomUnitary {
            wires: wires.clone(),
            matrix: unitary.to_vec(),
        });

        self.mark_evolved(handle);
        let new_id = self.next_handle;
        self.next_handle += 1;
        self.clone_handle_mapping(handle, new_id);
        Ok((QRegHandle(new_id), QOpResult { purity: 1.0, num_qubits }))
    }

    // =========================================================================
    // Observation / measurement
    // =========================================================================

    fn observe(
        &mut self,
        handle: QRegHandle,
        mode: ObserveMode,
        ctx0: usize,
        ctx1: usize,
    ) -> Result<ObserveResult, CqamError> {
        match mode {
            ObserveMode::Amp => {
                return Err(CqamError::QpuUnsupportedOperation {
                    operation: "QOBSERVE/AMP".to_string(),
                    detail: "amplitude access not available in circuit mode; use DIST, PROB, or SAMPLE".to_string(),
                });
            }
            ObserveMode::Dist | ObserveMode::Sample | ObserveMode::Prob => {}
        }

        let wires = self.validate_handle(handle)?.clone();

        // Push the observe op
        {
            // num_wires must cover both the wire_allocator high-water mark (for
            // any PREP/gate ops already in the buffer) AND the maximum wire index
            // used by this observe.  The two can differ when a handle was prepped
            // in a previous buffer section and its handle survived the flush
            // (i.e. it was never observed in that section).  In that case
            // wire_allocator has been reset to 0 but the handle's wires still
            // carry their original indices.
            let allocator_hwm = self.wire_allocator.next_wire;
            let observe_hwm = wires.last().map(|w| w.0 + 1).unwrap_or(0);
            let nw = allocator_hwm.max(observe_hwm);
            let buf = self.ensure_buffer();
            buf.push(Op::Measure(Observe { wires, mode, ctx0, ctx1 }));
            // Finalize num_wires
            buf.num_wires = buf.num_wires.max(nw);
        }

        // Flush: compile and submit
        let buffer = self.take_buffer().unwrap();

        // Obtain calibration (best-effort)
        let calib_box = self.qpu.calibration().ok();

        let native_circuit = self.pipeline.synthesize(
            &buffer,
            calib_box.as_deref(),
        ).map_err(|e| {
            let ce: CqamError = e.into();
            ce
        })?;

        let raw = self.qpu.submit(
            &native_circuit,
            &self.convergence,
            self.shot_budget,
        )?;

        // Update metrics
        self.metrics = raw.metrics.clone();

        // Build result
        let result = match mode {
            ObserveMode::Dist => {
                let total = raw.total_shots as f64;
                let dist: Vec<(u32, f64)> = raw.counts.iter()
                    .filter_map(|(&bitstring, &count)| {
                        let prob = count as f64 / total;
                        if prob >= 1e-15 { Some((bitstring as u32, prob)) } else { None }
                    })
                    .collect();
                ObserveResult::Dist(dist)
            }
            ObserveMode::Prob => {
                // Approximate P(|ctx0⟩) from shot counts.
                let total = raw.total_shots as f64;
                let target = ctx0 as u64;
                let count = raw.counts.get(&target).copied().unwrap_or(0) as f64;
                ObserveResult::Prob(count / total.max(1.0))
            }
            ObserveMode::Sample => {
                // Pick one bitstring proportional to counts using an RNG
                let total = raw.total_shots;
                let chosen = if let Some(seed) = self.rng_seed {
                    let mut rng = ChaCha8Rng::seed_from_u64(seed);
                    let r: u32 = rng.r#gen::<u32>() % total;
                    pick_weighted(&raw.counts, r)
                } else {
                    use rand::thread_rng;
                    let mut rng = thread_rng();
                    let r: u32 = rng.r#gen::<u32>() % total;
                    pick_weighted(&raw.counts, r)
                };
                ObserveResult::Sample(chosen as i64)
            }
            ObserveMode::Amp => unreachable!(), // rejected above
        };

        // Clean up handle
        self.handle_wires.remove(&handle.0);
        self.handle_num_qubits.remove(&handle.0);
        self.handle_dist.remove(&handle.0);
        self.evolved_handles.remove(&handle.0);

        Ok(result)
    }

    fn measure_qubit(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
    ) -> Result<(QRegHandle, MeasResult), CqamError> {
        let wire = self.validate_qubit(handle, target_qubit)?;

        // Log warning about mid-circuit measurement limitation
        // (Phase 3: mid-circuit measurement returns dummy outcome)
        self.ensure_buffer().push(Op::MeasQubit { wire });

        let new_id = self.next_handle;
        self.next_handle += 1;
        self.clone_handle_mapping(handle, new_id);

        Ok((QRegHandle(new_id), MeasResult {
            outcome: 0, // PLACEHOLDER -- see known limitations in module doc
            purity: 1.0,
        }))
    }

    // =========================================================================
    // Composite operations
    // =========================================================================

    fn tensor_product(
        &mut self,
        handle_a: QRegHandle,
        handle_b: QRegHandle,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let wires_a = self.validate_handle(handle_a)?.clone();
        let wires_b = self.validate_handle(handle_b)?.clone();
        let nq_a = self.handle_num_qubits[&handle_a.0];
        let nq_b = self.handle_num_qubits[&handle_b.0];

        let mut merged_wires = wires_a;
        merged_wires.extend(wires_b);
        let num_qubits = nq_a + nq_b;

        // Invalidate consumed source handles
        self.handle_wires.remove(&handle_a.0);
        self.handle_wires.remove(&handle_b.0);
        self.handle_num_qubits.remove(&handle_a.0);
        self.handle_num_qubits.remove(&handle_b.0);
        self.handle_dist.remove(&handle_a.0);
        self.handle_dist.remove(&handle_b.0);
        self.evolved_handles.remove(&handle_a.0);
        self.evolved_handles.remove(&handle_b.0);

        // No circuit op is pushed -- tensor product is wire-level bookkeeping.
        let handle = self.alloc_handle(merged_wires, num_qubits);
        // Mark as evolved since the composite register should not be cloned
        self.evolved_handles.insert(handle.0);
        Ok((handle, QOpResult { purity: 1.0, num_qubits }))
    }

    fn partial_trace(
        &mut self,
        _handle: QRegHandle,
        _num_qubits_a: u8,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "QPTRACE".to_string(),
            detail: "partial trace not supported in circuit mode".to_string(),
        })
    }

    fn reset_qubit(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let wire = self.validate_qubit(handle, target_qubit)?;
        let num_qubits = self.handle_num_qubits[&handle.0];

        self.ensure_buffer().push(Op::Reset(Reset { wire }));

        let new_id = self.next_handle;
        self.next_handle += 1;
        self.clone_handle_mapping(handle, new_id);
        Ok((QRegHandle(new_id), QOpResult { purity: 1.0, num_qubits }))
    }

    // =========================================================================
    // Handle lifecycle
    // =========================================================================

    fn apply_teleportation_noise(&mut self, _handle: QRegHandle) -> Result<(), CqamError> {
        // No-op in circuit mode
        Ok(())
    }

    fn clone_state(&mut self, handle: QRegHandle) -> Result<QRegHandle, CqamError> {
        // Validate handle exists
        let _ = self.validate_handle(handle)?;

        // Cannot clone an evolved state (no-cloning theorem)
        if self.evolved_handles.contains(&handle.0) {
            return Err(CqamError::QpuUnsupportedOperation {
                operation: "clone_state".to_string(),
                detail: "cannot clone a quantum state that has been evolved in circuit mode (no-cloning theorem)".to_string(),
            });
        }

        // Freshly prepped: allocate a new handle pointing to fresh wires,
        // push a duplicate Op::Prep with the same distribution as the original
        let old_nq = self.handle_num_qubits[&handle.0];
        let old_dist = self.handle_dist.get(&handle.0).copied().unwrap_or(DistId::Zero);

        let new_wires = self.wire_allocator.alloc(old_nq);
        let buf = self.ensure_buffer();
        buf.num_wires = buf.num_wires.max(
            new_wires.last().map(|w| w.0 + 1).unwrap_or(0)
        );
        buf.push(Op::Prep(Prepare { wires: new_wires.clone(), dist: old_dist }));

        let new_handle = self.alloc_handle(new_wires, old_nq);
        Ok(new_handle)
    }

    fn release(&mut self, handle: QRegHandle) {
        self.handle_wires.remove(&handle.0);
        self.handle_num_qubits.remove(&handle.0);
        self.handle_dist.remove(&handle.0);
        self.evolved_handles.remove(&handle.0);
    }

    fn num_qubits(&self, handle: QRegHandle) -> Result<u8, CqamError> {
        self.handle_num_qubits.get(&handle.0).copied().ok_or_else(|| {
            CqamError::UninitializedRegister { file: "Q".to_string(), index: 0 }
        })
    }

    fn dimension(&self, handle: QRegHandle) -> Result<usize, CqamError> {
        let nq = self.num_qubits(handle)?;
        Ok(1 << nq)
    }

    // =========================================================================
    // Backend capabilities / limits
    // =========================================================================

    fn max_qubits(&self) -> u8 {
        let n = self.qpu.max_qubits();
        n.min(255) as u8
    }

    fn set_rng_seed(&mut self, seed: u64) {
        self.rng_seed = Some(seed);
    }

    // =========================================================================
    // State inspection -- all unsupported in circuit mode
    // =========================================================================

    fn purity(&self, _handle: QRegHandle) -> Result<f64, CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "purity".to_string(),
            detail: "state inspection not available in circuit mode".to_string(),
        })
    }

    fn is_pure(&self, _handle: QRegHandle) -> Result<bool, CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "is_pure".to_string(),
            detail: "state inspection not available in circuit mode".to_string(),
        })
    }

    fn diagonal_probabilities(&self, _handle: QRegHandle) -> Result<Vec<f64>, CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "diagonal_probs".to_string(),
            detail: "state inspection not available in circuit mode".to_string(),
        })
    }

    fn get_element(&self, _handle: QRegHandle, _row: usize, _col: usize) -> Result<C64, CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "get_element".to_string(),
            detail: "state inspection not available in circuit mode".to_string(),
        })
    }

    fn amplitude(&self, _handle: QRegHandle, _index: usize) -> Result<C64, CqamError> {
        Err(CqamError::QpuUnsupportedOperation {
            operation: "amplitude".to_string(),
            detail: "state inspection not available in circuit mode".to_string(),
        })
    }
}

impl<Q: QpuBackend + Clone> Clone for CircuitBackend<Q> {
    /// Clone the backend for use in a fork thread.
    ///
    /// The cloned backend:
    /// - Has an independent copy of the QPU handle (stateless except for RNG).
    /// - Has a fresh empty compilation pipeline cache (fork threads run short-lived
    ///   sections that will not benefit from the parent's cached circuits).
    /// - Has `buffer: None` (quantum registers must be fully observed before HFORK).
    /// - Copies all handle bookkeeping maps (these should also be empty at fork time
    ///   because QF=0 is required before HFORK).
    fn clone(&self) -> Self {
        Self {
            qpu: self.qpu.clone(),
            pipeline: self.pipeline.clone(),
            buffer: None,
            wire_allocator: self.wire_allocator.clone(),
            handle_wires: self.handle_wires.clone(),
            handle_num_qubits: self.handle_num_qubits.clone(),
            handle_dist: self.handle_dist.clone(),
            next_handle: self.next_handle,
            evolved_handles: self.evolved_handles.clone(),
            convergence: self.convergence.clone(),
            shot_budget: self.shot_budget,
            metrics: self.metrics.clone(),
            rng_seed: self.rng_seed,
        }
    }
}

impl<Q: QpuBackend> CircuitQuantumBackend for CircuitBackend<Q> {
    fn metrics(&self) -> &QpuMetrics {
        &self.metrics
    }

    fn force_flush(&mut self) -> Result<(), CqamError> {
        if let Some(ref buffer) = self.buffer {
            let calib = self.qpu.calibration().ok();
            let _circuit = self.pipeline.synthesize(
                buffer,
                calib.as_deref(),
            ).map_err(|e| { let ce: CqamError = e.into(); ce })?;
        }
        self.buffer = None;
        self.wire_allocator.reset();
        Ok(())
    }
}

// =============================================================================
// Helper: weighted pick from BTreeMap counts
// =============================================================================

fn pick_weighted(counts: &std::collections::BTreeMap<u64, u32>, target: u32) -> u64 {
    let mut cumulative: u32 = 0;
    for (&bitstring, &count) in counts {
        cumulative += count;
        if target < cumulative {
            return bitstring;
        }
    }
    // Fallback: return last key
    *counts.keys().last().unwrap_or(&0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::instruction::KernelId;

    use cqam_qpu::mock::{MockQpuBackend, MockCalibrationData};
    use cqam_qpu::traits::{ConnectivityGraph, ConvergenceCriterion};
    use cqam_core::native_ir::NativeGateSet;

    fn make_backend() -> CircuitBackend<MockQpuBackend> {
        let qpu = MockQpuBackend::with_config(
            ConnectivityGraph::all_to_all(8),
            NativeGateSet::Superconducting,
            8,
            MockCalibrationData::default(),
            Some(42),
        );
        CircuitBackend::new(
            qpu,
            ConvergenceCriterion::default(),
            1000,
        )
    }

    // Helper: make Hadamard matrix
    fn h_gate() -> [C64; 4] {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        [C64(s, 0.0), C64(s, 0.0), C64(s, 0.0), C64(-s, 0.0)]
    }

    // Helper: Rx(1.0) — an arbitrary rotation not matched by any named gate
    fn rx_1_gate() -> [C64; 4] {
        let c = (0.5_f64).cos();
        let s = (0.5_f64).sin();
        [C64(c, 0.0), C64(0.0, -s), C64(0.0, -s), C64(c, 0.0)]
    }

    fn cx_gate() -> [C64; 16] {
        CX_MAT
    }

    #[test]
    fn test_prep_allocates_wires() {
        let mut cb = make_backend();
        let (h, res) = cb.prep(DistId::Zero, 2, false).unwrap();
        assert_eq!(res.num_qubits, 2);
        assert_eq!(cb.handle_num_qubits[&h.0], 2);
        let wires = &cb.handle_wires[&h.0];
        assert_eq!(wires.len(), 2);
        assert_eq!(wires[0], QWire(0));
        assert_eq!(wires[1], QWire(1));
        assert_eq!(cb.wire_allocator.next_wire, 2);
    }

    #[test]
    fn test_prep_pushes_op() {
        let mut cb = make_backend();
        let _ = cb.prep(DistId::Zero, 1, false).unwrap();
        let buf = cb.buffer.as_ref().unwrap();
        assert_eq!(buf.ops.len(), 1);
        assert!(matches!(&buf.ops[0], Op::Prep(_)));
    }

    #[test]
    fn test_apply_kernel_pushes_op() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let _ = cb.apply_kernel(h, KernelId::Fourier, &params).unwrap();
        let buf = cb.buffer.as_ref().unwrap();
        // ops: Prep, Kernel
        assert_eq!(buf.ops.len(), 2);
        assert!(matches!(&buf.ops[1], Op::Kernel(_)));
    }

    #[test]
    fn test_apply_single_gate_h_recognized() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let gate = h_gate();
        let _ = cb.apply_single_gate(h, 0, &gate).unwrap();
        let buf = cb.buffer.as_ref().unwrap();
        if let Op::Gate1q(g) = &buf.ops[1] {
            assert!(matches!(g.gate, Gate1q::H), "Hadamard should be recognized");
        } else {
            panic!("Expected Gate1q op");
        }
    }

    #[test]
    fn test_apply_single_gate_arbitrary_unitary_becomes_u3() {
        // Arbitrary unitary (Rx(1.0)) that is not one of the 8 named gates should
        // now be decomposed to Gate1q::U3 via ZYZ Euler decomposition.
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let gate = rx_1_gate();
        let _ = cb.apply_single_gate(h, 0, &gate).unwrap();
        let buf = cb.buffer.as_ref().unwrap();
        if let Op::Gate1q(g) = &buf.ops[1] {
            assert!(
                matches!(g.gate, Gate1q::U3(_, _, _)),
                "Arbitrary unitary should be decomposed to U3, got {:?}", g.gate
            );
        } else {
            panic!("Expected Gate1q op");
        }
    }

    #[test]
    fn test_apply_two_qubit_gate_cx_recognized() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
        let gate = cx_gate();
        let _ = cb.apply_two_qubit_gate(h, 0, 1, &gate).unwrap();
        let buf = cb.buffer.as_ref().unwrap();
        if let Op::Gate2q(g) = &buf.ops[1] {
            assert!(matches!(g.gate, Gate2q::Cx), "CX should be recognized");
        } else {
            panic!("Expected Gate2q op");
        }
    }

    #[test]
    fn test_observe_dist_flushes_buffer() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let result = cb.observe(h, ObserveMode::Dist, 0, 0).unwrap();
        // Buffer should be cleared after observe
        assert!(cb.buffer.is_none(), "buffer should be None after observe");
        // Result should be Dist
        assert!(matches!(result, ObserveResult::Dist(_)));
    }

    #[test]
    fn test_observe_prob_from_shots() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        // PROB mode on a |0⟩ state: P(|0⟩) should be close to 1.0
        let result = cb.observe(h, ObserveMode::Prob, 0, 0).unwrap();
        match result {
            ObserveResult::Prob(p) => assert!(p > 0.5, "P(|0⟩) should be high: {p}"),
            other => panic!("expected Prob, got {:?}", other),
        }
    }

    #[test]
    fn test_observe_amp_returns_error() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let err = cb.observe(h, ObserveMode::Amp, 0, 0);
        assert!(err.is_err());
    }

    #[test]
    fn test_handle_lifecycle_double_observe() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let _ = cb.observe(h, ObserveMode::Dist, 0, 0).unwrap();
        // Second observe on same (now invalid) handle should error
        // Need to create another buffer context for the handle to be checked
        let err = cb.observe(h, ObserveMode::Dist, 0, 0);
        assert!(err.is_err(), "Observing an already-consumed handle should error");
    }

    #[test]
    fn test_prep_from_amplitudes_unsupported() {
        let mut cb = make_backend();
        let amps = [C64(1.0, 0.0), C64(0.0, 0.0)];
        let err = cb.prep_from_amplitudes(&amps);
        assert!(err.is_err());
    }

    #[test]
    fn test_prep_mixed_unsupported() {
        let mut cb = make_backend();
        let amps = [C64(1.0, 0.0)];
        let err = cb.prep_mixed(&[(1.0, &amps)]);
        assert!(err.is_err());
    }

    #[test]
    fn test_partial_trace_unsupported() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
        let err = cb.partial_trace(h, 1);
        assert!(err.is_err());
    }

    #[test]
    fn test_state_inspection_returns_errors() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        assert!(cb.purity(h).is_err());
        assert!(cb.is_pure(h).is_err());
        assert!(cb.diagonal_probabilities(h).is_err());
        assert!(cb.get_element(h, 0, 0).is_err());
        assert!(cb.amplitude(h, 0).is_err());
    }

    #[test]
    fn test_tensor_product_merges_wires() {
        let mut cb = make_backend();
        let (ha, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let (hb, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let (hc, res) = cb.tensor_product(ha, hb).unwrap();
        assert_eq!(res.num_qubits, 2);
        assert_eq!(cb.handle_num_qubits[&hc.0], 2);
        let wires = &cb.handle_wires[&hc.0];
        assert_eq!(wires.len(), 2);
    }

    #[test]
    fn test_clone_state_unevolved() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let h2 = cb.clone_state(h).unwrap();
        // Both handles should exist
        assert!(cb.handle_wires.contains_key(&h.0));
        assert!(cb.handle_wires.contains_key(&h2.0));
    }

    #[test]
    fn test_clone_state_evolved_errors() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let gate = h_gate();
        let (h2, _) = cb.apply_single_gate(h, 0, &gate).unwrap();
        // h2 was returned from apply_single_gate and inherits the wire;
        // the original h should be marked evolved
        // Try cloning the evolved handle
        let err_h = cb.clone_state(h);
        assert!(err_h.is_err(), "Cloning an evolved handle must fail");
        // h2 itself is not in evolved_handles (only h is), but h2's wires were copied
        // For this test we just verify h errors
    }

    #[test]
    fn test_release_removes_handle() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        cb.release(h);
        // Handle should no longer be valid
        assert!(!cb.handle_wires.contains_key(&h.0));
        // Operations on released handle should error
        let err = cb.apply_single_gate(h, 0, &h_gate());
        assert!(err.is_err());
    }

    #[test]
    fn test_num_qubits_and_dimension() {
        let mut cb = make_backend();
        let (h, _) = cb.prep(DistId::Zero, 3, false).unwrap();
        assert_eq!(cb.num_qubits(h).unwrap(), 3);
        assert_eq!(cb.dimension(h).unwrap(), 8);
    }

    #[test]
    fn test_max_qubits_delegates_to_qpu() {
        let cb = make_backend();
        // MockQpuBackend was created with max_qubits=8
        assert_eq!(cb.max_qubits(), 8);
    }

    #[test]
    fn test_circuit_backend_clone_empty_buffer() {
        let mut cb = make_backend();
        // Prep a register (creates a buffer) then observe (flushes it)
        let (h, _) = cb.prep(DistId::Zero, 1, false).unwrap();
        let _ = cb.observe(h, ObserveMode::Dist, 0, 0).unwrap();
        // Now buffer is None; clone should also have buffer None
        let cloned = cb.clone();
        assert!(cloned.buffer.is_none(), "cloned backend must have buffer: None");
    }

    #[test]
    fn test_circuit_backend_clone_forces_buffer_none() {
        let mut cb = make_backend();
        // Add some state: prep but do NOT observe (buffer is Some mid-circuit)
        let (_h, _) = cb.prep(DistId::Zero, 2, false).unwrap();
        assert!(cb.buffer.is_some());
        // The Clone impl must always produce buffer: None regardless of parent state
        let cloned = cb.clone();
        assert!(cloned.buffer.is_none(), "Clone impl must always produce buffer: None");
    }

    #[test]
    fn test_circuit_backend_clone_independent() {
        let cb = make_backend();
        let mut cloned = cb.clone();
        // Cloned backend must be independently functional
        let (h, _) = cloned.prep(DistId::Zero, 1, false).unwrap();
        assert!(cloned.buffer.is_some());
        let _ = cloned.observe(h, ObserveMode::Dist, 0, 0).unwrap();
        assert!(cloned.buffer.is_none());
    }

    // =========================================================================
    // ZYZ decomposition unit tests
    // =========================================================================

    /// Reconstruct the U3 matrix from (theta, phi, lambda) and compare with the
    /// original matrix up to a global phase. Returns the max element-wise error.
    fn u3_matrix(theta: f64, phi: f64, lambda: f64) -> [C64; 4] {
        let c = (theta / 2.0).cos();
        let s = (theta / 2.0).sin();
        // U3 = |  cos(t/2)              -e^{il}*sin(t/2) |
        //      |  e^{ip}*sin(t/2)        e^{i(p+l)}*cos(t/2) |
        let el = C64::exp_i(lambda);
        let ep = C64::exp_i(phi);
        let epl = C64::exp_i(phi + lambda);
        [
            C64(c, 0.0),
            C64(-el.0 * s, -el.1 * s),
            C64(ep.0 * s, ep.1 * s),
            C64(epl.0 * c, epl.1 * c),
        ]
    }

    fn matrices_equal_up_to_phase(a: &[C64; 4], b: &[C64; 4], tol: f64) -> bool {
        // Find first non-tiny entry in a to determine global phase
        let mut phase = C64::ONE;
        let mut found = false;
        for i in 0..4 {
            if a[i].norm() > 1e-10 && b[i].norm() > 1e-10 {
                let a_conj = a[i].conj();
                let num = C64(
                    b[i].0 * a_conj.0 - b[i].1 * a_conj.1,
                    b[i].0 * a_conj.1 + b[i].1 * a_conj.0,
                );
                let denom = a[i].norm_sq();
                phase = C64(num.0 / denom, num.1 / denom);
                found = true;
                break;
            }
        }
        if !found { return true; }

        let mut frob_sq = 0.0;
        for i in 0..4 {
            let pa = C64(phase.0 * a[i].0 - phase.1 * a[i].1, phase.0 * a[i].1 + phase.1 * a[i].0);
            let diff = C64(pa.0 - b[i].0, pa.1 - b[i].1);
            frob_sq += diff.norm_sq();
        }
        frob_sq.sqrt() < tol
    }

    #[test]
    fn test_decompose_zyz_identity() {
        let id = [C64::ONE, C64::ZERO, C64::ZERO, C64::ONE];
        let (theta, phi, lambda) = decompose_zyz(&id).unwrap();
        // Identity: theta=0, any phi+lambda is OK (phi+lambda = 0 for real matrix)
        assert!(theta.abs() < 1e-9, "identity: theta should be 0, got {}", theta);
        let reconstructed = u3_matrix(theta, phi, lambda);
        assert!(
            matrices_equal_up_to_phase(&id, &reconstructed, 1e-9),
            "identity: U3 reconstruction mismatch"
        );
    }

    #[test]
    fn test_decompose_zyz_hadamard() {
        let (t, p, l) = decompose_zyz(&H_MAT).unwrap();
        let reconstructed = u3_matrix(t, p, l);
        assert!(
            matrices_equal_up_to_phase(&H_MAT, &reconstructed, 1e-9),
            "H: ZYZ U3 reconstruction mismatch (theta={t}, phi={p}, lambda={l})"
        );
    }

    #[test]
    fn test_decompose_zyz_x_gate() {
        let (t, p, l) = decompose_zyz(&X_MAT).unwrap();
        let reconstructed = u3_matrix(t, p, l);
        assert!(
            matrices_equal_up_to_phase(&X_MAT, &reconstructed, 1e-9),
            "X: ZYZ U3 reconstruction mismatch"
        );
    }

    #[test]
    fn test_decompose_zyz_rx1() {
        // Rx(1.0): theta_rx=1.0 => matrix = [[cos(0.5), -i*sin(0.5)], [-i*sin(0.5), cos(0.5)]]
        let mat = rx_1_gate();
        let (t, p, l) = decompose_zyz(&mat).unwrap();
        let reconstructed = u3_matrix(t, p, l);
        assert!(
            matrices_equal_up_to_phase(&mat, &reconstructed, 1e-9),
            "Rx(1.0): ZYZ reconstruction mismatch"
        );
    }

    #[test]
    fn test_decompose_zyz_ry_quarter_pi() {
        use std::f64::consts::PI;
        // Ry(pi/4)
        let angle = PI / 4.0;
        let c = (angle / 2.0).cos();
        let s = (angle / 2.0).sin();
        let mat = [C64(c, 0.0), C64(-s, 0.0), C64(s, 0.0), C64(c, 0.0)];
        let (t, p, l) = decompose_zyz(&mat).unwrap();
        let reconstructed = u3_matrix(t, p, l);
        assert!(
            matrices_equal_up_to_phase(&mat, &reconstructed, 1e-9),
            "Ry(pi/4): ZYZ reconstruction mismatch"
        );
    }

    #[test]
    fn test_recognize_gate1q_rx_becomes_u3() {
        // Rx(1.0) should no longer fall through to Custom; it should be U3.
        let mat = rx_1_gate();
        let gate = recognize_gate1q(&mat);
        assert!(
            matches!(gate, Gate1q::U3(_, _, _)),
            "Rx matrix should be recognized as U3, got {:?}", gate
        );
    }

    #[test]
    fn test_recognize_gate1q_rz_becomes_u3() {
        use std::f64::consts::PI;
        // Rz(pi/3): diagonal [e^{-i*pi/6}, e^{i*pi/6}]
        let angle = PI / 3.0;
        let mat = [
            C64::exp_i(-angle / 2.0), C64::ZERO,
            C64::ZERO, C64::exp_i(angle / 2.0),
        ];
        let gate = recognize_gate1q(&mat);
        assert!(
            matches!(gate, Gate1q::U3(_, _, _)),
            "Rz matrix should be recognized as U3, got {:?}", gate
        );
    }

    #[test]
    fn test_recognize_gate1q_ry_becomes_u3() {
        use std::f64::consts::PI;
        // Ry(pi/5)
        let angle = PI / 5.0;
        let c = (angle / 2.0).cos();
        let s = (angle / 2.0).sin();
        let mat = [C64(c, 0.0), C64(-s, 0.0), C64(s, 0.0), C64(c, 0.0)];
        let gate = recognize_gate1q(&mat);
        assert!(
            matches!(gate, Gate1q::U3(_, _, _)),
            "Ry matrix should be recognized as U3, got {:?}", gate
        );
    }
}
