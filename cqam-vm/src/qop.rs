// cqam-vm/src/qop.rs
//
// Phase 4: Quantum operation handlers returning Result<(), CqamError>.

use cqam_core::error::CqamError;
use cqam_core::instruction::{Instruction, dist_id, kernel_id};
use cqam_core::register::HybridValue;
use cqam_sim::qdist::{QDist, Measurable};
use cqam_sim::kernel::Kernel;
use cqam_sim::kernels::init::InitDist;
use cqam_sim::kernels::entangle::Entangle;
use cqam_sim::kernels::fourier::Fourier;
use cqam_sim::kernels::diffuse::Diffuse;
use cqam_sim::kernels::grover::GroverIter;
use crate::context::ExecutionContext;

/// Execute a quantum instruction.
///
/// Returns `Ok(())` on success, or `Err(CqamError)` on runtime errors
/// (unknown kernel, uninitialized quantum register, etc.).
///
/// # PC Ownership
///
/// This function does NOT advance the PC. The caller (executor) handles
/// PC advancement after this function returns.
pub fn execute_qop(ctx: &mut ExecutionContext, instr: &Instruction) -> Result<(), CqamError> {
    match instr {
        Instruction::QPrep { dst, dist } => {
            let qdist = match *dist {
                dist_id::UNIFORM => {
                    let domain: Vec<u16> = (0..4).collect();
                    let n = domain.len();
                    let prob = 1.0 / n as f64;
                    QDist::new("uniform", domain, vec![prob; n])
                        .map_err(CqamError::ConfigError)?
                }
                dist_id::ZERO => {
                    QDist::new("zero", vec![0u16], vec![1.0])
                        .map_err(CqamError::ConfigError)?
                }
                dist_id::BELL => {
                    QDist::new("bell", vec![0u16, 3], vec![0.5, 0.5])
                        .map_err(CqamError::ConfigError)?
                }
                dist_id::GHZ => {
                    QDist::new("ghz", vec![0u16, 15], vec![0.5, 0.5])
                        .map_err(CqamError::ConfigError)?
                }
                _ => {
                    return Err(CqamError::UnknownKernel(
                        format!("Unknown distribution ID: {}", dist),
                    ));
                }
            };
            ctx.qregs[*dst as usize] = Some(qdist);
            Ok(())
        }

        Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
            let qsrc = ctx.qregs[*src as usize].clone();
            let param0 = ctx.iregs.get(*ctx0)?;
            let _param1 = ctx.iregs.get(*ctx1)?;

            if let Some(qdist) = qsrc {
                let k: Box<dyn Kernel<u16>> = match *kernel {
                    kernel_id::INIT => {
                        let domain: Vec<u16> = qdist.domain.clone();
                        Box::new(InitDist { domain })
                    }
                    kernel_id::ENTANGLE => {
                        Box::new(Entangle { strength: 0.3 })
                    }
                    kernel_id::FOURIER => {
                        Box::new(Fourier)
                    }
                    kernel_id::DIFFUSE => {
                        Box::new(Diffuse)
                    }
                    kernel_id::GROVER_ITER => {
                        // Target state is read from int_regs[ctx0]
                        let target = param0 as u16;
                        Box::new(GroverIter { target })
                    }
                    _ => {
                        return Err(CqamError::UnknownKernel(
                            format!("Unknown kernel ID: {}", kernel),
                        ));
                    }
                };

                let result = k.apply(&qdist);

                // Update PSW quantum flags with real fidelity metrics
                let superposition = result.superposition_metric();
                let entanglement = result.entanglement_metric();

                ctx.qregs[*dst as usize] = Some(result);
                ctx.psw.update_from_qmeta(
                    superposition,
                    entanglement,
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

        Instruction::QObserve { dst_h, src_q } => {
            if let Some(qdist) = ctx.qregs[*src_q as usize].take() {
                let measured_value = qdist.measure();

                // Collapse: store a delta distribution at the measured value
                let dist_pairs: Vec<(u16, f64)> = if let Some(value) = measured_value {
                    vec![(value, 1.0)]
                } else {
                    // Empty domain: store empty distribution
                    vec![]
                };

                ctx.hregs.set(*dst_h, HybridValue::Dist(dist_pairs))?;
                ctx.psw.mark_measured();
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "Q".to_string(),
                    index: *src_q,
                })
            }
        }

        Instruction::QLoad { dst_q, addr } => {
            if let Some(qdist) = ctx.qmem.load(*addr) {
                ctx.qregs[*dst_q as usize] = Some(qdist.clone());
                Ok(())
            } else {
                Err(CqamError::UninitializedRegister {
                    file: "QMEM".to_string(),
                    index: *addr,
                })
            }
        }

        Instruction::QStore { src_q, addr } => {
            if let Some(ref qdist) = ctx.qregs[*src_q as usize] {
                ctx.qmem.store(*addr, qdist.clone());
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
