//! Quantum operation handlers for the CQAM virtual machine.
//!
//! Implements QPREP, QKERNEL, QOBSERVE, QLOAD, and QSTORE using the
//! `QuantumRegister` simulation backend from `cqam-sim`, which dispatches
//! to either Statevector (pure) or DensityMatrix (mixed) as appropriate.

use cqam_core::error::CqamError;
use cqam_core::instruction::{Instruction, dist_id, file_sel, kernel_id, observe_mode, rot_axis};
use cqam_core::register::HybridValue;
use cqam_sim::complex::{C64, ZERO, ONE};
use cqam_sim::density_matrix::DensityMatrix;
use cqam_sim::quantum_register::QuantumRegister;
use cqam_sim::kernel::Kernel;
use cqam_sim::kernels::init::Init;
use cqam_sim::kernels::entangle::Entangle;
use cqam_sim::kernels::fourier::Fourier;
use cqam_sim::kernels::diffuse::Diffuse;
use cqam_sim::kernels::grover::GroverIter;
use cqam_sim::kernels::rotate::Rotate;
use cqam_sim::kernels::phase::PhaseShift;
use cqam_sim::kernels::fourier_inv::FourierInv;
use cqam_sim::kernels::controlled_u::ControlledU;
use cqam_sim::kernels::diagonal::DiagonalUnitary;
use cqam_sim::kernels::permutation::Permutation;
use rand::Rng;
use crate::context::ExecutionContext;

// =============================================================================
// Gate matrices for masked register-level operations
// =============================================================================

/// Hadamard gate: (1/sqrt(2)) * [[1,1],[1,-1]]
fn hadamard() -> [C64; 4] {
    let h = std::f64::consts::FRAC_1_SQRT_2;
    [(h, 0.0), (h, 0.0), (h, 0.0), (-h, 0.0)]
}

/// Pauli-X (bit flip): [[0,1],[1,0]]
fn pauli_x() -> [C64; 4] {
    [ZERO, ONE, ONE, ZERO]
}

/// Pauli-Z (phase flip): [[1,0],[0,-1]]
fn pauli_z() -> [C64; 4] {
    [ONE, ZERO, ZERO, (-1.0, 0.0)]
}

/// CZ gate (4x4): diag(1, 1, 1, -1)
/// |00> -> |00>, |01> -> |01>, |10> -> |10>, |11> -> -|11>
fn cz_gate() -> [C64; 16] {
    [
        ONE,  ZERO, ZERO, ZERO,
        ZERO, ONE,  ZERO, ZERO,
        ZERO, ZERO, ONE,  ZERO,
        ZERO, ZERO, ZERO, (-1.0, 0.0),
    ]
}

/// SWAP gate (4x4): swaps |01> <-> |10>
/// [[1,0,0,0],[0,0,1,0],[0,1,0,0],[0,0,0,1]]
fn swap_gate() -> [C64; 16] {
    [
        ONE,  ZERO, ZERO, ZERO,
        ZERO, ZERO, ONE,  ZERO,
        ZERO, ONE,  ZERO, ZERO,
        ZERO, ZERO, ZERO, ONE,
    ]
}

/// CNOT gate (4x4): |00> -> |00>, |01> -> |01>, |10> -> |11>, |11> -> |10>
/// Matrix: [[1,0,0,0],[0,1,0,0],[0,0,0,1],[0,0,1,0]]
fn cnot_gate() -> [C64; 16] {
    [
        ONE,  ZERO, ZERO, ZERO,
        ZERO, ONE,  ZERO, ZERO,
        ZERO, ZERO, ZERO, ONE,
        ZERO, ZERO, ONE,  ZERO,
    ]
}

/// Build Rx(theta) = [[cos(t/2), -i*sin(t/2)], [-i*sin(t/2), cos(t/2)]]
fn rotation_x(theta: f64) -> [C64; 4] {
    let half = theta / 2.0;
    let c = half.cos();
    let s = half.sin();
    [
        (c, 0.0), (0.0, -s),
        (0.0, -s), (c, 0.0),
    ]
}

/// Build Ry(theta) = [[cos(t/2), -sin(t/2)], [sin(t/2), cos(t/2)]]
fn rotation_y(theta: f64) -> [C64; 4] {
    let half = theta / 2.0;
    let c = half.cos();
    let s = half.sin();
    [
        (c, 0.0), (-s, 0.0),
        (s, 0.0), (c, 0.0),
    ]
}

/// Build Rz(theta) = [[exp(-i*t/2), 0], [0, exp(i*t/2)]]
fn rotation_z(theta: f64) -> [C64; 4] {
    let half = theta / 2.0;
    [
        (half.cos(), -half.sin()), ZERO,
        ZERO, (half.cos(), half.sin()),
    ]
}

// =============================================================================
// Masked gate execution
// =============================================================================

/// Execute a masked single-qubit gate across selected qubits.
///
/// Reads the bitmask from R[mask_reg], iterates over qubits 0..num_qubits,
/// and applies the given gate to each qubit where the corresponding mask bit
/// is set.
fn execute_masked_gate(
    ctx: &mut ExecutionContext,
    dst: u8,
    src: u8,
    mask_reg: u8,
    gate_fn: fn() -> [C64; 4],
    _instr_name: &str,
) -> Result<(), CqamError> {
    if let Some(ref qr) = ctx.qregs[src as usize] {
        let mask = ctx.iregs.get(mask_reg)? as u64;
        let n = qr.num_qubits();
        let gate = gate_fn();

        let mut result = qr.clone();
        for qubit in 0..n {
            if (mask >> qubit) & 1 == 1 {
                result.apply_single_qubit_gate(qubit, &gate);
            }
        }

        let purity = result.purity();
        let entangled = result.is_entangled();
        let in_sup = result.is_in_superposition();

        ctx.qregs[dst as usize] = Some(result);
        ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
        Ok(())
    } else {
        Err(CqamError::UninitializedRegister {
            file: "Q".to_string(),
            index: src,
        })
    }
}

/// Execute a quantum instruction.
///
/// Returns `Ok(())` on success, or `Err(CqamError)` on runtime errors
/// (unknown kernel, uninitialized quantum register, etc.).
pub fn execute_qop(ctx: &mut ExecutionContext, instr: &Instruction) -> Result<(), CqamError> {
    match instr {
        Instruction::QPrep { dst, dist } => {
            let num_qubits = ctx.config.default_qubits;
            let force_dm = ctx.config.force_density_matrix;
            let qr = match *dist {
                dist_id::UNIFORM => QuantumRegister::new_uniform(num_qubits, force_dm),
                dist_id::ZERO => QuantumRegister::new_zero_state(num_qubits, force_dm),
                dist_id::BELL => QuantumRegister::new_bell(force_dm),
                dist_id::GHZ => QuantumRegister::new_ghz(num_qubits, force_dm)
                    .map_err(|e| CqamError::TypeMismatch {
                        instruction: "QPREP/GHZ".to_string(),
                        detail: e,
                    })?,
                _ => {
                    return Err(CqamError::UnknownDistribution(*dist));
                }
            };
            // BELL and GHZ are entangled by construction
            let entangled = matches!(*dist, dist_id::BELL | dist_id::GHZ);
            // UNIFORM, BELL, GHZ produce superposition; ZERO does not
            let in_sup = matches!(*dist, dist_id::UNIFORM | dist_id::BELL | dist_id::GHZ);
            ctx.qregs[*dst as usize] = Some(qr);
            ctx.psw.qf = true;
            ctx.psw.sf = in_sup;
            ctx.psw.ef = entangled;
            ctx.psw.clear_decoherence();
            ctx.psw.cf = false;
            Ok(())
        }

        Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
            let param0 = ctx.iregs.get(*ctx0)?;
            let param1 = ctx.iregs.get(*ctx1)?;

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                let k: Box<dyn Kernel> = match *kernel {
                    kernel_id::INIT => Box::new(Init),
                    kernel_id::ENTANGLE => Box::new(Entangle),
                    kernel_id::FOURIER => Box::new(Fourier),
                    kernel_id::DIFFUSE => Box::new(Diffuse),
                    kernel_id::GROVER_ITER => {
                        let target = param0 as u16;
                        let multi_addr = param1;

                        if multi_addr == 0 {
                            // Single-target mode (backward compatible)
                            Box::new(GroverIter::single(target))
                        } else {
                            // Multi-target mode: read target list from CMEM
                            let base = multi_addr as u16;
                            let count = ctx.cmem.load(base) as usize;
                            let mut targets = Vec::with_capacity(count);
                            for i in 0..count {
                                let t = ctx.cmem.load(base.wrapping_add(1 + i as u16)) as u16;
                                targets.push(t);
                            }
                            Box::new(GroverIter::multi(targets))
                        }
                    }
                    kernel_id::FOURIER_INV => Box::new(FourierInv),
                    kernel_id::CONTROLLED_U => {
                        // R[ctx0] = control qubit index
                        // R[ctx1] = CMEM base address for 5-cell parameter block
                        //   CMEM[base+0] = sub_kernel_id
                        //   CMEM[base+1] = power
                        //   CMEM[base+2] = param_re (f64 bits) or sub-data CMEM addr (i64)
                        //   CMEM[base+3] = param_im (f64 bits) or unused
                        //   CMEM[base+4] = target_qubits
                        let control_qubit = param0 as u8;
                        let base = param1 as u16;
                        let sub_kernel_id = ctx.cmem.load(base) as u8;
                        let power = ctx.cmem.load(base.wrapping_add(1)) as u32;
                        let param_re = f64::from_bits(ctx.cmem.load(base.wrapping_add(2)) as u64);
                        let param_im = f64::from_bits(ctx.cmem.load(base.wrapping_add(3)) as u64);
                        let target_qubits = ctx.cmem.load(base.wrapping_add(4)) as u8;

                        // For CMEM-dependent sub-kernels, pre-build from CMEM data.
                        // CMEM[base+2] holds the sub-data base address as a plain integer.
                        let sub_kernel_override: Option<Box<dyn cqam_sim::kernel::Kernel>> =
                            match sub_kernel_id {
                                kernel_id::DIAGONAL_UNITARY => {
                                    let sub_base = ctx.cmem.load(base.wrapping_add(2)) as u16;
                                    let t = if target_qubits == 0 {
                                        qr.num_qubits() - 1
                                    } else {
                                        target_qubits
                                    };
                                    let sub_dim = 1usize << t;
                                    let mut diagonal = Vec::with_capacity(sub_dim);
                                    for k in 0..sub_dim {
                                        let addr = sub_base.wrapping_add((2 * k) as u16);
                                        let re = f64::from_bits(ctx.cmem.load(addr) as u64);
                                        let im = f64::from_bits(ctx.cmem.load(addr.wrapping_add(1)) as u64);
                                        diagonal.push((re, im));
                                    }
                                    Some(Box::new(DiagonalUnitary { diagonal }))
                                }
                                kernel_id::PERMUTATION => {
                                    let sub_base = ctx.cmem.load(base.wrapping_add(2)) as u16;
                                    let t = if target_qubits == 0 {
                                        qr.num_qubits() - 1
                                    } else {
                                        target_qubits
                                    };
                                    let sub_dim = 1usize << t;
                                    let mut table = Vec::with_capacity(sub_dim);
                                    for k in 0..sub_dim {
                                        let addr = sub_base.wrapping_add(k as u16);
                                        table.push(ctx.cmem.load(addr) as usize);
                                    }
                                    let perm = Permutation::new(table).map_err(|e| {
                                        CqamError::TypeMismatch {
                                            instruction: "QKERNEL/CONTROLLED_U(PERMUTATION)".to_string(),
                                            detail: e,
                                        }
                                    })?;
                                    Some(Box::new(perm))
                                }
                                _ => None,
                            };

                        Box::new(ControlledU {
                            control_qubit,
                            sub_kernel_id,
                            power,
                            param_re,
                            param_im,
                            target_qubits,
                            sub_kernel_override,
                        })
                    }
                    kernel_id::DIAGONAL_UNITARY => {
                        // R[ctx0] = CMEM base address for diagonal entries
                        // R[ctx1] = dimension (must equal Q[src].dimension())
                        let base = param0 as u16;
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
                        // Read diagonal entries from CMEM
                        let mut diagonal = Vec::with_capacity(dim);
                        for k in 0..dim {
                            let addr = base.wrapping_add((2 * k) as u16);
                            let re = f64::from_bits(ctx.cmem.load(addr) as u64);
                            let im = f64::from_bits(ctx.cmem.load(addr.wrapping_add(1)) as u64);
                            diagonal.push((re, im));
                        }
                        Box::new(DiagonalUnitary { diagonal })
                    }
                    kernel_id::PERMUTATION => {
                        // R[ctx0] = CMEM base address for permutation table
                        // R[ctx1] = unused (dimension inferred from register)
                        let base = param0 as u16;
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

                        // Read permutation table from CMEM: dim entries, each a plain i64
                        let mut table = Vec::with_capacity(dim);
                        for k in 0..dim {
                            let addr = base.wrapping_add(k as u16);
                            let val = ctx.cmem.load(addr);
                            table.push(val as usize);
                        }

                        // Construct and validate permutation
                        let perm = Permutation::new(table).map_err(|e| {
                            CqamError::TypeMismatch {
                                instruction: "QKERNEL/PERMUTATION".to_string(),
                                detail: e,
                            }
                        })?;
                        Box::new(perm)
                    }
                    _ => {
                        return Err(CqamError::UnknownKernel(
                            format!("Unknown kernel ID: {}", kernel),
                        ));
                    }
                };

                let result = qr.apply_kernel(k.as_ref())?;

                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
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

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                let k: Box<dyn Kernel> = match *kernel {
                    kernel_id::INIT => Box::new(Init),
                    kernel_id::ENTANGLE => Box::new(Entangle),
                    kernel_id::FOURIER => Box::new(Fourier),
                    kernel_id::DIFFUSE => Box::new(Diffuse),
                    kernel_id::GROVER_ITER => {
                        let target = fparam0 as u16;
                        Box::new(GroverIter::single(target))
                    }
                    kernel_id::ROTATE => Box::new(Rotate { theta: fparam0 }),
                    kernel_id::PHASE_SHIFT => Box::new(PhaseShift { amplitude: (fparam0, 0.0) }),
                    kernel_id::FOURIER_INV => Box::new(FourierInv),
                    kernel_id::CONTROLLED_U => {
                        // QKernelF shorthand: F[fctx0] = control qubit, F[fctx1] = theta
                        // Controlled-ROTATE with power=0, all target qubits
                        Box::new(ControlledU {
                            control_qubit: fparam0 as u8,
                            sub_kernel_id: kernel_id::ROTATE,
                            power: 0,
                            param_re: fparam1,
                            param_im: 0.0,
                            target_qubits: 0,
                            sub_kernel_override: None,
                        })
                    }
                    _ => {
                        return Err(CqamError::UnknownKernel(
                            format!("Unknown kernel ID: {}", kernel),
                        ));
                    }
                };

                let result = qr.apply_kernel(k.as_ref())?;
                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
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

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                let k: Box<dyn Kernel> = match *kernel {
                    kernel_id::INIT => Box::new(Init),
                    kernel_id::ENTANGLE => Box::new(Entangle),
                    kernel_id::FOURIER => Box::new(Fourier),
                    kernel_id::DIFFUSE => Box::new(Diffuse),
                    kernel_id::GROVER_ITER => {
                        let target = zparam0.0 as u16;
                        Box::new(GroverIter::single(target))
                    }
                    kernel_id::ROTATE => Box::new(Rotate { theta: zparam0.0 }),
                    kernel_id::PHASE_SHIFT => Box::new(PhaseShift { amplitude: zparam0 }),
                    kernel_id::FOURIER_INV => Box::new(FourierInv),
                    kernel_id::CONTROLLED_U => {
                        // QKernelZ: Z[zctx0] = (control_qubit, sub_kernel_id)
                        //           Z[zctx1] = (param_re, param_im)
                        Box::new(ControlledU {
                            control_qubit: zparam0.0 as u8,
                            sub_kernel_id: zparam0.1 as u8,
                            power: 0,
                            param_re: zparam1.0,
                            param_im: zparam1.1,
                            target_qubits: 0,
                            sub_kernel_override: None,
                        })
                    }
                    _ => {
                        return Err(CqamError::UnknownKernel(
                            format!("Unknown kernel ID: {}", kernel),
                        ));
                    }
                };

                let result = qr.apply_kernel(k.as_ref())?;
                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 } => {
            if let Some(qr) = ctx.qregs[*src_q as usize].take() {
                let hval = match *mode {
                    observe_mode::DIST => {
                        let probs = qr.diagonal_probabilities();
                        let dist_pairs: Vec<(u16, f64)> = probs.iter().enumerate()
                            .filter(|(_, p)| **p >= 1e-15)
                            .map(|(k, p)| (k as u16, *p))
                            .collect();
                        HybridValue::Dist(dist_pairs)
                    }
                    observe_mode::PROB => {
                        let index = ctx.iregs.get(*ctx0)? as usize;
                        let dim = qr.dimension();
                        if index >= dim {
                            return Err(CqamError::QuantumIndexOutOfRange {
                                instruction: "QOBSERVE/PROB".to_string(),
                                index,
                                limit: dim,
                            });
                        }
                        let prob = qr.get_element(index, index).0;
                        HybridValue::Complex(prob, 0.0)
                    }
                    observe_mode::AMP => {
                        let row = ctx.iregs.get(*ctx0)? as usize;
                        let col = ctx.iregs.get(*ctx1)? as usize;
                        let dim = qr.dimension();
                        if row >= dim || col >= dim {
                            return Err(CqamError::QuantumIndexOutOfRange {
                                instruction: "QOBSERVE/AMP".to_string(),
                                index: row.max(col),
                                limit: dim,
                            });
                        }
                        let (re, im) = qr.get_element(row, col);
                        HybridValue::Complex(re, im)
                    }
                    observe_mode::SAMPLE => {
                        let probs = qr.diagonal_probabilities();
                        let r: f64 = ctx.rng.gen_range(0.0..1.0);
                        let mut cumulative = 0.0;
                        let mut outcome = (probs.len() - 1) as i64;
                        for (k, p) in probs.iter().enumerate() {
                            cumulative += p;
                            if r < cumulative {
                                outcome = k as i64;
                                break;
                            }
                        }
                        ctx.psw.zf = outcome == 0;
                        HybridValue::Int(outcome)
                    }
                    _ => {
                        return Err(CqamError::TypeMismatch {
                            instruction: "QOBSERVE".to_string(),
                            detail: format!("unknown mode: {}", mode),
                        });
                    }
                };
                ctx.hregs.set(*dst_h, hval)?;
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
            if let Some(ref qr) = ctx.qregs[*src_q as usize] {
                let hval = match *mode {
                    observe_mode::DIST => {
                        let probs = qr.diagonal_probabilities();
                        let dist_pairs: Vec<(u16, f64)> = probs.iter().enumerate()
                            .filter(|(_, p)| **p >= 1e-15)
                            .map(|(k, p)| (k as u16, *p))
                            .collect();
                        HybridValue::Dist(dist_pairs)
                    }
                    observe_mode::PROB => {
                        let index = ctx.iregs.get(*ctx0)? as usize;
                        let dim = qr.dimension();
                        if index >= dim {
                            return Err(CqamError::QuantumIndexOutOfRange {
                                instruction: "QSAMPLE/PROB".to_string(),
                                index,
                                limit: dim,
                            });
                        }
                        let prob = qr.get_element(index, index).0;
                        HybridValue::Complex(prob, 0.0)
                    }
                    observe_mode::AMP => {
                        let row = ctx.iregs.get(*ctx0)? as usize;
                        let col = ctx.iregs.get(*ctx1)? as usize;
                        let dim = qr.dimension();
                        if row >= dim || col >= dim {
                            return Err(CqamError::QuantumIndexOutOfRange {
                                instruction: "QSAMPLE/AMP".to_string(),
                                index: row.max(col),
                                limit: dim,
                            });
                        }
                        let (re, im) = qr.get_element(row, col);
                        HybridValue::Complex(re, im)
                    }
                    _ => {
                        return Err(CqamError::TypeMismatch {
                            instruction: "QSAMPLE".to_string(),
                            detail: format!("unknown mode: {}", mode),
                        });
                    }
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
            if let Some(qr) = ctx.qmem.load(*addr) {
                let entangled = qr.is_entangled();
                ctx.qregs[*dst_q as usize] = Some(qr.clone());
                ctx.psw.qf = true;
                ctx.psw.ef = entangled;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "QMEM".to_string(),
                    index: *addr,
                })
            }
        }

        Instruction::QStore { src_q, addr } => {
            if let Some(ref qr) = ctx.qregs[*src_q as usize] {
                ctx.qmem.store(*addr, qr.clone());
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
                })
            }
        }

        Instruction::QPrepR { dst, dist_reg } => {
            let dist_id_val = ctx.iregs.get(*dist_reg)? as u8;
            let num_qubits = ctx.config.default_qubits;
            let force_dm = ctx.config.force_density_matrix;
            let qr = match dist_id_val {
                dist_id::UNIFORM => QuantumRegister::new_uniform(num_qubits, force_dm),
                dist_id::ZERO => QuantumRegister::new_zero_state(num_qubits, force_dm),
                dist_id::BELL => QuantumRegister::new_bell(force_dm),
                dist_id::GHZ => QuantumRegister::new_ghz(num_qubits, force_dm)
                    .map_err(|e| CqamError::TypeMismatch {
                        instruction: "QPREPR/GHZ".to_string(),
                        detail: e,
                    })?,
                _ => {
                    return Err(CqamError::UnknownDistribution(dist_id_val));
                }
            };
            let entangled = matches!(dist_id_val, dist_id::BELL | dist_id::GHZ);
            let in_sup = matches!(dist_id_val, dist_id::UNIFORM | dist_id::BELL | dist_id::GHZ);
            ctx.qregs[*dst as usize] = Some(qr);
            ctx.psw.qf = true;
            ctx.psw.sf = in_sup;
            ctx.psw.ef = entangled;
            ctx.psw.clear_decoherence();
            ctx.psw.cf = false;
            Ok(())
        }

        Instruction::QHadM { dst, src, mask_reg } => {
            execute_masked_gate(ctx, *dst, *src, *mask_reg, hadamard, "QHADM")
        }

        Instruction::QFlip { dst, src, mask_reg } => {
            execute_masked_gate(ctx, *dst, *src, *mask_reg, pauli_x, "QFLIP")
        }

        Instruction::QPhase { dst, src, mask_reg } => {
            execute_masked_gate(ctx, *dst, *src, *mask_reg, pauli_z, "QPHASE")
        }

        Instruction::QEncode { dst, src_base, count, file_sel: fs } => {
            let count_val = *count as usize;

            // Pre-validate: count must be > 0 and a power of 2
            if count_val == 0 || (count_val & (count_val - 1)) != 0 {
                return Err(CqamError::TypeMismatch {
                    instruction: "QENCODE".to_string(),
                    detail: format!(
                        "count must be a power of 2, got {}",
                        count_val
                    ),
                });
            }

            // Build amplitude vector from the selected register file
            let mut psi: Vec<(f64, f64)> = Vec::with_capacity(count_val);
            for i in 0..count_val {
                let reg_idx = src_base + i as u8;
                let amplitude: (f64, f64) = match *fs {
                    file_sel::R_FILE => {
                        let val = ctx.iregs.get(reg_idx)?;
                        (val as f64, 0.0)
                    }
                    file_sel::F_FILE => {
                        let val = ctx.fregs.get(reg_idx)?;
                        (val, 0.0)
                    }
                    file_sel::Z_FILE => {
                        ctx.zregs.get(reg_idx)?
                    }
                    _ => {
                        return Err(CqamError::TypeMismatch {
                            instruction: "QENCODE".to_string(),
                            detail: format!("invalid file_sel: {}", fs),
                        });
                    }
                };
                psi.push(amplitude);
            }

            // Validate statevector is not all-zero
            let norm_sq: f64 = psi.iter()
                .map(|(re, im)| re * re + im * im)
                .sum();
            if norm_sq < 1e-30 {
                return Err(CqamError::TypeMismatch {
                    instruction: "QENCODE".to_string(),
                    detail: "statevector has zero norm".to_string(),
                });
            }

            // Delegate to QuantumRegister::from_amplitudes (always Pure)
            let qr = QuantumRegister::from_amplitudes(psi).map_err(|e| {
                CqamError::TypeMismatch {
                    instruction: "QENCODE".to_string(),
                    detail: e,
                }
            })?;

            let entangled = qr.is_entangled();
            ctx.qregs[*dst as usize] = Some(qr);
            ctx.psw.qf = true;
            ctx.psw.ef = entangled;
            Ok(())
        }

        Instruction::QCnot { dst, src, ctrl_qubit_reg, tgt_qubit_reg } => {
            let ctrl = ctx.iregs.get(*ctrl_qubit_reg)? as u8;
            let tgt = ctx.iregs.get(*tgt_qubit_reg)? as u8;

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                if ctrl == tgt {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QCNOT".to_string(),
                        detail: format!("ctrl ({}) == tgt ({})", ctrl, tgt),
                    });
                }
                if ctrl >= qr.num_qubits() || tgt >= qr.num_qubits() {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QCNOT".to_string(),
                        index: ctrl.max(tgt) as usize,
                        limit: qr.num_qubits() as usize,
                    });
                }

                let gate = cnot_gate();
                let mut result = qr.clone();
                result.apply_two_qubit_gate(ctrl, tgt, &gate);

                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
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

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                if qubit >= qr.num_qubits() {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QROT".to_string(),
                        index: qubit as usize,
                        limit: qr.num_qubits() as usize,
                    });
                }

                let gate = match *axis {
                    rot_axis::X => rotation_x(theta),
                    rot_axis::Y => rotation_y(theta),
                    rot_axis::Z => rotation_z(theta),
                    _ => {
                        return Err(CqamError::TypeMismatch {
                            instruction: "QROT".to_string(),
                            detail: format!("unknown axis: {}", axis),
                        });
                    }
                };

                let mut result = qr.clone();
                result.apply_single_qubit_gate(qubit, &gate);

                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
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

            if let Some(qr) = ctx.qregs[*src_q as usize].take() {
                if qubit >= qr.num_qubits() {
                    let nq = qr.num_qubits();
                    ctx.qregs[*src_q as usize] = Some(qr);
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QMEAS".to_string(),
                        index: qubit as usize,
                        limit: nq as usize,
                    });
                }

                let (outcome, post_qr) = qr.measure_qubit_with_rng(qubit, &mut ctx.rng);

                let purity = post_qr.purity();
                let entangled = post_qr.is_entangled();
                let in_sup = post_qr.is_in_superposition();
                ctx.iregs.set(*dst_r, outcome as i64)?;
                ctx.qregs[*src_q as usize] = Some(post_qr);

                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
                ctx.psw.mark_decohered();
                ctx.psw.mark_collapsed();
                ctx.psw.zf = outcome == 0;
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
                })
            }
        }

        Instruction::QTensor { dst, src0, src1 } => {
            let qr0 = ctx.qregs[*src0 as usize].take().ok_or_else(|| {
                CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src0,
                }
            })?;
            let qr1 = ctx.qregs[*src1 as usize].take().ok_or_else(|| {
                CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src1,
                }
            })?;

            let result = qr0.tensor_product(&qr1).map_err(|_e| {
                CqamError::QubitLimitExceeded {
                    instruction: "QTENSOR".to_string(),
                    required: qr0.num_qubits() + qr1.num_qubits(),
                    max: if ctx.config.force_density_matrix {
                        cqam_sim::density_matrix::MAX_QUBITS
                    } else {
                        cqam_sim::statevector::MAX_SV_QUBITS
                    },
                }
            })?;

            let purity = result.purity();
            let entangled = result.is_entangled();
            let in_sup = result.is_in_superposition();

            ctx.qregs[*dst as usize] = Some(result);
            ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
            Ok(())
        }

        Instruction::QCustom { dst, src, base_addr_reg, dim_reg } => {
            let base_addr = ctx.iregs.get(*base_addr_reg)? as u16;
            let dim_val = ctx.iregs.get(*dim_reg)? as usize;

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                let qr_dim = qr.dimension();
                if dim_val != qr_dim {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QCUSTOM".to_string(),
                        detail: format!("dim_reg={} but Q[src] dimension={}", dim_val, qr_dim),
                    });
                }

                // Read unitary from CMEM: 2 * dim * dim cells (re, im pairs)
                let mut unitary = Vec::with_capacity(dim_val * dim_val);
                for idx in 0..dim_val * dim_val {
                    let addr = base_addr.wrapping_add((2 * idx) as u16);
                    let re = f64::from_bits(ctx.cmem.load(addr) as u64);
                    let im = f64::from_bits(ctx.cmem.load(addr.wrapping_add(1)) as u64);
                    unitary.push((re, im));
                }

                // Validate unitarity: U^dagger * U ~= I
                let tol = 1e-6;
                for i in 0..dim_val {
                    for j in 0..dim_val {
                        // (U^dagger * U)[i][j] = sum_k conj(U[k][i]) * U[k][j]
                        let mut re_sum = 0.0_f64;
                        let mut im_sum = 0.0_f64;
                        for k in 0..dim_val {
                            let (a_re, a_im) = unitary[k * dim_val + i];
                            let (b_re, b_im) = unitary[k * dim_val + j];
                            // conj(a) * b = (a_re - a_im*i)(b_re + b_im*i)
                            re_sum += a_re * b_re + a_im * b_im;
                            im_sum += a_re * b_im - a_im * b_re;
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

                let mut result = qr.clone();
                result.apply_unitary(&unitary);

                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
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

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                if ctrl == tgt {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QCZ".to_string(),
                        detail: format!("ctrl ({}) == tgt ({})", ctrl, tgt),
                    });
                }
                if ctrl >= qr.num_qubits() || tgt >= qr.num_qubits() {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QCZ".to_string(),
                        index: ctrl.max(tgt) as usize,
                        limit: qr.num_qubits() as usize,
                    });
                }

                let gate = cz_gate();
                let mut result = qr.clone();
                result.apply_two_qubit_gate(ctrl, tgt, &gate);

                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
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

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                if qubit_a == qubit_b {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QSWAP".to_string(),
                        detail: format!("qubit_a ({}) == qubit_b ({})", qubit_a, qubit_b),
                    });
                }
                if qubit_a >= qr.num_qubits() || qubit_b >= qr.num_qubits() {
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QSWAP".to_string(),
                        index: qubit_a.max(qubit_b) as usize,
                        limit: qr.num_qubits() as usize,
                    });
                }

                let gate = swap_gate();
                let mut result = qr.clone();
                result.apply_two_qubit_gate(qubit_a, qubit_b, &gate);

                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        // -- QMIXED: build a mixed quantum state from a weighted ensemble in CMEM --
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
                    psi.push((re, im));
                    addr = addr.wrapping_add(2);
                }
                states.push((weight, psi));
            }

            let refs: Vec<(f64, &[C64])> = states.iter()
                .map(|(w, psi)| (*w, psi.as_slice()))
                .collect();

            let dm = DensityMatrix::from_mixture(&refs).map_err(|e| {
                CqamError::TypeMismatch {
                    instruction: "QMIXED".to_string(),
                    detail: e,
                }
            })?;

            let entangled = dm.is_any_qubit_entangled();
            let in_sup = dm.is_in_superposition();
            ctx.qregs[*dst as usize] = Some(QuantumRegister::Mixed(dm));
            ctx.psw.qf = true;
            ctx.psw.sf = in_sup;
            ctx.psw.ef = entangled;
            Ok(())
        }

        // -- QPREPN: prepare a quantum register with a runtime-specified qubit count --
        Instruction::QPrepN { dst, dist, qubit_count_reg } => {
            let num_qubits = ctx.iregs.get(*qubit_count_reg)? as u8;
            let force_dm = ctx.config.force_density_matrix;

            // Validate against the appropriate max for the chosen backend
            let max_qubits = if force_dm {
                cqam_sim::density_matrix::MAX_QUBITS
            } else {
                cqam_sim::statevector::MAX_SV_QUBITS
            };

            if num_qubits == 0 || num_qubits > max_qubits {
                return Err(CqamError::QubitLimitExceeded {
                    instruction: "QPREPN".to_string(),
                    required: num_qubits,
                    max: max_qubits,
                });
            }

            let qr = match *dist {
                dist_id::UNIFORM => QuantumRegister::new_uniform(num_qubits, force_dm),
                dist_id::ZERO => QuantumRegister::new_zero_state(num_qubits, force_dm),
                dist_id::BELL => QuantumRegister::new_bell(force_dm),
                dist_id::GHZ => QuantumRegister::new_ghz(num_qubits, force_dm)
                    .map_err(|e| CqamError::TypeMismatch {
                        instruction: "QPREPN/GHZ".to_string(),
                        detail: e,
                    })?,
                _ => {
                    return Err(CqamError::UnknownDistribution(*dist));
                }
            };
            let entangled = matches!(*dist, dist_id::BELL | dist_id::GHZ);
            let in_sup = matches!(*dist, dist_id::UNIFORM | dist_id::BELL | dist_id::GHZ);
            ctx.qregs[*dst as usize] = Some(qr);
            ctx.psw.qf = true;
            ctx.psw.sf = in_sup;
            ctx.psw.ef = entangled;
            ctx.psw.clear_decoherence();
            ctx.psw.cf = false;
            Ok(())
        }

        // -- QPTRACE: reduce a composite system to subsystem A via partial trace --
        Instruction::QPtrace { dst, src, num_qubits_a_reg } => {
            let num_qubits_a = ctx.iregs.get(*num_qubits_a_reg)? as u8;

            if let Some(ref qr) = ctx.qregs[*src as usize] {
                if num_qubits_a == 0 || num_qubits_a >= qr.num_qubits() {
                    return Err(CqamError::TypeMismatch {
                        instruction: "QPTRACE".to_string(),
                        detail: format!(
                            "num_qubits_a must be 1..{}, got {}",
                            qr.num_qubits(), num_qubits_a
                        ),
                    });
                }

                let result = qr.partial_trace_b(num_qubits_a).map_err(|e| {
                    CqamError::TypeMismatch {
                        instruction: "QPTRACE".to_string(),
                        detail: e,
                    }
                })?;

                let purity = result.purity();
                let entangled = result.is_entangled();
                let in_sup = result.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        // -- QRESET: measure a qubit and conditionally flip it back to |0> --
        Instruction::QReset { dst, src, qubit_reg } => {
            let qubit = ctx.iregs.get(*qubit_reg)? as u8;

            if let Some(qr) = ctx.qregs[*src as usize].take() {
                if qubit >= qr.num_qubits() {
                    let nq = qr.num_qubits();
                    ctx.qregs[*src as usize] = Some(qr);
                    return Err(CqamError::QuantumIndexOutOfRange {
                        instruction: "QRESET".to_string(),
                        index: qubit as usize,
                        limit: nq as usize,
                    });
                }

                let (outcome, mut post_qr) = qr.measure_qubit_with_rng(qubit, &mut ctx.rng);

                if outcome == 1 {
                    let x_gate = pauli_x();
                    post_qr.apply_single_qubit_gate(qubit, &x_gate);
                }

                let purity = post_qr.purity();
                let entangled = post_qr.is_entangled();
                let in_sup = post_qr.is_in_superposition();

                ctx.qregs[*dst as usize] = Some(post_qr);
                ctx.psw.update_from_qmeta(purity, ctx.config.min_purity, entangled, in_sup);
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
