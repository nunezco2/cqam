//! RegisterSnapshot for change detection between execution steps.
//!
//! A lightweight copy of all classical register values taken before each step,
//! used to detect which values changed after execution.

use cqam_vm::context::ExecutionContext;

/// A snapshot of all classical register values and PSW flags at a point in time.
///
/// Used to diff against the current state after an instruction executes, so
/// the STATE pane can highlight changed values.
#[derive(Debug, Clone)]
pub struct RegisterSnapshot {
    /// Integer register values: R0--R15.
    pub iregs: [i64; 16],
    /// Float register values: F0--F15.
    pub fregs: [f64; 16],
    /// Complex register values: Z0--Z15.
    pub zregs: [(f64, f64); 16],
    /// PSW flags in flag_id order: ZF, NF, OF, PF, QF, SF, EF, HF, DF, CF, FK, MG.
    pub psw_flags: [bool; 12],
}

impl RegisterSnapshot {
    /// Create a snapshot from the current execution context.
    pub fn capture(ctx: &ExecutionContext) -> Self {
        let mut iregs = [0i64; 16];
        for i in 0..16 {
            iregs[i] = ctx.iregs.regs[i];
        }

        let mut fregs = [0.0f64; 16];
        for i in 0..16 {
            fregs[i] = ctx.fregs.regs[i];
        }

        let mut zregs = [(0.0f64, 0.0f64); 16];
        for i in 0..16 {
            zregs[i] = ctx.zregs.regs[i];
        }

        let psw_flags = [
            ctx.psw.zf,
            ctx.psw.nf,
            ctx.psw.of,
            ctx.psw.pf,
            ctx.psw.qf,
            ctx.psw.sf,
            ctx.psw.ef,
            ctx.psw.hf,
            ctx.psw.df,
            ctx.psw.cf,
            ctx.psw.forked,
            ctx.psw.merged,
        ];

        Self {
            iregs,
            fregs,
            zregs,
            psw_flags,
        }
    }

    /// Check if integer register `idx` changed between this snapshot and current state.
    pub fn ireg_changed(&self, ctx: &ExecutionContext, idx: usize) -> bool {
        idx < 16 && self.iregs[idx] != ctx.iregs.regs[idx]
    }

    /// Check if float register `idx` changed between this snapshot and current state.
    pub fn freg_changed(&self, ctx: &ExecutionContext, idx: usize) -> bool {
        idx < 16 && self.fregs[idx] != ctx.fregs.regs[idx]
    }

    /// Check if complex register `idx` changed between this snapshot and current state.
    pub fn zreg_changed(&self, ctx: &ExecutionContext, idx: usize) -> bool {
        if idx >= 16 {
            return false;
        }
        let (old_re, old_im) = self.zregs[idx];
        let (new_re, new_im) = ctx.zregs.regs[idx];
        old_re != new_re || old_im != new_im
    }

    /// Check if PSW flag `flag_id` changed between this snapshot and current state.
    pub fn psw_flag_changed(&self, ctx: &ExecutionContext, flag_id: usize) -> bool {
        if flag_id >= 12 {
            return false;
        }
        let current = ctx.psw.get_flag(flag_id as u8);
        self.psw_flags[flag_id] != current
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::instruction::Instruction;

    fn make_ctx() -> ExecutionContext {
        ExecutionContext::new(vec![Instruction::Halt])
    }

    #[test]
    fn test_capture_and_no_change() {
        let ctx = make_ctx();
        let snap = RegisterSnapshot::capture(&ctx);
        // No changes yet -- all should report false.
        for i in 0..16 {
            assert!(!snap.ireg_changed(&ctx, i));
            assert!(!snap.freg_changed(&ctx, i));
            assert!(!snap.zreg_changed(&ctx, i));
        }
    }

    #[test]
    fn test_detect_ireg_change() {
        let mut ctx = make_ctx();
        let snap = RegisterSnapshot::capture(&ctx);
        ctx.iregs.regs[3] = 42;
        assert!(snap.ireg_changed(&ctx, 3));
        assert!(!snap.ireg_changed(&ctx, 0));
    }

    #[test]
    fn test_detect_freg_change() {
        let mut ctx = make_ctx();
        let snap = RegisterSnapshot::capture(&ctx);
        ctx.fregs.regs[1] = 3.14;
        assert!(snap.freg_changed(&ctx, 1));
        assert!(!snap.freg_changed(&ctx, 0));
    }

    #[test]
    fn test_detect_zreg_change() {
        let mut ctx = make_ctx();
        let snap = RegisterSnapshot::capture(&ctx);
        ctx.zregs.regs[5] = (1.0, 2.0);
        assert!(snap.zreg_changed(&ctx, 5));
        assert!(!snap.zreg_changed(&ctx, 0));
    }

}
