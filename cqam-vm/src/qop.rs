//! Quantum operation handlers for the CQAM virtual machine.
//!
//! Implements QPREP, QKERNEL, QOBSERVE, QLOAD, and QSTORE using the
//! `DensityMatrix` simulation backend from `cqam-sim`.

use cqam_core::error::CqamError;
use cqam_core::instruction::{Instruction, dist_id, kernel_id, observe_mode};
use cqam_core::register::HybridValue;
use cqam_sim::density_matrix::DensityMatrix;
use cqam_sim::kernel::Kernel;
use cqam_sim::kernels::init::Init;
use cqam_sim::kernels::entangle::Entangle;
use cqam_sim::kernels::fourier::Fourier;
use cqam_sim::kernels::diffuse::Diffuse;
use cqam_sim::kernels::grover::GroverIter;
use cqam_sim::kernels::rotate::Rotate;
use cqam_sim::kernels::phase::PhaseShift;
use crate::context::ExecutionContext;

/// Execute a quantum instruction.
///
/// Returns `Ok(())` on success, or `Err(CqamError)` on runtime errors
/// (unknown kernel, uninitialized quantum register, etc.).
pub fn execute_qop(ctx: &mut ExecutionContext, instr: &Instruction) -> Result<(), CqamError> {
    match instr {
        Instruction::QPrep { dst, dist } => {
            let num_qubits = ctx.config.default_qubits;
            let dm = match *dist {
                dist_id::UNIFORM => DensityMatrix::new_uniform(num_qubits),
                dist_id::ZERO => DensityMatrix::new_zero_state(num_qubits),
                dist_id::BELL => DensityMatrix::new_bell(),
                dist_id::GHZ => DensityMatrix::new_ghz(num_qubits),
                _ => {
                    return Err(CqamError::UnknownDistribution(*dist));
                }
            };
            ctx.qregs[*dst as usize] = Some(dm);
            Ok(())
        }

        Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
            let param0 = ctx.iregs.get(*ctx0)?;
            let param1 = ctx.iregs.get(*ctx1)?;
            let _ = param1;

            if let Some(ref dm) = ctx.qregs[*src as usize] {
                let k: Box<dyn Kernel> = match *kernel {
                    kernel_id::INIT => Box::new(Init),
                    kernel_id::ENTANGLE => Box::new(Entangle),
                    kernel_id::FOURIER => Box::new(Fourier),
                    kernel_id::DIFFUSE => Box::new(Diffuse),
                    kernel_id::GROVER_ITER => {
                        let target = param0 as u16;
                        Box::new(GroverIter { target })
                    }
                    _ => {
                        return Err(CqamError::UnknownKernel(
                            format!("Unknown kernel ID: {}", kernel),
                        ));
                    }
                };

                let result = k.apply(dm);

                // Compute metrics from density matrix
                let superposition = result.von_neumann_entropy();
                let purity = result.purity();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(
                    superposition,
                    purity,
                    (ctx.config.min_superposition, ctx.config.min_entanglement),
                );
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
            let _ = fparam1; // reserved for future use

            if let Some(ref dm) = ctx.qregs[*src as usize] {
                let k: Box<dyn Kernel> = match *kernel {
                    kernel_id::INIT => Box::new(Init),
                    kernel_id::ENTANGLE => Box::new(Entangle),
                    kernel_id::FOURIER => Box::new(Fourier),
                    kernel_id::DIFFUSE => Box::new(Diffuse),
                    kernel_id::GROVER_ITER => {
                        let target = fparam0 as u16;
                        Box::new(GroverIter { target })
                    }
                    kernel_id::ROTATE => Box::new(Rotate { theta: fparam0 }),
                    kernel_id::PHASE_SHIFT => Box::new(PhaseShift { amplitude: (fparam0, 0.0) }),
                    _ => {
                        return Err(CqamError::UnknownKernel(
                            format!("Unknown kernel ID: {}", kernel),
                        ));
                    }
                };

                let result = k.apply(dm);
                let superposition = result.von_neumann_entropy();
                let purity = result.purity();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(
                    superposition,
                    purity,
                    (ctx.config.min_superposition, ctx.config.min_entanglement),
                );
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
            let _ = zparam1; // reserved for future use

            if let Some(ref dm) = ctx.qregs[*src as usize] {
                let k: Box<dyn Kernel> = match *kernel {
                    kernel_id::INIT => Box::new(Init),
                    kernel_id::ENTANGLE => Box::new(Entangle),
                    kernel_id::FOURIER => Box::new(Fourier),
                    kernel_id::DIFFUSE => Box::new(Diffuse),
                    kernel_id::GROVER_ITER => {
                        let target = zparam0.0 as u16;
                        Box::new(GroverIter { target })
                    }
                    kernel_id::ROTATE => Box::new(Rotate { theta: zparam0.0 }),
                    kernel_id::PHASE_SHIFT => Box::new(PhaseShift { amplitude: zparam0 }),
                    _ => {
                        return Err(CqamError::UnknownKernel(
                            format!("Unknown kernel ID: {}", kernel),
                        ));
                    }
                };

                let result = k.apply(dm);
                let superposition = result.von_neumann_entropy();
                let purity = result.purity();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(
                    superposition,
                    purity,
                    (ctx.config.min_superposition, ctx.config.min_entanglement),
                );
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src,
                })
            }
        }

        Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 } => {
            if let Some(dm) = ctx.qregs[*src_q as usize].take() {
                let hval = match *mode {
                    observe_mode::DIST => {
                        let probs = dm.diagonal_probabilities();
                        let dist_pairs: Vec<(u16, f64)> = probs.iter().enumerate()
                            .filter(|(_, p)| **p >= 1e-15)
                            .map(|(k, p)| (k as u16, *p))
                            .collect();
                        HybridValue::Dist(dist_pairs)
                    }
                    observe_mode::PROB => {
                        let index = ctx.iregs.get(*ctx0)? as usize;
                        let dim = dm.dimension();
                        if index >= dim {
                            return Err(CqamError::AddressOutOfRange {
                                instruction: "QOBSERVE/PROB".to_string(),
                                address: index as i64,
                            });
                        }
                        let prob = dm.get(index, index).0;
                        HybridValue::Float(prob)
                    }
                    observe_mode::AMP => {
                        let row = ctx.iregs.get(*ctx0)? as usize;
                        let col = ctx.iregs.get(*ctx1)? as usize;
                        let dim = dm.dimension();
                        if row >= dim || col >= dim {
                            return Err(CqamError::AddressOutOfRange {
                                instruction: "QOBSERVE/AMP".to_string(),
                                address: row.max(col) as i64,
                            });
                        }
                        let (re, im) = dm.get(row, col);
                        HybridValue::Complex(re, im)
                    }
                    _ => {
                        return Err(CqamError::TypeMismatch {
                            instruction: "QOBSERVE".to_string(),
                            detail: format!("unknown mode: {}", mode),
                        });
                    }
                };
                ctx.hregs.set(*dst_h, hval)?;
                ctx.psw.mark_measured();
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
                })
            }
        }

        Instruction::QSample { dst_h, src_q, mode, ctx0, ctx1 } => {
            if let Some(ref dm) = ctx.qregs[*src_q as usize] {
                let hval = match *mode {
                    observe_mode::DIST => {
                        let probs = dm.diagonal_probabilities();
                        let dist_pairs: Vec<(u16, f64)> = probs.iter().enumerate()
                            .filter(|(_, p)| **p >= 1e-15)
                            .map(|(k, p)| (k as u16, *p))
                            .collect();
                        HybridValue::Dist(dist_pairs)
                    }
                    observe_mode::PROB => {
                        let index = ctx.iregs.get(*ctx0)? as usize;
                        let dim = dm.dimension();
                        if index >= dim {
                            return Err(CqamError::AddressOutOfRange {
                                instruction: "QSAMPLE/PROB".to_string(),
                                address: index as i64,
                            });
                        }
                        let prob = dm.get(index, index).0;
                        HybridValue::Float(prob)
                    }
                    observe_mode::AMP => {
                        let row = ctx.iregs.get(*ctx0)? as usize;
                        let col = ctx.iregs.get(*ctx1)? as usize;
                        let dim = dm.dimension();
                        if row >= dim || col >= dim {
                            return Err(CqamError::AddressOutOfRange {
                                instruction: "QSAMPLE/AMP".to_string(),
                                address: row.max(col) as i64,
                            });
                        }
                        let (re, im) = dm.get(row, col);
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
            if let Some(dm) = ctx.qmem.load(*addr) {
                ctx.qregs[*dst_q as usize] = Some(dm.clone());
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "QMEM".to_string(),
                    index: *addr,
                })
            }
        }

        Instruction::QStore { src_q, addr } => {
            if let Some(ref dm) = ctx.qregs[*src_q as usize] {
                ctx.qmem.store(*addr, dm.clone());
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
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
