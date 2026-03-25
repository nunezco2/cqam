//! Hybrid operation handlers (HFORK, HMERGE, JMPF, HREDUCE, HATMS, HATME).
//!
//! Provides fork/merge parallelism and reduction operations that bridge
//! quantum measurement results into classical register values.

use std::sync::Arc;
use cqam_core::error::CqamError;
use cqam_core::instruction::{Instruction, ReduceFn};
use cqam_core::quantum_backend::QuantumBackend;
use cqam_core::register::HybridValue;
use crate::context::ExecutionContext;
use crate::fork::ForkManager;
use crate::thread_pool::{SharedQuantumFile, SharedMemory, SharedRegionConfig, ThreadBarrier};
use rayon::prelude::*;

use cqam_core::constants::PAR_THRESHOLD;

/// Execute a hybrid instruction with fork/merge support.
///
/// Returns `Ok(true)` if a jump was taken (JmpF with condition true), in which
/// case the caller should NOT advance the PC. Returns `Ok(false)` otherwise.
/// Returns `Err(CqamError)` on runtime errors (unknown reduce function, type mismatch).
pub fn execute_hybrid<B: QuantumBackend + Clone + Send + 'static>(
    ctx: &mut ExecutionContext,
    instr: &Instruction,
    fork_mgr: &mut ForkManager,
    backend: &mut B,
) -> Result<bool, CqamError> {
    match instr {
        Instruction::HFork => {
            // Validate: not already forked
            if ctx.psw.forked {
                return Err(CqamError::ForkError(
                    "nested HFORK is not allowed".to_string(),
                ));
            }

            let n = ctx.thread_count;

            if n <= 1 {
                // Single-threaded: just set flags, no actual threading
                ctx.psw.hf = true;
                ctx.psw.forked = true;
                ctx.psw.merged = false;
                return Ok(false);
            }

            // Multi-threaded: QF must be down (all quantum registers observed)
            // because quantum registers will be moved to SharedQuantumFile.
            if ctx.psw.qf {
                return Err(CqamError::ForkError(
                    "HFORK requires QF=0: all quantum registers must be observed before forking".to_string(),
                ));
            }

            // 1. Create SharedQuantumFile from current Q registers
            let shared_qfile = Arc::new(SharedQuantumFile::from_qregs(
                std::mem::take(&mut ctx.qregs),
            ));

            // 2. Create SharedMemory from .shared CMEM region
            let shared_mem = if let Some((base, size)) = ctx.shared_region {
                let initial: Vec<i64> = (0..size)
                    .map(|i| ctx.cmem.load(base + i))
                    .collect();
                Arc::new(SharedMemory::new(
                    SharedRegionConfig { base, size },
                    &initial,
                ))
            } else {
                Arc::new(SharedMemory::new(
                    SharedRegionConfig { base: 0, size: 0 },
                    &[],
                ))
            };

            // 3. Create barrier
            let barrier = Arc::new(ThreadBarrier::new(n));

            // 4. Spawn N-1 worker threads
            let fork_pc = ctx.pc + 1;

            for tid in 1..n {
                let mut worker_ctx = ctx.clone();
                worker_ctx.pc = fork_pc;
                worker_ctx.thread_id = tid;
                worker_ctx.psw.hf = true;
                worker_ctx.psw.forked = true;
                worker_ctx.psw.merged = false;
                worker_ctx.skip_to_hatme = false;
                // Workers get empty Q registers (they access via shared_qfile)
                worker_ctx.qregs = Default::default();
                // Workers get a reference to shared memory for load/store interception
                worker_ctx.shared_memory = Some(Arc::clone(&shared_mem));

                let sqf = Arc::clone(&shared_qfile);
                let sm = Arc::clone(&shared_mem);
                let b = Arc::clone(&barrier);
                let depth = fork_mgr.depth();
                let max_depth = fork_mgr.max_depth();
                let mut worker_backend = backend.clone();

                let handle = std::thread::Builder::new()
                    .name(format!("cqam-spmd-t{}", tid))
                    .spawn(move || {
                        let mut fm = ForkManager::nested(depth, max_depth);
                        fm.set_shared_resources(sqf, sm, b);
                        run_spmd_thread(&mut worker_ctx, &mut fm, &mut worker_backend)?;
                        Ok(worker_ctx)
                    })
                    .map_err(CqamError::IoError)?;

                fork_mgr.active_forks.push(handle);
            }

            // 5. Configure thread 0
            ctx.thread_id = 0;
            ctx.psw.hf = true;
            ctx.psw.forked = true;
            ctx.psw.merged = false;
            ctx.skip_to_hatme = false;
            ctx.qregs = Default::default(); // Q regs moved to shared file
            ctx.shared_memory = Some(Arc::clone(&shared_mem));

            fork_mgr.set_shared_resources(
                Arc::clone(&shared_qfile),
                Arc::clone(&shared_mem),
                Arc::clone(&barrier),
            );

            Ok(false)
        }

        Instruction::HMerge => {
            if !ctx.psw.forked {
                return Err(CqamError::ForkError(
                    "HMERGE without prior HFORK".into(),
                ));
            }

            // Set merge flags (both leader and workers)
            ctx.psw.hf = true;
            ctx.psw.merged = true;
            ctx.psw.forked = false;
            ctx.in_atomic_section = false;
            ctx.psw.af = false;
            ctx.skip_to_hatme = false;

            if ctx.thread_count <= 1 {
                return Ok(false);
            }

            // Multi-threaded: QF must be down before merging
            if ctx.psw.qf {
                return Err(CqamError::ForkError(
                    "HMERGE requires QF=0".into(),
                ));
            }

            if ctx.thread_id == 0 {
                // Thread 0: join all workers
                fork_mgr.join_all()?;

                // Restore quantum registers from shared file
                if let Some(sqf) = fork_mgr.take_shared_qfile() {
                    if let Ok(sqf) = Arc::try_unwrap(sqf) {
                        ctx.qregs = sqf.into_qregs();
                    }
                }

                // Write back shared memory to thread 0's CMEM
                if let Some(sm) = fork_mgr.take_shared_mem() {
                    sm.write_back(&mut ctx.cmem);
                }

                // Clear shared memory reference from context
                ctx.shared_memory = None;
            }
            // Workers (thread_id > 0): merged=true causes run_spmd_thread to exit

            Ok(false)
        }

        Instruction::JmpF { flag, target } => {
            let cond = ctx.psw.get_flag(u8::from(*flag));
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
                        HybridValue::Complex(re, _im) => {
                            $ctx.iregs.set($dst, ($body)(re))?;
                        }
                        HybridValue::Dist(ref entries) => {
                            let mean: f64 = if entries.len() >= PAR_THRESHOLD {
                                entries.par_iter()
                                    .map(|(val, prob)| *val as f64 * prob)
                                    .sum()
                            } else {
                                entries.iter()
                                    .map(|(val, prob)| *val as f64 * prob)
                                    .sum()
                            };
                            $ctx.iregs.set($dst, ($body)(mean))?;
                        }
                        HybridValue::Hist(ref hist) => {
                            let entries = hist.to_dist();
                            let mean: f64 = entries.iter()
                                .map(|(val, prob)| *val as f64 * prob)
                                .sum();
                            $ctx.iregs.set($dst, ($body)(mean))?;
                        }
                        HybridValue::Int(v) => {
                            $ctx.iregs.set($dst, ($body)(v as f64))?;
                        }
                        _ => {
                            return Err(CqamError::TypeMismatch {
                                instruction: concat!("HREDUCE/", $name).to_string(),
                                detail: format!("expected Float, Complex, Dist, or Int, got {:?}", $val),
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
                    match $val {
                        HybridValue::Dist(ref entries) => {
                            $ctx.fregs.set($dst, ($body)(entries))?;
                        }
                        HybridValue::Hist(ref hist) => {
                            let entries = hist.to_dist();
                            $ctx.fregs.set($dst, ($body)(&entries))?;
                        }
                        HybridValue::Int(v) => {
                            $ctx.fregs.set($dst, v as f64)?;
                        }
                        HybridValue::Float(v) => {
                            $ctx.fregs.set($dst, v)?;
                        }
                        _ => {
                            return Err(CqamError::TypeMismatch {
                                instruction: concat!("HREDUCE/", $name).to_string(),
                                detail: format!("expected Dist, Int, or Float, got {:?}", $val),
                            });
                        }
                    }
                };
            }

            macro_rules! hreduce_dist_to_int {
                ($val:expr, $name:expr, $body:expr, $ctx:expr, $dst:expr) => {
                    match $val {
                        HybridValue::Dist(ref entries) => {
                            $ctx.iregs.set($dst, ($body)(entries))?;
                        }
                        HybridValue::Hist(ref hist) => {
                            let entries = hist.to_dist();
                            $ctx.iregs.set($dst, ($body)(&entries))?;
                        }
                        HybridValue::Int(v) => {
                            $ctx.iregs.set($dst, v)?;
                        }
                        _ => {
                            return Err(CqamError::TypeMismatch {
                                instruction: concat!("HREDUCE/", $name).to_string(),
                                detail: format!("expected Dist or Int, got {:?}", $val),
                            });
                        }
                    }
                };
            }

            match *func {
                ReduceFn::Round   => hreduce_float_to_int!(hybrid_val, "ROUND",   |x: f64| x.round() as i64,  ctx, *dst),
                ReduceFn::Floor   => hreduce_float_to_int!(hybrid_val, "FLOOR",   |x: f64| x.floor() as i64,  ctx, *dst),
                ReduceFn::Ceil    => hreduce_float_to_int!(hybrid_val, "CEIL",    |x: f64| x.ceil() as i64,   ctx, *dst),
                ReduceFn::Trunc   => hreduce_float_to_int!(hybrid_val, "TRUNC",   |x: f64| x.trunc() as i64,  ctx, *dst),
                ReduceFn::Abs     => hreduce_float_to_int!(hybrid_val, "ABS",     |x: f64| x.abs() as i64,    ctx, *dst),
                ReduceFn::Negate  => hreduce_float_to_int!(hybrid_val, "NEGATE",  |x: f64| (-x) as i64,       ctx, *dst),

                ReduceFn::Magnitude => hreduce_complex_to_float!(hybrid_val, "MAGNITUDE", |re: f64, im: f64| (re * re + im * im).sqrt(), ctx, *dst),
                ReduceFn::Phase     => hreduce_complex_to_float!(hybrid_val, "PHASE",     |re: f64, im: f64| im.atan2(re),               ctx, *dst),
                ReduceFn::Real      => hreduce_complex_to_float!(hybrid_val, "REAL",      |re: f64, _im: f64| re,                        ctx, *dst),
                ReduceFn::Imag      => hreduce_complex_to_float!(hybrid_val, "IMAG",      |_re: f64, im: f64| im,                        ctx, *dst),

                ReduceFn::Mean => hreduce_dist_to_float!(hybrid_val, "MEAN", |e: &[(u32, f64)]| {
                    if e.len() >= PAR_THRESHOLD {
                        e.par_iter().map(|(val, prob)| *val as f64 * prob).sum::<f64>()
                    } else {
                        e.iter().map(|(val, prob)| *val as f64 * prob).sum::<f64>()
                    }
                }, ctx, *dst),

                ReduceFn::Variance => hreduce_dist_to_float!(hybrid_val, "VARIANCE", |e: &[(u32, f64)]| {
                    if e.len() >= PAR_THRESHOLD {
                        let mean: f64 = e.par_iter().map(|(val, prob)| *val as f64 * prob).sum();
                        e.par_iter().map(|(val, prob)| { let d = *val as f64 - mean; d * d * prob }).sum::<f64>()
                    } else {
                        let mean: f64 = e.iter().map(|(val, prob)| *val as f64 * prob).sum();
                        e.iter().map(|(val, prob)| { let d = *val as f64 - mean; d * d * prob }).sum::<f64>()
                    }
                }, ctx, *dst),

                ReduceFn::Mode => hreduce_dist_to_int!(hybrid_val, "MODE", |e: &[(u32, f64)]| {
                    if e.len() >= PAR_THRESHOLD {
                        e.par_iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(val, _)| *val as i64).unwrap_or(0)
                    } else {
                        e.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(val, _)| *val as i64).unwrap_or(0)
                    }
                }, ctx, *dst),

                ReduceFn::Argmax => hreduce_dist_to_int!(hybrid_val, "ARGMAX", |e: &[(u32, f64)]| {
                    if e.len() >= PAR_THRESHOLD {
                        e.par_iter()
                            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(val, _)| *val as i64).unwrap_or(0)
                    } else {
                        e.iter()
                            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(val, _)| *val as i64).unwrap_or(0)
                    }
                }, ctx, *dst),

                ReduceFn::ConjZ => {
                    if let HybridValue::Complex(re, im) = hybrid_val {
                        ctx.zregs.set(*dst, (re, -im))?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/CONJ_Z".to_string(),
                            detail: format!("expected Complex, got {:?}", hybrid_val),
                        });
                    }
                }

                ReduceFn::NegateZ => {
                    if let HybridValue::Complex(re, im) = hybrid_val {
                        ctx.zregs.set(*dst, (-re, -im))?;
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/NEGATE_Z".to_string(),
                            detail: format!("expected Complex, got {:?}", hybrid_val),
                        });
                    }
                }

                ReduceFn::Expect => {
                    let entries: Vec<(u32, f64)>;
                    let entries_ref: &[(u32, f64)] = if let HybridValue::Dist(ref e) = hybrid_val {
                        e
                    } else if let HybridValue::Hist(ref hist) = hybrid_val {
                        entries = hist.to_dist();
                        &entries
                    } else {
                        return Err(CqamError::TypeMismatch {
                            instruction: "HREDUCE/EXPECT".to_string(),
                            detail: format!("expected Dist or Hist, got {:?}", hybrid_val),
                        });
                    };

                    let base_addr = ctx.iregs.get(*dst)? as u16;

                    let expectation = if entries_ref.len() >= PAR_THRESHOLD {
                        let cmem = &ctx.cmem;
                        entries_ref.par_iter().map(|(val, prob)| {
                            let eigenvalue_addr = base_addr.wrapping_add(*val as u16);
                            let eigenvalue = f64::from_bits(cmem.load(eigenvalue_addr) as u64);
                            eigenvalue * prob
                        }).sum::<f64>()
                    } else {
                        let mut exp = 0.0f64;
                        for (val, prob) in entries_ref {
                            let eigenvalue_addr = base_addr.wrapping_add(*val as u16);
                            let eigenvalue = f64::from_bits(ctx.cmem.load(eigenvalue_addr) as u64);
                            exp += eigenvalue * prob;
                        }
                        exp
                    };

                    ctx.fregs.set(*dst, expectation)?;
                }
            }

            // Consuming a measurement result clears the collapsed signal.
            ctx.psw.clear_collapsed();

            // Update PSW flags from the reduction result
            match *func {
                ReduceFn::Round | ReduceFn::Floor | ReduceFn::Ceil
                | ReduceFn::Trunc | ReduceFn::Abs | ReduceFn::Negate
                | ReduceFn::Mode | ReduceFn::Argmax => {
                    if let Ok(val) = ctx.iregs.get(*dst) {
                        ctx.psw.zf = val == 0;
                        ctx.psw.nf = val < 0;
                    }
                }
                ReduceFn::Magnitude | ReduceFn::Phase | ReduceFn::Real
                | ReduceFn::Imag | ReduceFn::Mean | ReduceFn::Variance
                | ReduceFn::Expect => {
                    if let Ok(val) = ctx.fregs.get(*dst) {
                        ctx.psw.zf = val == 0.0;
                        ctx.psw.nf = val < 0.0;
                    }
                }
                ReduceFn::ConjZ | ReduceFn::NegateZ => {
                    if let Ok((re, im)) = ctx.zregs.get(*dst) {
                        ctx.psw.zf = re == 0.0 && im == 0.0;
                    }
                }
            }

            Ok(false)
        }

        Instruction::HAtmS => {
            if !ctx.psw.forked {
                return Err(CqamError::ForkError("HATMS outside parallel region".into()));
            }
            if ctx.thread_count <= 1 {
                // Single-threaded: trivially enter atomic section
                ctx.in_atomic_section = true;
                ctx.psw.af = true;
                return Ok(false);
            }

            // Full barrier: wait for all threads
            let barrier = fork_mgr.get_barrier()
                .expect("barrier must exist in forked region");
            let result = barrier.wait(ctx.thread_id);

            if result.is_leader {
                ctx.in_atomic_section = true;
                ctx.psw.af = true;
                // Leader proceeds to execute the atomic section
            } else {
                ctx.in_atomic_section = false;
                ctx.psw.af = false;
                ctx.skip_to_hatme = true;
                // Non-leaders will skip instructions until HATME
            }
            Ok(false)
        }

        Instruction::HAtmE => {
            if ctx.thread_count <= 1 {
                ctx.in_atomic_section = false;
                ctx.psw.af = false;
                return Ok(false);
            }

            // Leader: commit shared memory snapshot
            if ctx.in_atomic_section {
                if let Some(sm) = fork_mgr.get_shared_mem() {
                    sm.commit_snapshot();
                }
                ctx.in_atomic_section = false;
                ctx.psw.af = false;
            }

            // Full barrier: all threads synchronize here
            let barrier = fork_mgr.get_barrier()
                .expect("barrier must exist in forked region");
            barrier.wait(ctx.thread_id);

            // All threads resume normal execution
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

/// Run an SPMD worker thread until it reaches HMERGE.
///
/// Similar to `run_program` but stops when `merged` is set (at HMERGE)
/// rather than only on HALT. Worker threads exit their loop when HMERGE
/// sets `psw.merged = true`.
pub fn run_spmd_thread<B: QuantumBackend + Clone + Send + 'static>(
    ctx: &mut ExecutionContext,
    fork_mgr: &mut ForkManager,
    backend: &mut B,
) -> Result<(), CqamError> {
    use std::sync::Arc;
    use crate::executor::execute_instruction;

    let program = Arc::clone(&ctx.program);
    while ctx.pc < program.len() {
        let instr = &program[ctx.pc];
        execute_instruction(ctx, instr, fork_mgr, backend)?;

        if ctx.psw.trap_halt || ctx.psw.merged {
            break;
        }
    }
    Ok(())
}
