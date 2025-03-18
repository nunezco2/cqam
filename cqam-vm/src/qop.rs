// cqam-vm/src/qop.rs

use cqam_core::instruction::Instruction;
use cqam_core::register::CValue;
use cqam_sim::qdist::{QDist, Measurable};
use cqam_sim::kernel::Kernel;
use cqam_sim::kernels::init::InitDist;
use cqam_sim::kernels::entangle::Entangle;
use crate::context::ExecutionContext;

/// Execute a quantum instruction.
pub fn execute_qop(ctx: &mut ExecutionContext, instr: Instruction) {
    match instr {
        Instruction::QPrep { dst, dist_src: _ } => {
            let domain = vec![0, 1];
            let kernel = InitDist { domain };
            let dummy = QDist::new("dummy", vec![0], vec![1.0]);
            let qdist = kernel.apply(&dummy);
            ctx.qmem.insert(&dst, qdist);
        }

        Instruction::QKernel { dst, src, kernel, ctx: _ } => {
            let qsrc = ctx.qmem.get(&src).cloned();
            if let Some(qdist) = qsrc {
                let k: Box<dyn Kernel<i32>> = match kernel.as_str() {
                    "entangle" => Box::new(Entangle { strength: 0.3 }),
                    _ => panic!("Unknown kernel"),
                };
                let result = k.apply(&qdist);
                ctx.qmem.insert(&dst, result.clone());
        
                // Simulated fidelity levels (placeholders for now)
                let superposition = 0.4; // example placeholder
                let entanglement = 0.3;  // example placeholder
        
                ctx.psw.update_from_qmeta(
                    superposition,
                    entanglement,
                    (
                        ctx.config.min_superposition,
                        ctx.config.min_entanglement,
                    ),
                );
            }
        }

        Instruction::QMeas { dst, src } => {
            if let Some(qdist) = ctx.qmem.get(&src) {
                if let Some(val) = qdist.measure() {
                    ctx.registers.store_c(&dst, CValue::Int(val));
                    ctx.psw.mark_measured();
                }
            }
        }

        Instruction::QObserve { dst, src } => {
            let qdist = ctx.qmem.get(&src);
            if let Some(dist) = qdist {
                let avg = dist
                    .domain
                    .iter()
                    .zip(dist.probabilities.iter())
                    .map(|(x, p)| *x as f64 * p)
                    .sum();
                ctx.registers.store_c(&dst, CValue::Float(avg));
            }
        }

        _ => panic!("Invalid QOP instruction passed to execute_qop"),
    }
}
