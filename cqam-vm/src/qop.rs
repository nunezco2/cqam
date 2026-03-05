// cqam-vm/src/qop.rs
//
// Phase 2 (density matrix): Quantum operation handlers using DensityMatrix.

use cqam_core::error::CqamError;
use cqam_core::instruction::{Instruction, dist_id, kernel_id};
use cqam_core::register::HybridValue;
use cqam_sim::density_matrix::DensityMatrix;
use cqam_sim::kernel::Kernel;
use cqam_sim::kernels::init::Init;
use cqam_sim::kernels::entangle::Entangle;
use cqam_sim::kernels::fourier::Fourier;
use cqam_sim::kernels::diffuse::Diffuse;
use cqam_sim::kernels::grover::GroverIter;
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
            let _param1 = ctx.iregs.get(*ctx1)?;

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

        Instruction::QObserve { dst_h, src_q } => {
            if let Some(dm) = ctx.qregs[*src_q as usize].take() {
                let (measured_value, _collapsed) = dm.measure_all();
                let dist_pairs: Vec<(u16, f64)> = vec![(measured_value, 1.0)];
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
