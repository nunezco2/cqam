//! Integration tests for DF, CF, FK, MG PSW flag semantics.
//!
//! These tests construct `ExecutionContext` instances with hand-built instruction
//! sequences and verify flag transitions through quantum and hybrid operations.

use cqam_core::instruction::*;
use cqam_vm::context::ExecutionContext;
use cqam_vm::fork::ForkManager;
use cqam_vm::hybrid::execute_hybrid;
use cqam_vm::qop::execute_qop;

// =============================================================================
// DF (flag_id=8, sticky): set by QOBSERVE/QMEAS, cleared by QPREP or clear()
// =============================================================================

#[test]
fn test_df_set_by_qobserve() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
    )
    .unwrap();

    // Before observation, DF and CF should be false.
    assert!(!ctx.psw.df, "DF should be false after QPREP");
    assert!(!ctx.psw.cf, "CF should be false after QPREP");

    execute_qop(
        &mut ctx,
        &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 },
    )
    .unwrap();

    assert!(ctx.psw.df, "DF should be true after QOBSERVE");
    assert!(ctx.psw.cf, "CF should be true after QOBSERVE");
}

#[test]
fn test_df_set_by_qmeas() {
    let mut ctx = ExecutionContext::new(vec![]);

    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: dist_id::ZERO },
    )
    .unwrap();

    // QMEAS measures a single qubit (qubit index comes from an integer register).
    ctx.iregs.set(0, 0).unwrap(); // qubit index 0
    execute_qop(
        &mut ctx,
        &Instruction::QMeas { dst_r: 1, src_q: 0, qubit_reg: 0 },
    )
    .unwrap();

    assert!(ctx.psw.df, "DF should be true after QMEAS");
    assert!(ctx.psw.cf, "CF should be true after QMEAS");
}

// =============================================================================
// CF (flag_id=9, transient): cleared by HREDUCE and by QKERNEL update_from_qmeta
// =============================================================================

#[test]
fn test_cf_cleared_by_update_from_qmeta() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Step 1: Prepare the register we will use for QKERNEL *before* setting DF/CF,
    // because QPREP clears DF.
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 1, dist: dist_id::UNIFORM },
    )
    .unwrap();

    // Step 2: QPREP -> QOBSERVE on a different register to set both DF and CF.
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
    )
    .unwrap();
    execute_qop(
        &mut ctx,
        &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 },
    )
    .unwrap();

    assert!(ctx.psw.df, "DF should be true after QOBSERVE");
    assert!(ctx.psw.cf, "CF should be true after QOBSERVE");

    // Step 3: Apply QKERNEL on the pre-prepared register. QKERNEL calls
    // update_from_qmeta() which clears CF but does NOT clear DF (sticky).
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();
    execute_qop(
        &mut ctx,
        &Instruction::QKernel {
            dst: 2,
            src: 1,
            kernel: kernel_id::INIT,
            ctx0: 0,
            ctx1: 1,
        },
    )
    .unwrap();

    assert!(
        ctx.psw.df,
        "DF must remain true after QKERNEL (sticky across kernels)"
    );
    assert!(
        !ctx.psw.cf,
        "CF must be false after QKERNEL (transient, cleared by update_from_qmeta)"
    );
}

#[test]
fn test_cf_cleared_by_hreduce() {
    let mut ctx = ExecutionContext::new(vec![]);

    // QOBSERVE sets CF.
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
    )
    .unwrap();
    execute_qop(
        &mut ctx,
        &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 },
    )
    .unwrap();
    assert!(ctx.psw.cf, "CF should be true after QOBSERVE");

    // HREDUCE consumes the measurement result and clears CF.
    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MODE },
        &mut fm,
    )
    .unwrap();

    assert!(
        !ctx.psw.cf,
        "CF should be false after HREDUCE (consuming measurement clears collapsed signal)"
    );
    assert!(
        ctx.psw.df,
        "DF must remain true after HREDUCE (sticky)"
    );
}

// =============================================================================
// DF cleared by QPREP
// =============================================================================

#[test]
fn test_df_cleared_by_qprep() {
    let mut ctx = ExecutionContext::new(vec![]);

    // Set DF via QOBSERVE.
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: dist_id::UNIFORM },
    )
    .unwrap();
    execute_qop(
        &mut ctx,
        &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 },
    )
    .unwrap();
    assert!(ctx.psw.df, "DF should be set after QOBSERVE");

    // A new QPREP clears DF (fresh quantum state, no decoherence).
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: dist_id::ZERO },
    )
    .unwrap();

    assert!(
        !ctx.psw.df,
        "DF should be false after QPREP (re-initialisation clears decoherence)"
    );
    assert!(
        !ctx.psw.cf,
        "CF should be false after QPREP"
    );
}

// =============================================================================
// FK (flag_id=10) and MG (flag_id=11) lifecycle
// =============================================================================

#[test]
fn test_fk_mg_lifecycle() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();

    // Initially both should be false.
    assert!(!ctx.psw.forked, "FK should be false initially");
    assert!(!ctx.psw.merged, "MG should be false initially");

    // After HFORK: FK=true, MG=false.
    execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm).unwrap();
    assert!(ctx.psw.forked, "FK should be true after HFORK");
    assert!(!ctx.psw.merged, "MG should be false after HFORK");

    // After HMERGE: FK=false, MG=true.
    execute_hybrid(&mut ctx, &Instruction::HMerge, &mut fm).unwrap();
    assert!(!ctx.psw.forked, "FK should be false after HMERGE");
    assert!(ctx.psw.merged, "MG should be true after HMERGE");
}

#[test]
fn test_mg_cleared_by_hfork() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();

    // First establish MG=true via HFORK then HMERGE.
    execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm).unwrap();
    execute_hybrid(&mut ctx, &Instruction::HMerge, &mut fm).unwrap();
    assert!(ctx.psw.merged, "MG should be true after HMERGE");

    // HFORK should clear MG and set FK.
    execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm).unwrap();
    assert!(
        !ctx.psw.merged,
        "MG should be false after HFORK (HFORK clears MG)"
    );
    assert!(
        ctx.psw.forked,
        "FK should be true after HFORK"
    );
}

#[test]
fn test_fk_cleared_by_hmerge() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();

    // First establish FK=true via HFORK.
    execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm).unwrap();
    assert!(ctx.psw.forked, "FK should be true after HFORK");

    // HMERGE should clear FK and set MG.
    execute_hybrid(&mut ctx, &Instruction::HMerge, &mut fm).unwrap();
    assert!(
        !ctx.psw.forked,
        "FK should be false after HMERGE (HMERGE clears FK)"
    );
    assert!(
        ctx.psw.merged,
        "MG should be true after HMERGE"
    );
}

// =============================================================================
// get_flag() access for IDs 8 through 11
// =============================================================================

#[test]
fn test_get_flag_ids_8_through_11() {
    let mut ctx = ExecutionContext::new(vec![]);

    // All four flags start as false.
    assert!(!ctx.psw.get_flag(flag_id::DF), "DF should be false initially");
    assert!(!ctx.psw.get_flag(flag_id::CF), "CF should be false initially");
    assert!(!ctx.psw.get_flag(flag_id::FK), "FK should be false initially");
    assert!(!ctx.psw.get_flag(flag_id::MG), "MG should be false initially");

    // Set DF only.
    ctx.psw.mark_decohered();
    assert!(ctx.psw.get_flag(flag_id::DF), "get_flag(8) should return true for DF");
    assert!(!ctx.psw.get_flag(flag_id::CF), "CF should remain false");
    assert!(!ctx.psw.get_flag(flag_id::FK), "FK should remain false");
    assert!(!ctx.psw.get_flag(flag_id::MG), "MG should remain false");

    // Set CF only (additive).
    ctx.psw.mark_collapsed();
    assert!(ctx.psw.get_flag(flag_id::DF), "DF should still be true");
    assert!(ctx.psw.get_flag(flag_id::CF), "get_flag(9) should return true for CF");

    // Set FK only.
    ctx.psw.forked = true;
    assert!(ctx.psw.get_flag(flag_id::FK), "get_flag(10) should return true for FK");
    assert!(!ctx.psw.get_flag(flag_id::MG), "MG should still be false");

    // Set MG only.
    ctx.psw.merged = true;
    assert!(ctx.psw.get_flag(flag_id::MG), "get_flag(11) should return true for MG");

    // Clear everything and verify all are false.
    ctx.psw.clear();
    assert!(!ctx.psw.get_flag(flag_id::DF), "DF should be false after clear()");
    assert!(!ctx.psw.get_flag(flag_id::CF), "CF should be false after clear()");
    assert!(!ctx.psw.get_flag(flag_id::FK), "FK should be false after clear()");
    assert!(!ctx.psw.get_flag(flag_id::MG), "MG should be false after clear()");
}

// =============================================================================
// Combined lifecycle: QPREP -> QOBSERVE -> QKERNEL -> QPREP
// =============================================================================

#[test]
fn test_full_df_cf_lifecycle() {
    let mut ctx = ExecutionContext::new(vec![]);

    // 1. QPREP: fresh state, DF=false, CF=false.
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 0, dist: dist_id::BELL },
    )
    .unwrap();
    assert!(!ctx.psw.df, "DF false after initial QPREP");
    assert!(!ctx.psw.cf, "CF false after initial QPREP");

    // 2. QOBSERVE: sets DF and CF.
    execute_qop(
        &mut ctx,
        &Instruction::QObserve { dst_h: 0, src_q: 0, mode: 0, ctx0: 0, ctx1: 0 },
    )
    .unwrap();
    assert!(ctx.psw.df, "DF true after QOBSERVE");
    assert!(ctx.psw.cf, "CF true after QOBSERVE");

    // 3. QKERNEL: CF is cleared (transient), DF remains (sticky).
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 1, dist: dist_id::UNIFORM },
    )
    .unwrap();
    // Note: QPREP above also clears DF. To test the QKERNEL CF-clearing path
    // properly, we need to re-set DF manually first, then observe, then kernel.
    // Let's restart the lifecycle more carefully:

    // Reset for a clean QKERNEL CF-clearing test.
    ctx.psw.clear();
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 2, dist: dist_id::UNIFORM },
    )
    .unwrap();
    execute_qop(
        &mut ctx,
        &Instruction::QObserve { dst_h: 1, src_q: 2, mode: 0, ctx0: 0, ctx1: 0 },
    )
    .unwrap();
    assert!(ctx.psw.df, "DF set after second QOBSERVE");
    assert!(ctx.psw.cf, "CF set after second QOBSERVE");

    // Apply QKERNEL on a different register to trigger update_from_qmeta.
    ctx.iregs.set(0, 0).unwrap();
    ctx.iregs.set(1, 0).unwrap();
    execute_qop(
        &mut ctx,
        &Instruction::QKernel {
            dst: 3,
            src: 1,  // Q1 was prepared above
            kernel: kernel_id::INIT,
            ctx0: 0,
            ctx1: 1,
        },
    )
    .unwrap();
    assert!(ctx.psw.df, "DF must be sticky through QKERNEL");
    assert!(!ctx.psw.cf, "CF must be cleared by QKERNEL");

    // 4. QPREP clears DF.
    execute_qop(
        &mut ctx,
        &Instruction::QPrep { dst: 4, dist: dist_id::ZERO },
    )
    .unwrap();
    assert!(!ctx.psw.df, "DF false after QPREP (re-initialisation)");
    assert!(!ctx.psw.cf, "CF false after QPREP");
}
