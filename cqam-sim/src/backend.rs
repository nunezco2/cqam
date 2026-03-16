//! SimulationBackend: concrete QuantumBackend implementation using cqam-sim.
//!
//! Stores quantum states internally as `QuantumRegister` values, keyed by
//! monotonically increasing `QRegHandle` IDs. All quantum operations are
//! delegated to the existing kernel infrastructure in `cqam-sim`.

use std::collections::HashMap;
use std::sync::Arc;
use rand::SeedableRng;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use cqam_core::error::CqamError;
use cqam_core::instruction::{DistId, KernelId, ObserveMode};
use cqam_core::quantum_backend::{
    KernelParams, MeasResult, ObserveResult, QOpResult, QRegHandle, QuantumBackend,
};

use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use crate::kernel::Kernel;
use crate::kernels::controlled_u::ControlledU;
use crate::kernels::diagonal::DiagonalUnitary;
use crate::kernels::diffuse::Diffuse;
use crate::kernels::entangle::Entangle;
use crate::kernels::fourier::Fourier;
use crate::kernels::fourier_inv::FourierInv;
use crate::kernels::grover::GroverIter;
use crate::kernels::init::Init;
use crate::kernels::permutation::Permutation;
use crate::kernels::phase::PhaseShift;
use crate::kernels::rotate::Rotate;
use crate::quantum_register::QuantumRegister;
use crate::constants::{MAX_SV_QUBITS, MAX_QUBITS};
use crate::noise::{NoiseModel, NoiseMethod};

/// Concrete QuantumBackend implementation using the cqam-sim simulation engine.
///
/// Stores quantum states in a HashMap keyed by monotonically increasing handle IDs.
/// Owns its own RNG for reproducible measurements.
pub struct SimulationBackend {
    /// State storage: handle -> QuantumRegister.
    states: HashMap<u64, QuantumRegister>,
    /// Next handle ID to assign (monotonically increasing).
    next_id: u64,
    /// Seedable RNG for reproducible measurements.
    rng: ChaCha8Rng,
    /// Whether density matrix mode is forced (affects max_qubits).
    force_density_matrix: bool,
    /// Optional noise model. When Some, noise is injected after every
    /// gate, during idle periods, at prep, and at readout.
    noise_model: Option<Arc<dyn NoiseModel>>,
    /// Active noise simulation method.
    noise_method: Option<NoiseMethod>,
}

impl SimulationBackend {
    /// Create a new SimulationBackend with default settings.
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            next_id: 0,
            rng: ChaCha8Rng::from_entropy(),
            force_density_matrix: false,
            noise_model: None,
            noise_method: None,
        }
    }

    /// Set whether density matrix mode is forced.
    pub fn set_force_density_matrix(&mut self, force: bool) {
        self.force_density_matrix = force;
    }

    /// Whether density matrix mode is forced.
    pub fn force_density_matrix(&self) -> bool {
        self.force_density_matrix
    }

    /// Set the noise model and method. When noise_model is Some and
    /// method is DensityMatrix, automatically forces density matrix mode.
    pub fn set_noise_model(
        &mut self,
        model: Option<Arc<dyn NoiseModel>>,
        method: NoiseMethod,
    ) {
        if model.is_some() && method == NoiseMethod::DensityMatrix {
            self.force_density_matrix = true;
        }
        self.noise_model = model;
        self.noise_method = Some(method);
    }

    /// Whether a noise model is active.
    pub fn has_noise_model(&self) -> bool {
        self.noise_model.is_some()
    }

    /// Ensure a QuantumRegister is in Mixed form.
    fn ensure_density_matrix(qr: &mut QuantumRegister) {
        if let QuantumRegister::Pure(_) = qr {
            qr.ensure_mixed().expect(
                "noise model requires density matrix but promotion failed"
            );
        }
    }

    /// Extract &mut DensityMatrix from a QuantumRegister that has been promoted.
    fn as_density_matrix_mut(qr: &mut QuantumRegister) -> &mut DensityMatrix {
        match qr {
            QuantumRegister::Mixed(dm) => dm,
            QuantumRegister::Pure(_) => panic!(
                "expected Mixed variant after ensure_density_matrix"
            ),
        }
    }

    /// Apply post-gate noise to a quantum register for a single-qubit gate.
    fn inject_single_gate_noise(&mut self, qr: &mut QuantumRegister, target_qubit: u8) {
        if let Some(ref noise) = self.noise_model {
            let gate_time = noise.single_gate_time();
            match self.noise_method {
                Some(NoiseMethod::DensityMatrix) => {
                    Self::ensure_density_matrix(qr);
                    let dm = Self::as_density_matrix_mut(qr);
                    noise.post_single_gate(dm, target_qubit, gate_time);
                }
                Some(NoiseMethod::Trajectory) => {
                    if let QuantumRegister::Pure(sv) = qr {
                        let n = sv.num_qubits();
                        noise.trajectory_single_gate(
                            sv.amplitudes_mut(), n,
                            target_qubit, gate_time, &mut self.rng,
                        );
                    }
                }
                None => {}
            }
        }
    }

    /// Apply post-gate noise to a quantum register for a two-qubit gate.
    fn inject_two_qubit_gate_noise(&mut self, qr: &mut QuantumRegister, qubit_a: u8, qubit_b: u8) {
        if let Some(ref noise) = self.noise_model {
            let gate_time = noise.two_gate_time();
            match self.noise_method {
                Some(NoiseMethod::DensityMatrix) => {
                    Self::ensure_density_matrix(qr);
                    let dm = Self::as_density_matrix_mut(qr);
                    noise.post_two_qubit_gate(dm, qubit_a, qubit_b, gate_time);
                }
                Some(NoiseMethod::Trajectory) => {
                    if let QuantumRegister::Pure(sv) = qr {
                        let n = sv.num_qubits();
                        noise.trajectory_two_qubit_gate(
                            sv.amplitudes_mut(), n,
                            qubit_a, qubit_b, gate_time, &mut self.rng,
                        );
                    }
                }
                None => {}
            }
        }
    }

    /// Allocate a new handle and store the given state.
    fn alloc(&mut self, state: QuantumRegister) -> QRegHandle {
        let id = self.next_id;
        self.next_id += 1;
        self.states.insert(id, state);
        QRegHandle(id)
    }

    /// Get a reference to the state for a handle.
    fn get_state(&self, handle: QRegHandle) -> Result<&QuantumRegister, CqamError> {
        self.states.get(&handle.0).ok_or_else(|| CqamError::UninitializedRegister {
            file: "Q".to_string(),
            index: 0,
        })
    }

    /// Make a QOpResult from a QuantumRegister reference.
    fn op_result(qr: &QuantumRegister) -> QOpResult {
        QOpResult {
            purity: qr.purity(),
            num_qubits: qr.num_qubits(),
        }
    }

    /// Build a kernel from a KernelId and KernelParams.
    ///
    /// This contains the logic that was previously in qop.rs for constructing
    /// kernel structs from register values. The cmem_data in KernelParams::Int
    /// carries pre-read CMEM data for kernels that need it.
    fn build_kernel(
        &self,
        kernel: KernelId,
        params: &KernelParams,
        qr: &QuantumRegister,
    ) -> Result<Box<dyn Kernel>, CqamError> {
        match params {
            KernelParams::Int { param0, param1, cmem_data } => {
                self.build_kernel_int(kernel, *param0, *param1, cmem_data, qr)
            }
            KernelParams::Float { param0, param1 } => {
                self.build_kernel_float(kernel, *param0, *param1, qr)
            }
            KernelParams::Complex { param0, param1 } => {
                self.build_kernel_complex(kernel, *param0, *param1, qr)
            }
        }
    }

    /// Build a kernel from integer parameters (QKERNEL).
    fn build_kernel_int(
        &self,
        kernel: KernelId,
        param0: i64,
        param1: i64,
        cmem_data: &[i64],
        qr: &QuantumRegister,
    ) -> Result<Box<dyn Kernel>, CqamError> {
        match kernel {
            KernelId::Init => Ok(Box::new(Init)),
            KernelId::Entangle => Ok(Box::new(Entangle)),
            KernelId::Fourier => Ok(Box::new(Fourier)),
            KernelId::Diffuse => Ok(Box::new(Diffuse)),
            KernelId::GroverIter => {
                let target = param0 as u16;
                let multi_addr = param1;

                if multi_addr == 0 {
                    Ok(Box::new(GroverIter::single(target)))
                } else {
                    // Multi-target mode: cmem_data contains [count, t0, t1, ...]
                    if cmem_data.is_empty() {
                        return Err(CqamError::TypeMismatch {
                            instruction: "QKERNEL/GROVER_ITER".to_string(),
                            detail: "multi-target mode requires cmem_data".to_string(),
                        });
                    }
                    let count = cmem_data[0] as usize;
                    let targets: Vec<u16> = cmem_data[1..=count]
                        .iter()
                        .map(|&t| t as u16)
                        .collect();
                    Ok(Box::new(GroverIter::multi(targets)))
                }
            }
            KernelId::FourierInv => Ok(Box::new(FourierInv)),
            KernelId::ControlledU => {
                // cmem_data layout:
                // [sub_kernel_id, power, param_re_bits, param_im_bits, target_qubits,
                //  ...optional sub-kernel data...]
                if cmem_data.len() < 5 {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QKERNEL/CONTROLLED_U".to_string(),
                        detail: "cmem_data too short for ControlledU".to_string(),
                    });
                }
                let control_qubit = param0 as u8;
                let sub_kernel_id = KernelId::try_from(cmem_data[0] as u8)?;
                let power = cmem_data[1] as u32;
                let param_re = f64::from_bits(cmem_data[2] as u64);
                let param_im = f64::from_bits(cmem_data[3] as u64);
                let target_qubits = cmem_data[4] as u8;

                // Check for sub-kernel data for CMEM-dependent kernels
                let sub_kernel_override: Option<Box<dyn Kernel>> =
                    if cmem_data.len() > 5 {
                        // Extra data present: decode based on sub_kernel_id
                        let extra = &cmem_data[5..];
                        match sub_kernel_id {
                            KernelId::DiagonalUnitary => {
                                let t = if target_qubits == 0 {
                                    qr.num_qubits() - 1
                                } else {
                                    target_qubits
                                };
                                let sub_dim = 1usize << t;
                                let mut diagonal = Vec::with_capacity(sub_dim);
                                for k in 0..sub_dim {
                                    let re = f64::from_bits(extra[2 * k] as u64);
                                    let im = f64::from_bits(extra[2 * k + 1] as u64);
                                    diagonal.push(C64(re, im));
                                }
                                Some(Box::new(DiagonalUnitary { diagonal }))
                            }
                            KernelId::Permutation => {
                                let t = if target_qubits == 0 {
                                    qr.num_qubits() - 1
                                } else {
                                    target_qubits
                                };
                                let sub_dim = 1usize << t;
                                let table: Vec<usize> = extra[..sub_dim]
                                    .iter()
                                    .map(|&v| v as usize)
                                    .collect();
                                let perm = Permutation::new(table)?;
                                Some(Box::new(perm))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                Ok(Box::new(ControlledU {
                    control_qubit,
                    sub_kernel_id,
                    power,
                    param_re,
                    param_im,
                    target_qubits,
                    sub_kernel_override,
                }))
            }
            KernelId::DiagonalUnitary => {
                // cmem_data contains interleaved (re_bits, im_bits) pairs
                let dim = param1 as usize;
                let qr_dim = qr.dimension();
                if dim != qr_dim {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QKERNEL/DIAGONAL_UNITARY".to_string(),
                        detail: format!(
                            "dim_reg={} but Q[src] dimension={}",
                            dim, qr_dim
                        ),
                    });
                }
                let mut diagonal = Vec::with_capacity(dim);
                for k in 0..dim {
                    let re = f64::from_bits(cmem_data[2 * k] as u64);
                    let im = f64::from_bits(cmem_data[2 * k + 1] as u64);
                    diagonal.push(C64(re, im));
                }
                Ok(Box::new(DiagonalUnitary { diagonal }))
            }
            KernelId::Permutation => {
                let dim = qr.dimension();
                if dim > 65536 {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QKERNEL/PERMUTATION".to_string(),
                        detail: format!(
                            "permutation table needs {} entries but CMEM has only 65536 cells",
                            dim
                        ),
                    });
                }
                let table: Vec<usize> = cmem_data[..dim]
                    .iter()
                    .map(|&v| v as usize)
                    .collect();
                let perm = Permutation::new(table)?;
                Ok(Box::new(perm))
            }
            _ => {
                Err(CqamError::UnknownKernel(
                    format!(
                        "Kernel {} not supported in QKERNEL (integer context); use QKERNELF or QKERNELZ",
                        kernel
                    ),
                ))
            }
        }
    }

    /// Build a kernel from float parameters (QKERNELF).
    fn build_kernel_float(
        &self,
        kernel: KernelId,
        fparam0: f64,
        fparam1: f64,
        _qr: &QuantumRegister,
    ) -> Result<Box<dyn Kernel>, CqamError> {
        match kernel {
            KernelId::Init => Ok(Box::new(Init)),
            KernelId::Entangle => Ok(Box::new(Entangle)),
            KernelId::Fourier => Ok(Box::new(Fourier)),
            KernelId::Diffuse => Ok(Box::new(Diffuse)),
            KernelId::GroverIter => {
                let target = fparam0 as u16;
                Ok(Box::new(GroverIter::single(target)))
            }
            KernelId::Rotate => Ok(Box::new(Rotate { theta: fparam0 })),
            KernelId::PhaseShift => Ok(Box::new(PhaseShift { amplitude: C64(fparam0, 0.0) })),
            KernelId::FourierInv => Ok(Box::new(FourierInv)),
            KernelId::ControlledU => {
                Ok(Box::new(ControlledU {
                    control_qubit: fparam0 as u8,
                    sub_kernel_id: KernelId::Rotate,
                    power: 0,
                    param_re: fparam1,
                    param_im: 0.0,
                    target_qubits: 0,
                    sub_kernel_override: None,
                }))
            }
            _ => {
                Err(CqamError::UnknownKernel(
                    format!("Unknown kernel ID: {}", kernel),
                ))
            }
        }
    }

    /// Build a kernel from complex parameters (QKERNELZ).
    fn build_kernel_complex(
        &self,
        kernel: KernelId,
        zparam0: C64,
        zparam1: C64,
        _qr: &QuantumRegister,
    ) -> Result<Box<dyn Kernel>, CqamError> {
        match kernel {
            KernelId::Init => Ok(Box::new(Init)),
            KernelId::Entangle => Ok(Box::new(Entangle)),
            KernelId::Fourier => Ok(Box::new(Fourier)),
            KernelId::Diffuse => Ok(Box::new(Diffuse)),
            KernelId::GroverIter => {
                let target = zparam0.0 as u16;
                Ok(Box::new(GroverIter::single(target)))
            }
            KernelId::Rotate => Ok(Box::new(Rotate { theta: zparam0.0 })),
            KernelId::PhaseShift => Ok(Box::new(PhaseShift { amplitude: C64(zparam0.0, zparam0.1) })),
            KernelId::FourierInv => Ok(Box::new(FourierInv)),
            KernelId::ControlledU => {
                Ok(Box::new(ControlledU {
                    control_qubit: zparam0.0 as u8,
                    sub_kernel_id: KernelId::try_from(zparam0.1 as u8)?,
                    power: 0,
                    param_re: zparam1.0,
                    param_im: zparam1.1,
                    target_qubits: 0,
                    sub_kernel_override: None,
                }))
            }
            _ => {
                Err(CqamError::UnknownKernel(
                    format!("Unknown kernel ID: {}", kernel),
                ))
            }
        }
    }

    /// Internal observe implementation (shared between observe and sample).
    fn observe_impl(
        &self,
        qr: &QuantumRegister,
        mode: ObserveMode,
        ctx0: usize,
        ctx1: usize,
        rng: Option<&mut ChaCha8Rng>,
    ) -> Result<ObserveResult, CqamError> {
        match mode {
            ObserveMode::Dist => {
                let mut probs = qr.diagonal_probabilities();
                // Apply readout noise to the full probability vector
                if let Some(ref noise) = self.noise_model {
                    if noise.has_readout_noise() {
                        noise.readout_noise(&mut probs, 0);
                    }
                }
                let dist_pairs: Vec<(u32, f64)> = probs
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| **p >= 1e-15)
                    .map(|(k, p)| (k as u32, *p))
                    .collect();
                Ok(ObserveResult::Dist(dist_pairs))
            }
            ObserveMode::Prob => {
                let dim = qr.dimension();
                if ctx0 >= dim {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QOBSERVE/PROB".to_string(),
                        index: ctx0,
                        limit: dim,
                    });
                }
                let prob = qr.get_element(ctx0, ctx0).0;
                Ok(ObserveResult::Prob(prob))
            }
            ObserveMode::Amp => {
                let dim = qr.dimension();
                if ctx0 >= dim || ctx1 >= dim {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QOBSERVE/AMP".to_string(),
                        index: ctx0.max(ctx1),
                        limit: dim,
                    });
                }
                let elem = qr.get_element(ctx0, ctx1);
                Ok(ObserveResult::Amp(elem))
            }
            ObserveMode::Sample => {
                let mut probs = qr.diagonal_probabilities();
                // Apply readout noise before sampling
                if let Some(ref noise) = self.noise_model {
                    if noise.has_readout_noise() {
                        noise.readout_noise(&mut probs, 0);
                    }
                }
                let r: f64 = if let Some(rng) = rng {
                    rng.gen_range(0.0..1.0)
                } else {
                    rand::thread_rng().gen_range(0.0..1.0)
                };
                let mut cumulative = 0.0;
                let mut outcome = (probs.len() - 1) as i64;
                for (k, p) in probs.iter().enumerate() {
                    cumulative += p;
                    if r < cumulative {
                        outcome = k as i64;
                        break;
                    }
                }
                Ok(ObserveResult::Sample(outcome))
            }
        }
    }
}

impl Default for SimulationBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SimulationBackend {
    fn clone(&self) -> Self {
        Self {
            states: self.states.clone(),
            next_id: self.next_id,
            rng: self.rng.clone(),
            force_density_matrix: self.force_density_matrix,
            noise_model: self.noise_model.clone(),
            noise_method: self.noise_method,
        }
    }
}

impl QuantumBackend for SimulationBackend {
    fn prep(
        &mut self,
        dist: DistId,
        num_qubits: u8,
        force_mixed: bool,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let mut qr = match dist {
            DistId::Uniform => QuantumRegister::new_uniform(num_qubits, force_mixed),
            DistId::Zero => QuantumRegister::new_zero_state(num_qubits, force_mixed),
            DistId::Bell => QuantumRegister::new_bell(force_mixed),
            DistId::Ghz => QuantumRegister::new_ghz(num_qubits, force_mixed)?,
        };
        // Prep noise (density matrix mode only)
        if let Some(ref noise) = self.noise_model {
            if self.noise_method == Some(NoiseMethod::DensityMatrix) {
                Self::ensure_density_matrix(&mut qr);
                let dm = Self::as_density_matrix_mut(&mut qr);
                noise.prep_noise(dm);
            }
        }
        let result = Self::op_result(&qr);
        let handle = self.alloc(qr);
        Ok((handle, result))
    }

    fn prep_from_amplitudes(
        &mut self,
        amplitudes: &[C64],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let qr = QuantumRegister::from_amplitudes(amplitudes.to_vec())?;
        let result = Self::op_result(&qr);
        let handle = self.alloc(qr);
        Ok((handle, result))
    }

    fn prep_mixed(
        &mut self,
        ensemble: &[(f64, &[C64])],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let dm = DensityMatrix::from_mixture(ensemble)?;
        let qr = QuantumRegister::Mixed(dm);
        let result = Self::op_result(&qr);
        let handle = self.alloc(qr);
        Ok((handle, result))
    }

    fn apply_kernel(
        &mut self,
        handle: QRegHandle,
        kernel: KernelId,
        params: &KernelParams,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let qr = self.get_state(handle)?.clone();
        let k = self.build_kernel(kernel, params, &qr)?;
        let mut result_qr = qr.apply_kernel(k.as_ref())?;
        // Post-kernel noise
        if let Some(ref noise) = self.noise_model {
            if self.noise_method == Some(NoiseMethod::DensityMatrix) {
                Self::ensure_density_matrix(&mut result_qr);
                let dm = Self::as_density_matrix_mut(&mut result_qr);
                noise.post_kernel(dm, 0, 0.0);
            }
        }
        let result = Self::op_result(&result_qr);
        let new_handle = self.alloc(result_qr);
        Ok((new_handle, result))
    }

    fn apply_single_gate(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
        gate: &[C64; 4],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let mut qr = self.get_state(handle)?.clone();
        qr.apply_single_qubit_gate(target_qubit, gate);
        self.inject_single_gate_noise(&mut qr, target_qubit);
        let result = Self::op_result(&qr);
        let new_handle = self.alloc(qr);
        Ok((new_handle, result))
    }

    fn apply_two_qubit_gate(
        &mut self,
        handle: QRegHandle,
        qubit_a: u8,
        qubit_b: u8,
        gate: &[C64; 16],
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let mut qr = self.get_state(handle)?.clone();
        qr.apply_two_qubit_gate(qubit_a, qubit_b, gate);
        self.inject_two_qubit_gate_noise(&mut qr, qubit_a, qubit_b);
        let result = Self::op_result(&qr);
        let new_handle = self.alloc(qr);
        Ok((new_handle, result))
    }

    fn apply_custom_unitary(
        &mut self,
        handle: QRegHandle,
        unitary: &[C64],
        dim: usize,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let qr = self.get_state(handle)?;
        let qr_dim = qr.dimension();
        if dim != qr_dim {
            return Err(CqamError::TypeMismatch {
                instruction: "QCUSTOM".to_string(),
                detail: format!("dim={} but Q[src] dimension={}", dim, qr_dim),
            });
        }

        // Validate unitarity: U^dagger * U ~= I
        let tol = 1e-6;
        for i in 0..dim {
            for j in 0..dim {
                let mut re_sum = 0.0_f64;
                let mut im_sum = 0.0_f64;
                for k in 0..dim {
                    let a = unitary[k * dim + i];
                    let b = unitary[k * dim + j];
                    re_sum += a.0 * b.0 + a.1 * b.1;
                    im_sum += a.0 * b.1 - a.1 * b.0;
                }
                let expected_re = if i == j { 1.0 } else { 0.0 };
                if (re_sum - expected_re).abs() > tol || im_sum.abs() > tol {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QCUSTOM".to_string(),
                        detail: format!(
                            "matrix is not unitary: (U^dagger*U)[{}][{}] = ({:.6}, {:.6}), expected ({:.1}, 0.0)",
                            i, j, re_sum, im_sum, expected_re
                        ),
                    });
                }
            }
        }

        let mut result_qr = qr.clone();
        result_qr.apply_unitary(unitary);
        let result = Self::op_result(&result_qr);
        let new_handle = self.alloc(result_qr);
        Ok((new_handle, result))
    }

    fn observe(
        &mut self,
        handle: QRegHandle,
        mode: ObserveMode,
        ctx0: usize,
        ctx1: usize,
    ) -> Result<ObserveResult, CqamError> {
        let qr = self.states.remove(&handle.0).ok_or_else(|| {
            CqamError::UninitializedRegister {
                file: "Q".to_string(),
                index: 0,
            }
        })?;
        let mut rng = self.rng.clone();
        let result = self.observe_impl(&qr, mode, ctx0, ctx1, Some(&mut rng));
        // Advance the real RNG if Sample mode was used (to keep determinism)
        if matches!(mode, ObserveMode::Sample) {
            self.rng = rng;
        }
        result
    }

    fn sample(
        &mut self,
        handle: QRegHandle,
        mode: ObserveMode,
        ctx0: usize,
        ctx1: usize,
    ) -> Result<ObserveResult, CqamError> {
        let qr = self.get_state(handle)?;
        self.observe_impl(qr, mode, ctx0, ctx1, None)
    }

    fn measure_qubit(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
    ) -> Result<(QRegHandle, MeasResult), CqamError> {
        // Pre-check qubit bounds before removing
        {
            let qr = self.get_state(handle)?;
            if target_qubit >= qr.num_qubits() {
                return Err(CqamError::QuantumIndexOutOfRange {
                    instruction: "QMEAS".to_string(),
                    index: target_qubit as usize,
                    limit: qr.num_qubits() as usize,
                });
            }
        }

        let qr = self.states.remove(&handle.0).unwrap();
        let (outcome, post_qr) = qr.measure_qubit_with_rng(target_qubit, &mut self.rng);
        let purity = post_qr.purity();
        let new_handle = self.alloc(post_qr);
        Ok((
            new_handle,
            MeasResult { outcome, purity },
        ))
    }

    fn tensor_product(
        &mut self,
        handle_a: QRegHandle,
        handle_b: QRegHandle,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        // Pre-check both handles exist
        self.get_state(handle_a)?;
        self.get_state(handle_b)?;

        let qr_a = self.states.remove(&handle_a.0).unwrap();
        let qr_b = self.states.remove(&handle_b.0).unwrap();

        let result_qr = qr_a.tensor_product(&qr_b)?;

        let result = Self::op_result(&result_qr);
        let new_handle = self.alloc(result_qr);
        Ok((new_handle, result))
    }

    fn partial_trace(
        &mut self,
        handle: QRegHandle,
        num_qubits_a: u8,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        let qr = self.get_state(handle)?;
        let result_qr = qr.partial_trace_b(num_qubits_a)?;
        let result = Self::op_result(&result_qr);
        let new_handle = self.alloc(result_qr);
        Ok((new_handle, result))
    }

    fn reset_qubit(
        &mut self,
        handle: QRegHandle,
        target_qubit: u8,
    ) -> Result<(QRegHandle, QOpResult), CqamError> {
        // Pre-check bounds
        {
            let qr = self.get_state(handle)?;
            if target_qubit >= qr.num_qubits() {
                return Err(CqamError::QuantumIndexOutOfRange {
                    instruction: "QRESET".to_string(),
                    index: target_qubit as usize,
                    limit: qr.num_qubits() as usize,
                });
            }
        }

        let qr = self.states.remove(&handle.0).unwrap();

        let (outcome, mut post_qr) = qr.measure_qubit_with_rng(target_qubit, &mut self.rng);
        if outcome == 1 {
            // Pauli-X to flip back to |0>
            let x_gate: [C64; 4] = [
                C64(0.0, 0.0), C64(1.0, 0.0),
                C64(1.0, 0.0), C64(0.0, 0.0),
            ];
            post_qr.apply_single_qubit_gate(target_qubit, &x_gate);
        }

        let result = Self::op_result(&post_qr);
        let new_handle = self.alloc(post_qr);
        Ok((new_handle, result))
    }

    fn clone_state(
        &mut self,
        handle: QRegHandle,
    ) -> Result<QRegHandle, CqamError> {
        let qr = self.get_state(handle)?.clone();
        Ok(self.alloc(qr))
    }

    fn release(&mut self, handle: QRegHandle) {
        self.states.remove(&handle.0);
    }

    fn num_qubits(&self, handle: QRegHandle) -> Result<u8, CqamError> {
        Ok(self.get_state(handle)?.num_qubits())
    }

    fn dimension(&self, handle: QRegHandle) -> Result<usize, CqamError> {
        Ok(self.get_state(handle)?.dimension())
    }

    fn max_qubits(&self) -> u8 {
        if self.force_density_matrix {
            MAX_QUBITS
        } else {
            MAX_SV_QUBITS
        }
    }

    fn set_rng_seed(&mut self, seed: u64) {
        self.rng = ChaCha8Rng::seed_from_u64(seed);
    }

    fn purity(&self, handle: QRegHandle) -> Result<f64, CqamError> {
        let state = self.get_state(handle)?;
        Ok(state.purity())
    }

    fn is_pure(&self, handle: QRegHandle) -> Result<bool, CqamError> {
        let state = self.get_state(handle)?;
        Ok(matches!(state, QuantumRegister::Pure(_)))
    }

    fn diagonal_probabilities(&self, handle: QRegHandle) -> Result<Vec<f64>, CqamError> {
        let state = self.get_state(handle)?;
        Ok(state.diagonal_probabilities())
    }

    fn get_element(&self, handle: QRegHandle, row: usize, col: usize) -> Result<C64, CqamError> {
        let state = self.get_state(handle)?;
        Ok(state.get_element(row, col))
    }

    fn amplitude(&self, handle: QRegHandle, index: usize) -> Result<C64, CqamError> {
        let state = self.get_state(handle)?;
        match state {
            QuantumRegister::Pure(sv) => {
                Ok(sv.amplitude(index))
            }
            QuantumRegister::Mixed(_) => {
                let p = state.diagonal_probabilities()[index];
                Ok(C64(p.sqrt(), 0.0))
            }
        }
    }
}
