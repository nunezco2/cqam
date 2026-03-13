//! Quantum operation handlers for the CQAM virtual machine.
//!
//! Dispatches quantum instructions to the QuantumBackend trait, extracts
//! parameters from registers/CMEM, and updates PSW flags from operation results.

use cqam_core::error::CqamError;
use cqam_core::instruction::{Instruction, DistId, FileSel, KernelId, RotAxis};
use cqam_core::quantum_backend::{
    KernelParams, ObserveResult, QuantumBackend,
};
use cqam_core::register::HybridValue;
use cqam_sim::complex::C64;
use crate::context::ExecutionContext;

// =============================================================================
// Intent flag functions (ISA-level, not backend-specific)
// =============================================================================

/// Return (SF, EF, IF) intent flags for a kernel ID.
fn kernel_intent(kid: KernelId) -> (bool, bool, bool) {
    match kid {
        KernelId::Init             => (true,  false, false),
        KernelId::Entangle         => (true,  true,  false),
        KernelId::Fourier          => (true,  false, true),
        KernelId::Diffuse          => (true,  false, true),
        KernelId::GroverIter       => (true,  false, true),
        KernelId::PhaseShift       => (false, false, true),
        KernelId::Rotate           => (true,  false, false),
        KernelId::FourierInv       => (true,  false, true),
        KernelId::ControlledU      => (true,  true,  false),
        KernelId::Permutation      => (false, false, false),
        KernelId::DiagonalUnitary  => (false, false, true),
    }
}

/// Return (SF, EF) intent flags for a distribution ID.
/// IF is always false for preparation instructions.
fn dist_intent(dist: DistId) -> (bool, bool) {
    match dist {
        DistId::Uniform => (true, false),
        DistId::Zero    => (false, false),
        DistId::Bell    => (true, true),
        DistId::Ghz     => (true, true),
    }
}

// =============================================================================
// Gate matrices for masked register-level operations
// =============================================================================

/// Hadamard gate: (1/sqrt(2)) * [[1,1],[1,-1]]
fn hadamard() -> [C64; 4] {
    let h = std::f64::consts::FRAC_1_SQRT_2;
    [C64(h, 0.0), C64(h, 0.0), C64(h, 0.0), C64(-h, 0.0)]
}

/// Pauli-X (bit flip): [[0,1],[1,0]]
fn pauli_x() -> [C64; 4] {
    [C64::ZERO, C64::ONE, C64::ONE, C64::ZERO]
}

/// Pauli-Z (phase flip): [[1,0],[0,-1]]
fn pauli_z() -> [C64; 4] {
    [C64::ONE, C64::ZERO, C64::ZERO, C64(-1.0, 0.0)]
}

/// CZ gate (4x4): diag(1, 1, 1, -1)
fn cz_gate() -> [C64; 16] {
    [
        C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
        C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
        C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
        C64::ZERO, C64::ZERO, C64::ZERO, C64(-1.0, 0.0),
    ]
}

/// SWAP gate (4x4): swaps |01> <-> |10>
fn swap_gate() -> [C64; 16] {
    [
        C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
        C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
        C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
        C64::ZERO, C64::ZERO, C64::ZERO, C64::ONE,
    ]
}

/// CNOT gate (4x4)
fn cnot_gate() -> [C64; 16] {
    [
        C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
        C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
        C64::ZERO, C64::ZERO, C64::ZERO, C64::ONE,
        C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
    ]
}

/// Build Rx(theta)
fn rotation_x(theta: f64) -> [C64; 4] {
    let half = theta / 2.0;
    let c = half.cos();
    let s = half.sin();
    [
        C64(c, 0.0), C64(0.0, -s),
        C64(0.0, -s), C64(c, 0.0),
    ]
}

/// Build Ry(theta)
fn rotation_y(theta: f64) -> [C64; 4] {
    let half = theta / 2.0;
    let c = half.cos();
    let s = half.sin();
    [
        C64(c, 0.0), C64(-s, 0.0),
        C64(s, 0.0), C64(c, 0.0),
    ]
}

/// Build Rz(theta)
fn rotation_z(theta: f64) -> [C64; 4] {
    let half = theta / 2.0;
    [
        C64(half.cos(), -half.sin()), C64::ZERO,
        C64::ZERO, C64(half.cos(), half.sin()),
    ]
}

/// Execute a quantum instruction using the given backend.
///
/// Returns `Ok(())` on success, or `Err(CqamError)` on runtime errors.
pub fn execute_qop<B: QuantumBackend + ?Sized>(
    ctx: &mut ExecutionContext,
    instr: &Instruction,
    backend: &mut B,
) -> Result<(), CqamError> {
    match instr {
        Instruction::QPrep { dst, dist } => {
            let num_qubits = ctx.config.default_qubits;
            let force_dm = ctx.config.force_density_matrix;
            let (handle, _result) = backend.prep(*dist, num_qubits, force_dm)?;
            let (sf, ef) = dist_intent(*dist);
            ctx.set_qreg(*dst, handle, backend);
            ctx.psw.qf = true;
            ctx.psw.sf = sf;
            ctx.psw.ef = ef;
            ctx.psw.inf = false;
            ctx.psw.clear_decoherence();
            ctx.psw.cf = false;
            Ok(())
        }

        Instruction::QPrepR { dst, dist_reg } => {
            let dist_id_val = DistId::try_from(ctx.iregs.get(*dist_reg)? as u8)?;
            let num_qubits = ctx.config.default_qubits;
            let force_dm = ctx.config.force_density_matrix;
            let (handle, _result) = backend.prep(dist_id_val, num_qubits, force_dm)?;
            let (sf, ef) = dist_intent(dist_id_val);
            ctx.set_qreg(*dst, handle, backend);
            ctx.psw.qf = true;
            ctx.psw.sf = sf;
            ctx.psw.ef = ef;
            ctx.psw.inf = false;
            ctx.psw.clear_decoherence();
            ctx.psw.cf = false;
            Ok(())
        }

        Instruction::QPrepN { dst, dist, qubit_count_reg } => {
            let num_qubits = ctx.iregs.get(*qubit_count_reg)? as u8;
            let force_dm = ctx.config.force_density_matrix;
            let max_qubits = backend.max_qubits();

            if num_qubits == 0 || num_qubits > max_qubits {
                return Err(CqamError::QubitLimitExceeded {
                    instruction: "QPREPN".to_string(),
                    required: num_qubits,
                    max: max_qubits,
                });
            }

            let (handle, _result) = backend.prep(*dist, num_qubits, force_dm)?;
            let (sf, ef) = dist_intent(*dist);
            ctx.set_qreg(*dst, handle, backend);
            ctx.psw.qf = true;
            ctx.psw.sf = sf;
            ctx.psw.ef = ef;
            ctx.psw.inf = false;
            ctx.psw.clear_decoherence();
            ctx.psw.cf = false;
            Ok(())
        }

        Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
            let param0 = ctx.iregs.get(*ctx0)?;
            let param1 = ctx.iregs.get(*ctx1)?;

            if let Some(handle) = ctx.qregs[*src as usize] {
                // Pre-read CMEM data for kernels that need it
                let cmem_data = pre_read_cmem_int(ctx, *kernel, param0, param1, handle, backend)?;

                let params = KernelParams::Int { param0, param1, cmem_data };
                let (new_handle, result) = backend.apply_kernel(handle, *kernel, &params)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                let (sf, ef, inf) = kernel_intent(*kernel);
                ctx.psw.sf = sf;
                ctx.psw.ef = ef;
                ctx.psw.inf = inf;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 } => {
            let fparam0 = ctx.fregs.get(*fctx0)?;
            let fparam1 = ctx.fregs.get(*fctx1)?;

            if let Some(handle) = ctx.qregs[*src as usize] {
                let params = KernelParams::Float { param0: fparam0, param1: fparam1 };
                let (new_handle, result) = backend.apply_kernel(handle, *kernel, &params)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                let (sf, ef, inf) = kernel_intent(*kernel);
                ctx.psw.sf = sf;
                ctx.psw.ef = ef;
                ctx.psw.inf = inf;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 } => {
            let zparam0 = ctx.zregs.get(*zctx0)?;
            let zparam1 = ctx.zregs.get(*zctx1)?;

            if let Some(handle) = ctx.qregs[*src as usize] {
                let params = KernelParams::Complex { param0: C64(zparam0.0, zparam0.1), param1: C64(zparam1.0, zparam1.1) };
                let (new_handle, result) = backend.apply_kernel(handle, *kernel, &params)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                let (sf, ef, inf) = kernel_intent(*kernel);
                ctx.psw.sf = sf;
                ctx.psw.ef = ef;
                ctx.psw.inf = inf;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 } => {
            if let Some(handle) = ctx.take_qreg(*src_q) {
                let c0 = ctx.iregs.get(*ctx0)? as usize;
                let c1 = ctx.iregs.get(*ctx1)? as usize;
                let obs_result = backend.observe(handle, *mode, c0, c1)?;
                // observe consumes the handle; no need to release

                let hval = match obs_result {
                    ObserveResult::Dist(pairs) => HybridValue::Dist(pairs),
                    ObserveResult::Prob(p) => HybridValue::Complex(p, 0.0),
                    ObserveResult::Amp(c) => HybridValue::Complex(c.0, c.1),
                    ObserveResult::Sample(k) => {
                        ctx.psw.zf = k == 0;
                        HybridValue::Int(k)
                    }
                };
                ctx.hregs.set(*dst_h, hval)?;
                ctx.psw.sf = false;
                ctx.psw.ef = false;
                ctx.psw.inf = false;
                ctx.psw.mark_decohered();
                ctx.psw.mark_collapsed();
                ctx.psw.qf = ctx.qregs.iter().any(|q| q.is_some());
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
                })
            }
        }

        Instruction::QSample { dst_h, src_q, mode, ctx0, ctx1 } => {
            if let Some(handle) = ctx.qregs[*src_q as usize] {
                let c0 = ctx.iregs.get(*ctx0)? as usize;
                let c1 = ctx.iregs.get(*ctx1)? as usize;
                let obs_result = backend.sample(handle, *mode, c0, c1)?;

                let hval = match obs_result {
                    ObserveResult::Dist(pairs) => HybridValue::Dist(pairs),
                    ObserveResult::Prob(p) => HybridValue::Complex(p, 0.0),
                    ObserveResult::Amp(c) => HybridValue::Complex(c.0, c.1),
                    ObserveResult::Sample(k) => HybridValue::Int(k),
                };
                ctx.hregs.set(*dst_h, hval)?;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
                })
            }
        }

        Instruction::QLoad { dst_q, addr } => {
            if let Some(qmem_handle) = ctx.qmem.load(*addr) {
                let new_handle = backend.clone_state(*qmem_handle)?;
                ctx.set_qreg(*dst_q, new_handle, backend);
                ctx.psw.qf = true;
                ctx.psw.sf = false;
                ctx.psw.ef = false;
                ctx.psw.inf = false;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "QMEM".to_string(),
                    index: *addr,
                })
            }
        }

        Instruction::QStore { src_q, addr } => {
            if let Some(handle) = ctx.qregs[*src_q as usize] {
                let cloned = backend.clone_state(handle)?;
                // Release any previous QMEM handle at this address
                if let Some(old) = ctx.qmem.take(*addr) {
                    backend.release(old);
                }
                ctx.qmem.store(*addr, cloned);
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
                })
            }
        }

        Instruction::QHadM { dst, src, mask_reg } => {
            execute_masked_gate_backend(ctx, backend, *dst, *src, *mask_reg, hadamard, "QHADM", (true, false, false))
        }

        Instruction::QFlip { dst, src, mask_reg } => {
            execute_masked_gate_backend(ctx, backend, *dst, *src, *mask_reg, pauli_x, "QFLIP", (false, false, false))
        }

        Instruction::QPhase { dst, src, mask_reg } => {
            execute_masked_gate_backend(ctx, backend, *dst, *src, *mask_reg, pauli_z, "QPHASE", (false, false, false))
        }

        Instruction::QEncode { dst, src_base, count, file_sel: fs } => {
            let count_val = *count as usize;

            if count_val == 0 || (count_val & (count_val - 1)) != 0 {
                return Err(CqamError::TypeMismatch {
                    instruction: "QENCODE".to_string(),
                    detail: format!(
                        "count must be a power of 2, got {}",
                        count_val
                    ),
                });
            }

            let mut psi: Vec<C64> = Vec::with_capacity(count_val);
            for i in 0..count_val {
                let reg_idx = src_base + i as u8;
                let amplitude: C64 = match *fs {
                    FileSel::RFile => {
                        let val = ctx.iregs.get(reg_idx)?;
                        C64(val as f64, 0.0)
                    }
                    FileSel::FFile => {
                        let val = ctx.fregs.get(reg_idx)?;
                        C64(val, 0.0)
                    }
                    FileSel::ZFile => {
                        let z = ctx.zregs.get(reg_idx)?;
                        C64(z.0, z.1)
                    }
                };
                psi.push(amplitude);
            }

            let norm_sq: f64 = psi.iter()
                .map(|c| c.0 * c.0 + c.1 * c.1)
                .sum();
            if norm_sq < 1e-30 {
                return Err(CqamError::TypeMismatch {
                    instruction: "QENCODE".to_string(),
                    detail: "statevector has zero norm".to_string(),
                });
            }

            let (handle, _result) = backend.prep_from_amplitudes(&psi)?;
            ctx.set_qreg(*dst, handle, backend);
            ctx.psw.qf = true;
            ctx.psw.sf = true;
            ctx.psw.ef = false;
            ctx.psw.inf = false;
            Ok(())
        }

        Instruction::QCnot { dst, src, ctrl_qubit_reg, tgt_qubit_reg } => {
            let ctrl = ctx.iregs.get(*ctrl_qubit_reg)? as u8;
            let tgt = ctx.iregs.get(*tgt_qubit_reg)? as u8;

            if let Some(handle) = ctx.qregs[*src as usize] {
                if ctrl == tgt {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QCNOT".to_string(),
                        detail: format!("ctrl ({}) == tgt ({})", ctrl, tgt),
                    });
                }
                let n = backend.num_qubits(handle)?;
                if ctrl >= n || tgt >= n {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QCNOT".to_string(),
                        index: ctrl.max(tgt) as usize,
                        limit: n as usize,
                    });
                }

                let gate = cnot_gate();
                let (new_handle, result) = backend.apply_two_qubit_gate(handle, ctrl, tgt, &gate)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                ctx.psw.sf = false;
                ctx.psw.ef = true;
                ctx.psw.inf = false;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QRot { dst, src, qubit_reg, axis, angle_freg } => {
            let qubit = ctx.iregs.get(*qubit_reg)? as u8;
            let theta = ctx.fregs.get(*angle_freg)?;

            if let Some(handle) = ctx.qregs[*src as usize] {
                let n = backend.num_qubits(handle)?;
                if qubit >= n {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QROT".to_string(),
                        index: qubit as usize,
                        limit: n as usize,
                    });
                }

                let gate = match *axis {
                    RotAxis::X => rotation_x(theta),
                    RotAxis::Y => rotation_y(theta),
                    RotAxis::Z => rotation_z(theta),
                };

                let (new_handle, result) = backend.apply_single_gate(handle, qubit, &gate)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                ctx.psw.sf = true;
                ctx.psw.ef = false;
                ctx.psw.inf = false;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QMeas { dst_r, src_q, qubit_reg } => {
            let qubit = ctx.iregs.get(*qubit_reg)? as u8;

            if let Some(handle) = ctx.take_qreg(*src_q) {
                let n = backend.num_qubits(handle)?;
                if qubit >= n {
                    // Put handle back
                    ctx.qregs[*src_q as usize] = Some(handle);
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QMEAS".to_string(),
                        index: qubit as usize,
                        limit: n as usize,
                    });
                }

                let (new_handle, meas) = backend.measure_qubit(handle, qubit)?;
                // Old handle consumed by measure_qubit

                ctx.iregs.set(*dst_r, meas.outcome as i64)?;
                ctx.qregs[*src_q as usize] = Some(new_handle);

                ctx.psw.update_from_qmeta(meas.purity, ctx.config.min_purity);
                ctx.psw.sf = false;
                ctx.psw.ef = false;
                ctx.psw.inf = false;
                ctx.psw.mark_decohered();
                ctx.psw.mark_collapsed();
                ctx.psw.zf = meas.outcome == 0;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
                })
            }
        }

        Instruction::QTensor { dst, src0, src1 } => {
            let h0 = ctx.take_qreg(*src0).ok_or_else(|| {
                CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src0,
                }
            })?;
            let h1 = ctx.take_qreg(*src1).ok_or_else(|| {
                // Put h0 back
                ctx.qregs[*src0 as usize] = Some(h0);
                CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src1,
                }
            })?;

            let (new_handle, result) = backend.tensor_product(h0, h1)?;
            // tensor_product consumes both handles

            ctx.set_qreg(*dst, new_handle, backend);
            ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
            ctx.psw.sf = false;
            ctx.psw.ef = false;
            ctx.psw.inf = false;
            Ok(())
        }

        Instruction::QCustom { dst, src, base_addr_reg, dim_reg } => {
            let base_addr = ctx.iregs.get(*base_addr_reg)? as u16;
            let dim_val = ctx.iregs.get(*dim_reg)? as usize;

            if let Some(handle) = ctx.qregs[*src as usize] {
                // Read unitary from CMEM: 2 * dim * dim cells (re, im pairs)
                let mut unitary = Vec::with_capacity(dim_val * dim_val);
                for idx in 0..dim_val * dim_val {
                    let addr = base_addr.wrapping_add((2 * idx) as u16);
                    let re = f64::from_bits(ctx.cmem.load(addr) as u64);
                    let im = f64::from_bits(ctx.cmem.load(addr.wrapping_add(1)) as u64);
                    unitary.push(C64(re, im));
                }

                let (new_handle, result) = backend.apply_custom_unitary(handle, &unitary, dim_val)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                ctx.psw.sf = true;
                ctx.psw.ef = true;
                ctx.psw.inf = false;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QCz { dst, src, ctrl_qubit_reg, tgt_qubit_reg } => {
            let ctrl = ctx.iregs.get(*ctrl_qubit_reg)? as u8;
            let tgt = ctx.iregs.get(*tgt_qubit_reg)? as u8;

            if let Some(handle) = ctx.qregs[*src as usize] {
                if ctrl == tgt {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QCZ".to_string(),
                        detail: format!("ctrl ({}) == tgt ({})", ctrl, tgt),
                    });
                }
                let n = backend.num_qubits(handle)?;
                if ctrl >= n || tgt >= n {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QCZ".to_string(),
                        index: ctrl.max(tgt) as usize,
                        limit: n as usize,
                    });
                }

                let gate = cz_gate();
                let (new_handle, result) = backend.apply_two_qubit_gate(handle, ctrl, tgt, &gate)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                ctx.psw.sf = false;
                ctx.psw.ef = true;
                ctx.psw.inf = false;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QSwap { dst, src, qubit_a_reg, qubit_b_reg } => {
            let qubit_a = ctx.iregs.get(*qubit_a_reg)? as u8;
            let qubit_b = ctx.iregs.get(*qubit_b_reg)? as u8;

            if let Some(handle) = ctx.qregs[*src as usize] {
                if qubit_a == qubit_b {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QSWAP".to_string(),
                        detail: format!("qubit_a ({}) == qubit_b ({})", qubit_a, qubit_b),
                    });
                }
                let n = backend.num_qubits(handle)?;
                if qubit_a >= n || qubit_b >= n {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QSWAP".to_string(),
                        index: qubit_a.max(qubit_b) as usize,
                        limit: n as usize,
                    });
                }

                let gate = swap_gate();
                let (new_handle, result) = backend.apply_two_qubit_gate(handle, qubit_a, qubit_b, &gate)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                ctx.psw.sf = false;
                ctx.psw.ef = false;
                ctx.psw.inf = false;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QMixed { dst, base_addr_reg, count_reg } => {
            let base = ctx.iregs.get(*base_addr_reg)? as u16;
            let count = ctx.iregs.get(*count_reg)? as usize;

            let mut states: Vec<(f64, Vec<C64>)> = Vec::with_capacity(count);
            let mut addr = base;
            for _ in 0..count {
                let weight = f64::from_bits(ctx.cmem.load(addr) as u64);
                addr = addr.wrapping_add(1);
                let dim = ctx.cmem.load(addr) as usize;
                addr = addr.wrapping_add(1);
                let mut psi = Vec::with_capacity(dim);
                for _ in 0..dim {
                    let re = f64::from_bits(ctx.cmem.load(addr) as u64);
                    let im = f64::from_bits(ctx.cmem.load(addr.wrapping_add(1)) as u64);
                    psi.push(C64(re, im));
                    addr = addr.wrapping_add(2);
                }
                states.push((weight, psi));
            }

            let refs: Vec<(f64, &[C64])> = states.iter()
                .map(|(w, psi)| (*w, psi.as_slice()))
                .collect();

            let (handle, _result) = backend.prep_mixed(&refs)?;

            ctx.set_qreg(*dst, handle, backend);
            ctx.psw.qf = true;
            ctx.psw.sf = true;
            ctx.psw.ef = false;
            ctx.psw.inf = false;
            Ok(())
        }

        Instruction::QPtrace { dst, src, num_qubits_a_reg } => {
            let num_qubits_a = ctx.iregs.get(*num_qubits_a_reg)? as u8;

            if let Some(handle) = ctx.qregs[*src as usize] {
                let n = backend.num_qubits(handle)?;
                if num_qubits_a == 0 || num_qubits_a >= n {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QPTRACE".to_string(),
                        detail: format!(
                            "num_qubits_a must be 1..{}, got {}",
                            n, num_qubits_a
                        ),
                    });
                }

                let (new_handle, result) = backend.partial_trace(handle, num_qubits_a)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                ctx.psw.sf = false;
                ctx.psw.ef = false;
                ctx.psw.inf = false;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QReset { dst, src, qubit_reg } => {
            let qubit = ctx.iregs.get(*qubit_reg)? as u8;

            if let Some(handle) = ctx.take_qreg(*src) {
                let n = backend.num_qubits(handle)?;
                if qubit >= n {
                    ctx.qregs[*src as usize] = Some(handle);
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QRESET".to_string(),
                        index: qubit as usize,
                        limit: n as usize,
                    });
                }

                let (new_handle, result) = backend.reset_qubit(handle, qubit)?;

                ctx.set_qreg(*dst, new_handle, backend);
                ctx.psw.update_from_qmeta(result.purity, ctx.config.min_purity);
                ctx.psw.sf = false;
                ctx.psw.ef = false;
                ctx.psw.inf = false;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        _ => {
            Err(CqamError::TypeMismatch {
                instruction: format!("{:?}", instr),
                detail: "Invalid instruction passed to execute_qop".to_string(),
            })
        }
    }
}

// =============================================================================
// Proper masked gate implementation
// =============================================================================

/// Execute a masked single-qubit gate across selected qubits using the backend.
#[allow(clippy::too_many_arguments)]
fn execute_masked_gate_backend<B: QuantumBackend + ?Sized>(
    ctx: &mut ExecutionContext,
    backend: &mut B,
    dst: u8,
    src: u8,
    mask_reg: u8,
    gate_fn: fn() -> [C64; 4],
    _instr_name: &str,
    intent: (bool, bool, bool),
) -> Result<(), CqamError> {
    if let Some(handle) = ctx.qregs[src as usize] {
        let mask = ctx.iregs.get(mask_reg)? as u64;
        let n = backend.num_qubits(handle)?;
        let gate = gate_fn();

        let mut current_handle = handle;
        let mut last_purity = 1.0_f64;
        for qubit in 0..n {
            if (mask >> qubit) & 1 == 1 {
                let (new_handle, result) = backend.apply_single_gate(current_handle, qubit, &gate)?;
                if current_handle != handle {
                    backend.release(current_handle);
                }
                current_handle = new_handle;
                last_purity = result.purity;
            }
        }

        // If no gates were applied (mask was empty), current_handle == handle.
        // When dst == src and no gate was applied, the state is unchanged -- skip set_qreg
        // to avoid releasing the original handle and invalidating it.
        if current_handle == handle && dst == src {
            // No-op: nothing changed.
        } else if current_handle == handle && dst != src {
            // Need an independent copy for the destination.
            let cloned = backend.clone_state(handle)?;
            ctx.set_qreg(dst, cloned, backend);
        } else {
            ctx.set_qreg(dst, current_handle, backend);
        }
        ctx.psw.update_from_qmeta(last_purity, ctx.config.min_purity);
        ctx.psw.sf = intent.0;
        ctx.psw.ef = intent.1;
        ctx.psw.inf = intent.2;
        Ok(())
    } else {
        Err(CqamError::UninitializedRegister {
            file: "Q".to_string(),
            index: src,
        })
    }
}

// =============================================================================
// CMEM pre-reading for integer-context kernels
// =============================================================================

/// Pre-read CMEM data needed for integer-context kernels.
///
/// Returns a Vec<i64> of pre-read data. For kernels that don't need CMEM data,
/// returns an empty vec.
fn pre_read_cmem_int<B: QuantumBackend + ?Sized>(
    ctx: &ExecutionContext,
    kernel: KernelId,
    param0: i64,
    param1: i64,
    handle: cqam_core::quantum_backend::QRegHandle,
    backend: &B,
) -> Result<Vec<i64>, CqamError> {
    match kernel {
        KernelId::GroverIter => {
            let multi_addr = param1;
            if multi_addr == 0 {
                Ok(vec![])
            } else {
                // Multi-target: read count + targets from CMEM
                let base = multi_addr as u16;
                let count = ctx.cmem.load(base) as usize;
                let mut data = Vec::with_capacity(1 + count);
                data.push(count as i64);
                for i in 0..count {
                    let t = ctx.cmem.load(base.wrapping_add(1 + i as u16));
                    data.push(t);
                }
                Ok(data)
            }
        }
        KernelId::ControlledU => {
            // R[ctx0] = control qubit index
            // R[ctx1] = CMEM base address for 5-cell parameter block
            let base = param1 as u16;
            let sub_kernel_id_raw = ctx.cmem.load(base);
            let power = ctx.cmem.load(base.wrapping_add(1));
            let param_re_bits = ctx.cmem.load(base.wrapping_add(2));
            let param_im_bits = ctx.cmem.load(base.wrapping_add(3));
            let target_qubits = ctx.cmem.load(base.wrapping_add(4));

            let mut data = vec![sub_kernel_id_raw, power, param_re_bits, param_im_bits, target_qubits];

            // Check if sub-kernel needs CMEM data
            let sub_kernel_id = KernelId::try_from(sub_kernel_id_raw as u8)?;
            let tq = target_qubits as u8;
            match sub_kernel_id {
                KernelId::DiagonalUnitary => {
                    let sub_base = param_re_bits as u16; // CMEM[base+2] is the sub-data addr
                    let t = if tq == 0 {
                        backend.num_qubits(handle)? - 1
                    } else {
                        tq
                    };
                    let sub_dim = 1usize << t;
                    for k in 0..sub_dim {
                        let addr = sub_base.wrapping_add((2 * k) as u16);
                        data.push(ctx.cmem.load(addr));
                        data.push(ctx.cmem.load(addr.wrapping_add(1)));
                    }
                }
                KernelId::Permutation => {
                    let sub_base = param_re_bits as u16;
                    let t = if tq == 0 {
                        backend.num_qubits(handle)? - 1
                    } else {
                        tq
                    };
                    let sub_dim = 1usize << t;
                    for k in 0..sub_dim {
                        let addr = sub_base.wrapping_add(k as u16);
                        data.push(ctx.cmem.load(addr));
                    }
                }
                _ => {}
            }

            Ok(data)
        }
        KernelId::DiagonalUnitary => {
            // R[ctx0] = CMEM base, R[ctx1] = dimension
            let base = param0 as u16;
            let dim = param1 as usize;
            let mut data = Vec::with_capacity(dim * 2);
            for k in 0..dim {
                let addr = base.wrapping_add((2 * k) as u16);
                data.push(ctx.cmem.load(addr));
                data.push(ctx.cmem.load(addr.wrapping_add(1)));
            }
            Ok(data)
        }
        KernelId::Permutation => {
            // R[ctx0] = CMEM base
            let base = param0 as u16;
            let dim = backend.dimension(handle)?;
            let mut data = Vec::with_capacity(dim);
            for k in 0..dim {
                let addr = base.wrapping_add(k as u16);
                data.push(ctx.cmem.load(addr));
            }
            Ok(data)
        }
        _ => Ok(vec![]),
    }
}
