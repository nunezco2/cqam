// cqam-vm/src/hybrid.rs
//
// Phase 6: Hybrid operation handlers with real HFORK/HMERGE parallelism.

use cqam_core::error::CqamError;
use cqam_core::instruction::{Instruction, reduce_fn};
use cqam_core::register::HybridValue;
use crate::context::ExecutionContext;
use crate::fork::ForkManager;

/// Execute a hybrid instruction with fork/merge support.
///
/// Returns `Ok(true)` if a jump was taken (HCExec with condition true), in which
/// case the caller should NOT advance the PC. Returns `Ok(false)` otherwise.
/// Returns `Err(CqamError)` on runtime errors (unknown reduce function, type mismatch).
pub fn execute_hybrid(
    ctx: &mut ExecutionContext,
    instr: &Instruction,
    fork_mgr: &mut ForkManager,
) -> Result<bool, CqamError> {
    match instr {
        Instruction::HFork => {
            // Clone context for the fork thread
            let mut fork_ctx = ctx.clone();
            fork_ctx.pc = ctx.pc + 1; // Fork starts at next instruction
            fork_ctx.psw.hf = true;
            fork_ctx.psw.forked = true;

            fork_mgr.spawn_fork(fork_ctx)?;

            ctx.psw.hf = true;
            ctx.psw.forked = true;
            Ok(false)
        }

        Instruction::HMerge => {
            if fork_mgr.active_count() > 0 {
                fork_mgr.join_all()?;
            }
            ctx.psw.hf = true;
            ctx.psw.merged = true;
            Ok(false)
        }

        Instruction::HCExec { flag, target } => {
            let cond = ctx.psw.get_flag(*flag);
            ctx.psw.update_from_predicate(cond);

            if cond {
                ctx.jump_to_label(target)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        Instruction::HReduce { src, dst, func } => {
            let hybrid_val = ctx.hregs.get(*src)?.clone();

            match *func {
                // -- Float -> Int reductions ----------------------------------

                reduce_fn::ROUND => {
                    if let HybridValue::Float(x) = hybrid_val {
                        ctx.iregs.set(*dst, x.round() as i64)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/ROUND".to_string(),
                            detail: format!("expected Float, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::FLOOR => {
                    if let HybridValue::Float(x) = hybrid_val {
                        ctx.iregs.set(*dst, x.floor() as i64)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/FLOOR".to_string(),
                            detail: format!("expected Float, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::CEIL => {
                    if let HybridValue::Float(x) = hybrid_val {
                        ctx.iregs.set(*dst, x.ceil() as i64)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/CEIL".to_string(),
                            detail: format!("expected Float, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::TRUNC => {
                    if let HybridValue::Float(x) = hybrid_val {
                        ctx.iregs.set(*dst, x.trunc() as i64)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/TRUNC".to_string(),
                            detail: format!("expected Float, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::ABS => {
                    if let HybridValue::Float(x) = hybrid_val {
                        ctx.iregs.set(*dst, x.abs() as i64)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/ABS".to_string(),
                            detail: format!("expected Float, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::NEGATE => {
                    if let HybridValue::Float(x) = hybrid_val {
                        ctx.iregs.set(*dst, (-x) as i64)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/NEGATE".to_string(),
                            detail: format!("expected Float, got {:?}", hybrid_val),
                        });
                    }
                }

                // -- Complex -> Float reductions ------------------------------

                reduce_fn::MAGNITUDE => {
                    if let HybridValue::Complex(re, im) = hybrid_val {
                        let mag = (re * re + im * im).sqrt();
                        ctx.fregs.set(*dst, mag)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/MAGNITUDE".to_string(),
                            detail: format!("expected Complex, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::PHASE => {
                    if let HybridValue::Complex(re, im) = hybrid_val {
                        ctx.fregs.set(*dst, im.atan2(re))?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/PHASE".to_string(),
                            detail: format!("expected Complex, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::REAL => {
                    if let HybridValue::Complex(re, _im) = hybrid_val {
                        ctx.fregs.set(*dst, re)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/REAL".to_string(),
                            detail: format!("expected Complex, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::IMAG => {
                    if let HybridValue::Complex(_re, im) = hybrid_val {
                        ctx.fregs.set(*dst, im)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/IMAG".to_string(),
                            detail: format!("expected Complex, got {:?}", hybrid_val),
                        });
                    }
                }

                // -- Distribution reductions ----------------------------------

                reduce_fn::MEAN => {
                    if let HybridValue::Dist(ref entries) = hybrid_val {
                        let mean: f64 = entries.iter()
                            .map(|(val, prob)| *val as f64 * prob)
                            .sum();
                        ctx.fregs.set(*dst, mean)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/MEAN".to_string(),
                            detail: format!("expected Dist, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::MODE => {
                    if let HybridValue::Dist(ref entries) = hybrid_val {
                        if let Some((val, _)) = entries.iter()
                            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                        {
                            ctx.iregs.set(*dst, *val as i64)?;
                        }
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/MODE".to_string(),
                            detail: format!("expected Dist, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::ARGMAX => {
                    if let HybridValue::Dist(ref entries) = hybrid_val {
                        if let Some((idx, _)) = entries.iter().enumerate()
                            .max_by(|a, b| (a.1).1.partial_cmp(&(b.1).1).unwrap_or(std::cmp::Ordering::Equal))
                        {
                            ctx.iregs.set(*dst, idx as i64)?;
                        }
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/ARGMAX".to_string(),
                            detail: format!("expected Dist, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::VARIANCE => {
                    if let HybridValue::Dist(ref entries) = hybrid_val {
                        let mean: f64 = entries.iter()
                            .map(|(val, prob)| *val as f64 * prob)
                            .sum();
                        let var: f64 = entries.iter()
                            .map(|(val, prob)| {
                                let diff = *val as f64 - mean;
                                diff * diff * prob
                            })
                            .sum();
                        ctx.fregs.set(*dst, var)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/VARIANCE".to_string(),
                            detail: format!("expected Dist, got {:?}", hybrid_val),
                        });
                    }
                }

                _ => {
                    return Err(CqamError::UnknownKernel(
                        format!("Unknown reduction function ID: {}", func),
                    ));
                }
            }

            Ok(false)
        }

        _ => {
            Err(CqamError::TypeMismatch {
                instruction: format!("{:?}", instr),
                detail: "Invalid instruction passed to execute_hybrid".to_string(),
            })
        }
    }
}
