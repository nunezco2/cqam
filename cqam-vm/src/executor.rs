//! Instruction dispatch for the CQAM virtual machine.
//!
//! `execute_instruction` is the central dispatch function: it handles all
//! classical instructions inline and delegates quantum operations to `qop.rs`
//! and hybrid fork/merge operations to `hybrid.rs`. PC advancement is the
//! exclusive responsibility of this module.

use crate::context::ExecutionContext;
use crate::fork::ForkManager;
use crate::resource::resource_cost;
use crate::qop::execute_qop;
use crate::hybrid::execute_hybrid;
use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;
use crate::isr::{Trap, MaskableTrap};

/// Execute a single instruction in the given context.
///
/// Returns `Ok(())` on success, or `Err(CqamError)` on runtime errors
/// (division by zero, unknown kernel, etc.).
///
/// # PC Ownership Contract
///
/// This function is the SOLE authority on PC advancement. The runner loop
/// must NOT call `ctx.advance_pc()` independently.
pub fn execute_instruction(
    ctx: &mut ExecutionContext,
    instr: &Instruction,
    fork_mgr: &mut ForkManager,
) -> Result<(), CqamError> {
    match instr {
        // =====================================================================
        // Integer arithmetic (R-file)
        // =====================================================================

        Instruction::IAdd { dst, lhs, rhs } => {
            let result = ctx.iregs.get(*lhs)?.wrapping_add(ctx.iregs.get(*rhs)?);
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        Instruction::ISub { dst, lhs, rhs } => {
            let result = ctx.iregs.get(*lhs)?.wrapping_sub(ctx.iregs.get(*rhs)?);
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        Instruction::IMul { dst, lhs, rhs } => {
            let result = ctx.iregs.get(*lhs)?.wrapping_mul(ctx.iregs.get(*rhs)?);
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        Instruction::IDiv { dst, lhs, rhs } => {
            let divisor = ctx.iregs.get(*rhs)?;
            if divisor == 0 {
                ctx.iregs.set(*dst, 0)?;
                ctx.psw.trap_arith = true;
            } else {
                let result = ctx.iregs.get(*lhs)? / divisor;
                ctx.iregs.set(*dst, result)?;
                ctx.psw.update_from_arithmetic(result);
            }
        }

        Instruction::IMod { dst, lhs, rhs } => {
            let divisor = ctx.iregs.get(*rhs)?;
            if divisor == 0 {
                ctx.iregs.set(*dst, 0)?;
                ctx.psw.trap_arith = true;
            } else {
                let result = ctx.iregs.get(*lhs)? % divisor;
                ctx.iregs.set(*dst, result)?;
                ctx.psw.update_from_arithmetic(result);
            }
        }

        // =====================================================================
        // Integer bitwise (R-file)
        // =====================================================================

        Instruction::IAnd { dst, lhs, rhs } => {
            let result = ctx.iregs.get(*lhs)? & ctx.iregs.get(*rhs)?;
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        Instruction::IOr { dst, lhs, rhs } => {
            let result = ctx.iregs.get(*lhs)? | ctx.iregs.get(*rhs)?;
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        Instruction::IXor { dst, lhs, rhs } => {
            let result = ctx.iregs.get(*lhs)? ^ ctx.iregs.get(*rhs)?;
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        Instruction::INot { dst, src } => {
            let result = !ctx.iregs.get(*src)?;
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        Instruction::IShl { dst, src, amt } => {
            let safe_amt = std::cmp::min(*amt, 63) as u32;
            let result = ctx.iregs.get(*src)? << safe_amt;
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        Instruction::IShr { dst, src, amt } => {
            let safe_amt = std::cmp::min(*amt, 63) as u32;
            let result = ctx.iregs.get(*src)? >> safe_amt;
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_arithmetic(result);
        }

        // =====================================================================
        // Integer memory
        // =====================================================================

        Instruction::ILdi { dst, imm } => {
            let result = *imm as i64;
            ctx.iregs.set(*dst, result)?;
        }

        Instruction::ILdm { dst, addr } => {
            let result = ctx.cmem.load(*addr);
            ctx.iregs.set(*dst, result)?;
        }

        Instruction::IStr { src, addr } => {
            let val = ctx.iregs.get(*src)?;
            ctx.cmem.store(*addr, val);
        }

        // =====================================================================
        // Integer comparison
        // =====================================================================

        Instruction::IEq { dst, lhs, rhs } => {
            let result = if ctx.iregs.get(*lhs)? == ctx.iregs.get(*rhs)? { 1i64 } else { 0 };
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_predicate(result != 0);
        }

        Instruction::ILt { dst, lhs, rhs } => {
            let result = if ctx.iregs.get(*lhs)? < ctx.iregs.get(*rhs)? { 1i64 } else { 0 };
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_predicate(result != 0);
        }

        Instruction::IGt { dst, lhs, rhs } => {
            let result = if ctx.iregs.get(*lhs)? > ctx.iregs.get(*rhs)? { 1i64 } else { 0 };
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_predicate(result != 0);
        }

        // =====================================================================
        // Float arithmetic (F-file)
        // =====================================================================

        Instruction::FAdd { dst, lhs, rhs } => {
            let result = ctx.fregs.get(*lhs)? + ctx.fregs.get(*rhs)?;
            ctx.fregs.set(*dst, result)?;
        }

        Instruction::FSub { dst, lhs, rhs } => {
            let result = ctx.fregs.get(*lhs)? - ctx.fregs.get(*rhs)?;
            ctx.fregs.set(*dst, result)?;
        }

        Instruction::FMul { dst, lhs, rhs } => {
            let result = ctx.fregs.get(*lhs)? * ctx.fregs.get(*rhs)?;
            ctx.fregs.set(*dst, result)?;
        }

        Instruction::FDiv { dst, lhs, rhs } => {
            let divisor = ctx.fregs.get(*rhs)?;
            if divisor == 0.0 {
                return Err(CqamError::DivisionByZero {
                    instruction: "FDIV".to_string(),
                });
            }
            let result = ctx.fregs.get(*lhs)? / divisor;
            ctx.fregs.set(*dst, result)?;
        }

        Instruction::FLdi { dst, imm } => {
            ctx.fregs.set(*dst, *imm as f64)?;
        }

        Instruction::FLdm { dst, addr } => {
            let bits = ctx.cmem.load(*addr) as u64;
            ctx.fregs.set(*dst, f64::from_bits(bits))?;
        }

        Instruction::FStr { src, addr } => {
            let bits = ctx.fregs.get(*src)?.to_bits() as i64;
            ctx.cmem.store(*addr, bits);
        }

        Instruction::FEq { dst, lhs, rhs } => {
            let result = if ctx.fregs.get(*lhs)? == ctx.fregs.get(*rhs)? { 1i64 } else { 0 };
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_predicate(result != 0);
        }

        Instruction::FLt { dst, lhs, rhs } => {
            let result = if ctx.fregs.get(*lhs)? < ctx.fregs.get(*rhs)? { 1i64 } else { 0 };
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_predicate(result != 0);
        }

        Instruction::FGt { dst, lhs, rhs } => {
            let result = if ctx.fregs.get(*lhs)? > ctx.fregs.get(*rhs)? { 1i64 } else { 0 };
            ctx.iregs.set(*dst, result)?;
            ctx.psw.update_from_predicate(result != 0);
        }

        // =====================================================================
        // Complex arithmetic (Z-file)
        // =====================================================================

        Instruction::ZAdd { dst, lhs, rhs } => {
            let (ar, ai) = ctx.zregs.get(*lhs)?;
            let (br, bi) = ctx.zregs.get(*rhs)?;
            ctx.zregs.set(*dst, (ar + br, ai + bi))?;
        }

        Instruction::ZSub { dst, lhs, rhs } => {
            let (ar, ai) = ctx.zregs.get(*lhs)?;
            let (br, bi) = ctx.zregs.get(*rhs)?;
            ctx.zregs.set(*dst, (ar - br, ai - bi))?;
        }

        Instruction::ZMul { dst, lhs, rhs } => {
            let (ar, ai) = ctx.zregs.get(*lhs)?;
            let (br, bi) = ctx.zregs.get(*rhs)?;
            ctx.zregs.set(*dst, (ar * br - ai * bi, ar * bi + ai * br))?;
        }

        Instruction::ZDiv { dst, lhs, rhs } => {
            let (ar, ai) = ctx.zregs.get(*lhs)?;
            let (br, bi) = ctx.zregs.get(*rhs)?;
            let denom = br * br + bi * bi;
            if denom == 0.0 {
                return Err(CqamError::DivisionByZero {
                    instruction: "ZDIV".to_string(),
                });
            }
            let re = (ar * br + ai * bi) / denom;
            let im = (ai * br - ar * bi) / denom;
            ctx.zregs.set(*dst, (re, im))?;
        }

        Instruction::ZLdi { dst, imm_re, imm_im } => {
            ctx.zregs.set(*dst, (*imm_re as f64, *imm_im as f64))?;
        }

        Instruction::ZLdm { dst, addr } => {
            let re_bits = ctx.cmem.load(*addr) as u64;
            let im_bits = ctx.cmem.load(addr.wrapping_add(1)) as u64;
            ctx.zregs.set(*dst, (f64::from_bits(re_bits), f64::from_bits(im_bits)))?;
        }

        Instruction::ZStr { src, addr } => {
            let (re, im) = ctx.zregs.get(*src)?;
            ctx.cmem.store(*addr, re.to_bits() as i64);
            ctx.cmem.store(addr.wrapping_add(1), im.to_bits() as i64);
        }

        // =====================================================================
        // Register-indirect memory
        // =====================================================================

        Instruction::ILdx { dst, addr_reg } => {
            let raw_addr = ctx.iregs.get(*addr_reg)?;
            let addr = validate_indirect_addr(raw_addr, 0xFFFF, "ILDX")?;
            let result = ctx.cmem.load(addr);
            ctx.iregs.set(*dst, result)?;
        }

        Instruction::IStrx { src, addr_reg } => {
            let raw_addr = ctx.iregs.get(*addr_reg)?;
            let addr = validate_indirect_addr(raw_addr, 0xFFFF, "ISTRX")?;
            let val = ctx.iregs.get(*src)?;
            ctx.cmem.store(addr, val);
        }

        Instruction::FLdx { dst, addr_reg } => {
            let raw_addr = ctx.iregs.get(*addr_reg)?;
            let addr = validate_indirect_addr(raw_addr, 0xFFFF, "FLDX")?;
            let bits = ctx.cmem.load(addr) as u64;
            ctx.fregs.set(*dst, f64::from_bits(bits))?;
        }

        Instruction::FStrx { src, addr_reg } => {
            let raw_addr = ctx.iregs.get(*addr_reg)?;
            let addr = validate_indirect_addr(raw_addr, 0xFFFF, "FSTRX")?;
            let bits = ctx.fregs.get(*src)?.to_bits() as i64;
            ctx.cmem.store(addr, bits);
        }

        Instruction::ZLdx { dst, addr_reg } => {
            let raw_addr = ctx.iregs.get(*addr_reg)?;
            let addr = validate_indirect_addr(raw_addr, 0xFFFE, "ZLDX")?;
            let re_bits = ctx.cmem.load(addr) as u64;
            let im_bits = ctx.cmem.load(addr.wrapping_add(1)) as u64;
            ctx.zregs.set(*dst, (f64::from_bits(re_bits), f64::from_bits(im_bits)))?;
        }

        Instruction::ZStrx { src, addr_reg } => {
            let raw_addr = ctx.iregs.get(*addr_reg)?;
            let addr = validate_indirect_addr(raw_addr, 0xFFFE, "ZSTRX")?;
            let (re, im) = ctx.zregs.get(*src)?;
            ctx.cmem.store(addr, re.to_bits() as i64);
            ctx.cmem.store(addr.wrapping_add(1), im.to_bits() as i64);
        }

        // =====================================================================
        // Type conversion
        // =====================================================================

        Instruction::CvtIF { dst_f, src_i } => {
            ctx.fregs.set(*dst_f, ctx.iregs.get(*src_i)? as f64)?;
        }

        Instruction::CvtFI { dst_i, src_f } => {
            ctx.iregs.set(*dst_i, ctx.fregs.get(*src_f)? as i64)?;
        }

        Instruction::CvtFZ { dst_z, src_f } => {
            ctx.zregs.set(*dst_z, (ctx.fregs.get(*src_f)?, 0.0))?;
        }

        Instruction::CvtZF { dst_f, src_z } => {
            let (re, _im) = ctx.zregs.get(*src_z)?;
            ctx.fregs.set(*dst_f, re)?;
        }

        // =====================================================================
        // Control flow
        // =====================================================================

        Instruction::Jmp { target } => {
            ctx.jump_to_label(target)?;
            return Ok(()); // Do NOT advance PC
        }

        Instruction::Jif { pred, target } => {
            if ctx.iregs.get(*pred)? != 0 {
                ctx.jump_to_label(target)?;
                return Ok(()); // Jump taken: do NOT advance PC
            }
            // Fall through: advance PC normally
        }

        Instruction::Call { target } => {
            ctx.push_call();
            ctx.jump_to_label(target)?;
            return Ok(()); // Do NOT advance PC
        }

        Instruction::Ret => {
            if let Some(addr) = ctx.pop_call() {
                ctx.pc = addr;
            } else {
                // Empty call stack: RET from top-level acts as HALT
                ctx.psw.trap_halt = true;
            }
            return Ok(()); // Do NOT advance PC (already set)
        }

        Instruction::Halt => {
            ctx.psw.trap_halt = true;
            return Ok(()); // Do NOT advance PC
        }

        Instruction::Reti => {
            if let Some(addr) = ctx.pop_call() {
                ctx.pc = addr;
                ctx.psw.clear_maskable_traps();
            } else {
                // Empty call stack: RETI from top-level acts as HALT
                ctx.psw.trap_halt = true;
            }
            return Ok(()); // Do NOT advance PC (already set)
        }

        Instruction::SetIV { trap_id, target } => {
            let mt = trap_id_to_maskable(*trap_id)?;
            let addr = *ctx.labels.get(target.as_str())
                .ok_or_else(|| CqamError::UnresolvedLabel(target.clone()))?;
            ctx.isr_table.set_handler(&Trap::Maskable(mt), addr);
        }

        // =====================================================================
        // Quantum -- delegate to qop.rs
        // =====================================================================

        Instruction::QPrep { .. }
        | Instruction::QKernel { .. }
        | Instruction::QObserve { .. }
        | Instruction::QLoad { .. }
        | Instruction::QStore { .. } => {
            execute_qop(ctx, instr)?;
        }

        // =====================================================================
        // Hybrid -- delegate to hybrid.rs
        // =====================================================================

        Instruction::HFork
        | Instruction::HMerge
        | Instruction::HCExec { .. }
        | Instruction::HReduce { .. } => {
            let jumped = execute_hybrid(ctx, instr, fork_mgr)?;
            if jumped {
                return Ok(()); // HCExec took a jump: do NOT advance PC
            }
        }

        // =====================================================================
        // Labels and Nops -- no execution, just advance PC
        // =====================================================================

        Instruction::Label(_) | Instruction::Nop => {}
    }

    // Apply resource cost and advance PC
    let delta = resource_cost(instr);
    ctx.resource_tracker.apply_delta(&delta);
    ctx.advance_pc();
    Ok(())
}

/// Convert a trap_id (u8) to a MaskableTrap variant.
fn trap_id_to_maskable(trap_id: u8) -> Result<MaskableTrap, CqamError> {
    match trap_id {
        0 => Ok(MaskableTrap::Arithmetic),
        1 => Ok(MaskableTrap::QuantumError),
        2 => Ok(MaskableTrap::SyncFailure),
        _ => Err(CqamError::TypeMismatch {
            instruction: "SETIV".to_string(),
            detail: format!("Invalid trap ID: {} (must be 0-2)", trap_id),
        }),
    }
}

/// Validate that an i64 register value is a legal CMEM address.
///
/// Returns the validated address as u16 on success.
/// Returns CqamError::AddressOutOfRange if value is negative or > max_addr.
fn validate_indirect_addr(val: i64, max_addr: u16, instruction: &str) -> Result<u16, CqamError> {
    if val < 0 || val > max_addr as i64 {
        return Err(CqamError::AddressOutOfRange {
            instruction: instruction.to_string(),
            address: val,
        });
    }
    Ok(val as u16)
}

/// Run a full program to termination.
///
/// Returns `Ok(())` on normal completion, or `Err(CqamError)` on runtime error.
///
/// This is also the execution loop used by fork threads (with their own
/// nested ForkManager).
pub fn run_program(ctx: &mut ExecutionContext, fork_mgr: &mut ForkManager) -> Result<(), CqamError> {
    while ctx.current_line().is_some() {
        // Clone is required here due to Rust's borrow rules: ctx.program[pc]
        // borrows ctx immutably, but execute_instruction needs &mut ctx.
        // The cost is O(1) for most variants; only String-containing variants
        // (Label, Jmp, Jif, Call, HCExec) allocate, and these are a small
        // fraction of typical execution. Eliminating this clone would require
        // splitting ExecutionContext into separate immutable/mutable parts.
        let instr = ctx.program[ctx.pc].clone();
        execute_instruction(ctx, &instr, fork_mgr)?;

        if ctx.psw.trap_halt {
            break;
        }
    }
    Ok(())
}
