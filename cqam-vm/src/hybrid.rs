//! Hybrid operation handlers (HFORK, HMERGE, HCEXEC, HREDUCE).
//!
//! Provides fork/merge parallelism and reduction operations that bridge
//! quantum measurement results into classical register values.

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

            macro_rules! hreduce_float_to_int {
                ($val:expr, $name:expr, $body:expr, $ctx:expr, $dst:expr) => {
                    match $val {
                        HybridValue::Float(x) => {
                            $ctx.iregs.set($dst, ($body)(x))?;
                        }
                        HybridValue::Dist(ref entries) => {
                            let mean: f64 = entries.iter()
                                .map(|(val, prob)| *val as f64 * prob)
                                .sum();
                            $ctx.iregs.set($dst, ($body)(mean))?;
                        }
                        _ => {
                            return Err(CqamError::TypeMismatch {
                                instruction: concat!("HREDUCE/", $name).to_string(),
                                detail: format!("expected Float or Dist, got {:?}", $val),
                            });
                        }
                    }
                };
            }

            macro_rules! hreduce_complex_to_float {
                ($val:expr, $name:expr, $body:expr, $ctx:expr, $dst:expr) => {
                    if let HybridValue::Complex(re, im) = $val {
                        $ctx.fregs.set($dst, ($body)(re, im))?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: concat!("HREDUCE/", $name).to_string(),
                            detail: format!("expected Complex, got {:?}", $val),
                        });
                    }
                };
            }

            macro_rules! hreduce_dist_to_float {
                ($val:expr, $name:expr, $body:expr, $ctx:expr, $dst:expr) => {
                    if let HybridValue::Dist(ref entries) = $val {
                        $ctx.fregs.set($dst, ($body)(entries))?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: concat!("HREDUCE/", $name).to_string(),
                            detail: format!("expected Dist, got {:?}", $val),
                        });
                    }
                };
            }

            macro_rules! hreduce_dist_to_int {
                ($val:expr, $name:expr, $body:expr, $ctx:expr, $dst:expr) => {
                    if let HybridValue::Dist(ref entries) = $val {
                        $ctx.iregs.set($dst, ($body)(entries))?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: concat!("HREDUCE/", $name).to_string(),
                            detail: format!("expected Dist, got {:?}", $val),
                        });
                    }
                };
            }

            match *func {
                reduce_fn::ROUND   => hreduce_float_to_int!(hybrid_val, "ROUND",   |x: f64| x.round() as i64,  ctx, *dst),
                reduce_fn::FLOOR   => hreduce_float_to_int!(hybrid_val, "FLOOR",   |x: f64| x.floor() as i64,  ctx, *dst),
                reduce_fn::CEIL    => hreduce_float_to_int!(hybrid_val, "CEIL",    |x: f64| x.ceil() as i64,   ctx, *dst),
                reduce_fn::TRUNC   => hreduce_float_to_int!(hybrid_val, "TRUNC",   |x: f64| x.trunc() as i64,  ctx, *dst),
                reduce_fn::ABS     => hreduce_float_to_int!(hybrid_val, "ABS",     |x: f64| x.abs() as i64,    ctx, *dst),
                reduce_fn::NEGATE  => hreduce_float_to_int!(hybrid_val, "NEGATE",  |x: f64| (-x) as i64,       ctx, *dst),

                reduce_fn::MAGNITUDE => hreduce_complex_to_float!(hybrid_val, "MAGNITUDE", |re: f64, im: f64| (re * re + im * im).sqrt(), ctx, *dst),
                reduce_fn::PHASE     => hreduce_complex_to_float!(hybrid_val, "PHASE",     |re: f64, im: f64| im.atan2(re),               ctx, *dst),
                reduce_fn::REAL      => hreduce_complex_to_float!(hybrid_val, "REAL",      |re: f64, _im: f64| re,                        ctx, *dst),
                reduce_fn::IMAG      => hreduce_complex_to_float!(hybrid_val, "IMAG",      |_re: f64, im: f64| im,                        ctx, *dst),

                reduce_fn::MEAN => hreduce_dist_to_float!(hybrid_val, "MEAN", |e: &[(u16, f64)]| {
                    e.iter().map(|(val, prob)| *val as f64 * prob).sum::<f64>()
                }, ctx, *dst),

                reduce_fn::VARIANCE => hreduce_dist_to_float!(hybrid_val, "VARIANCE", |e: &[(u16, f64)]| {
                    let mean: f64 = e.iter().map(|(val, prob)| *val as f64 * prob).sum();
                    e.iter().map(|(val, prob)| { let d = *val as f64 - mean; d * d * prob }).sum::<f64>()
                }, ctx, *dst),

                reduce_fn::MODE => hreduce_dist_to_int!(hybrid_val, "MODE", |e: &[(u16, f64)]| {
                    e.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                        .map(|(val, _)| *val as i64).unwrap_or(0)
                }, ctx, *dst),

                reduce_fn::ARGMAX => hreduce_dist_to_int!(hybrid_val, "ARGMAX", |e: &[(u16, f64)]| {
                    e.iter().enumerate()
                        .max_by(|a, b| (a.1).1.partial_cmp(&(b.1).1).unwrap_or(std::cmp::Ordering::Equal))
                        .map(|(idx, _)| idx as i64).unwrap_or(0)
                }, ctx, *dst),

                reduce_fn::CONJ_Z => {
                    if let HybridValue::Complex(re, im) = hybrid_val {
                        ctx.zregs.set(*dst, (re, -im))?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/CONJ_Z".to_string(),
                            detail: format!("expected Complex, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::NEGATE_Z => {
                    if let HybridValue::Complex(re, im) = hybrid_val {
                        ctx.zregs.set(*dst, (-re, -im))?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/NEGATE_Z".to_string(),
                            detail: format!("expected Complex, got {:?}", hybrid_val),
                        });
                    }
                }

                reduce_fn::EXPECT => {
                    if let HybridValue::Dist(ref entries) = hybrid_val {
                        let base_addr = ctx.iregs.get(*dst)? as u16;

                        let mut expectation = 0.0f64;
                        for (val, prob) in entries {
                            let eigenvalue_addr = base_addr.wrapping_add(*val);
                            let eigenvalue = f64::from_bits(ctx.cmem.load(eigenvalue_addr) as u64);
                            expectation += eigenvalue * prob;
                        }

                        ctx.fregs.set(*dst, expectation)?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/EXPECT".to_string(),
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
