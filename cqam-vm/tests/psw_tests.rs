//! Tests for `ProgramStateWord`: flag updates, `get_flag` numeric access,
//! pending trap priority, and maskable trap clearing.

use cqam_vm::psw::{ProgramStateWord, PendingTrap};

#[test]
fn test_arithmetic_flag_update() {
    let mut psw = ProgramStateWord::new();
    psw.update_from_arithmetic(0);
    assert!(psw.zf);
    assert!(!psw.nf);

    psw.update_from_arithmetic(-5);
    assert!(psw.nf);
}

#[test]
fn test_predicate_flag() {
    let mut psw = ProgramStateWord::new();
    psw.update_from_predicate(true);
    assert!(psw.pf);
}

#[test]
fn test_quantum_flag_update_no_interrupt() {
    let mut psw = ProgramStateWord::new();
    // Pure state (purity=1.0) with threshold 0.95 should not trigger alarm
    psw.sf = true;
    psw.ef = true;
    psw.update_from_qmeta(1.0, 0.95);
    assert!(psw.qf);
    // update_from_qmeta does NOT modify SF/EF (they are intent-based)
    assert!(psw.sf);
    assert!(psw.ef);
    assert!(!psw.int_quantum_err);
}

#[test]
fn test_quantum_interrupt_triggered() {
    let mut psw = ProgramStateWord::new();
    // Low purity (0.3) below threshold (0.5) should trigger alarm
    psw.update_from_qmeta(0.3, 0.5);
    assert!(psw.int_quantum_err);
    assert_eq!(
        psw.check_pending_traps(),
        Some(PendingTrap::QuantumError)
    );
}

#[test]
fn test_purity_threshold_pure_state_no_alarm() {
    let mut psw = ProgramStateWord::new();
    psw.update_from_qmeta(1.0, 0.95);
    assert!(!psw.int_quantum_err);
}

#[test]
fn test_purity_threshold_mixed_state_alarm() {
    let mut psw = ProgramStateWord::new();
    psw.update_from_qmeta(0.8, 0.95);
    assert!(psw.int_quantum_err);
}

#[test]
fn test_purity_threshold_disabled() {
    let mut psw = ProgramStateWord::new();
    psw.update_from_qmeta(0.5, 0.0);
    assert!(!psw.int_quantum_err);
}

#[test]
fn test_purity_threshold_boundary() {
    let mut psw = ProgramStateWord::new();
    // At threshold: not below, so no alarm
    psw.update_from_qmeta(0.95, 0.95);
    assert!(!psw.int_quantum_err);

    // Just below threshold: alarm
    let mut psw2 = ProgramStateWord::new();
    psw2.update_from_qmeta(0.9499, 0.95);
    assert!(psw2.int_quantum_err);
}

#[test]
fn test_get_flag_by_id() {
    let mut psw = ProgramStateWord::new();

    // Set some flags
    psw.zf = true;
    psw.qf = true;
    psw.hf = true;
    psw.df = true;
    psw.cf = true;
    psw.forked = true;
    psw.merged = true;

    assert!(psw.get_flag(0));   // ZF
    assert!(!psw.get_flag(1));  // NF (not set)
    assert!(!psw.get_flag(2));  // OF (not set)
    assert!(!psw.get_flag(3));  // PF (not set)
    assert!(psw.get_flag(4));   // QF
    assert!(!psw.get_flag(5));  // SF (not set)
    assert!(!psw.get_flag(6));  // EF (not set)
    assert!(psw.get_flag(7));   // HF
    assert!(psw.get_flag(8));   // DF
    assert!(psw.get_flag(9));   // CF
    assert!(psw.get_flag(10));  // FK (forked)
    assert!(psw.get_flag(11));  // MG (merged)

    psw.inf = true;
    assert!(psw.get_flag(12));  // IF (interference)

    assert!(!psw.get_flag(255)); // out of range -> false
}

#[test]
fn test_mark_decohered_and_collapsed() {
    let mut psw = ProgramStateWord::new();
    psw.mark_decohered();
    assert!(psw.df);
    assert!(!psw.cf);

    psw.mark_collapsed();
    assert!(psw.df);
    assert!(psw.cf);
}

#[test]
fn test_clear_collapsed() {
    let mut psw = ProgramStateWord::new();
    psw.mark_collapsed();
    assert!(psw.cf);
    psw.clear_collapsed();
    assert!(!psw.cf);
}

#[test]
fn test_clear_decoherence() {
    let mut psw = ProgramStateWord::new();
    psw.mark_decohered();
    assert!(psw.df);
    psw.clear_decoherence();
    assert!(!psw.df);
}

#[test]
fn test_df_sticky_in_update_from_qmeta() {
    let mut psw = ProgramStateWord::new();
    // Set DF via mark_decohered
    psw.mark_decohered();
    assert!(psw.df);

    // update_from_qmeta with pure state should NOT clear DF (sticky)
    psw.update_from_qmeta(1.0, 0.0);
    assert!(psw.df, "DF must be sticky: update_from_qmeta with purity=1.0 must not clear it");
}

#[test]
fn test_df_set_by_low_purity() {
    let mut psw = ProgramStateWord::new();
    assert!(!psw.df);

    // update_from_qmeta with low purity should set DF
    psw.update_from_qmeta(0.5, 0.0);
    assert!(psw.df, "DF should be set when purity < 1.0");
}

#[test]
fn test_cf_transient_in_update_from_qmeta() {
    let mut psw = ProgramStateWord::new();
    psw.mark_collapsed();
    assert!(psw.cf);

    // update_from_qmeta should clear CF (transient)
    psw.update_from_qmeta(1.0, 0.0);
    assert!(!psw.cf, "CF must be transient: update_from_qmeta must clear it");
}

#[test]
fn test_clear() {
    let mut psw = ProgramStateWord::new();
    psw.zf = true;
    psw.nf = true;
    psw.qf = true;
    psw.df = true;
    psw.cf = true;
    psw.forked = true;
    psw.merged = true;
    psw.trap_halt = true;
    psw.clear();

    assert!(!psw.zf);
    assert!(!psw.nf);
    assert!(!psw.qf);
    assert!(!psw.df);
    assert!(!psw.cf);
    assert!(!psw.forked);
    assert!(!psw.merged);
    assert!(!psw.trap_halt);
}

// --- Trap clearing and pending trap checks ---

#[test]
fn test_clear_maskable_traps_clears_arith_quantum_sync() {
    let mut psw = ProgramStateWord::new();
    psw.trap_arith = true;
    psw.int_quantum_err = true;
    psw.int_sync_fail = true;

    psw.clear_maskable_traps();

    assert!(!psw.trap_arith, "trap_arith should be cleared");
    assert!(!psw.int_quantum_err, "int_quantum_err should be cleared");
    assert!(!psw.int_sync_fail, "int_sync_fail should be cleared");
}

#[test]
fn test_clear_maskable_traps_does_not_clear_halt() {
    let mut psw = ProgramStateWord::new();
    psw.trap_halt = true;
    psw.trap_arith = true;

    psw.clear_maskable_traps();

    assert!(psw.trap_halt, "trap_halt (NMI-level) must NOT be cleared by clear_maskable_traps");
    assert!(!psw.trap_arith, "trap_arith should be cleared");
}

#[test]
fn test_check_pending_traps_priority_order() {
    // When multiple traps are pending, the highest-priority one is returned.
    // Priority: halt > arith > quantum_err > sync_fail
    let mut psw = ProgramStateWord::new();

    // All maskable traps pending
    psw.trap_arith = true;
    psw.int_quantum_err = true;
    psw.int_sync_fail = true;
    assert_eq!(psw.check_pending_traps(), Some(PendingTrap::Arithmetic));

    // Remove arith, quantum_err should be next
    psw.trap_arith = false;
    assert_eq!(psw.check_pending_traps(), Some(PendingTrap::QuantumError));

    // Remove quantum_err, sync_fail should be next
    psw.int_quantum_err = false;
    assert_eq!(psw.check_pending_traps(), Some(PendingTrap::SyncFailure));

    // Remove sync_fail, none pending
    psw.int_sync_fail = false;
    assert_eq!(psw.check_pending_traps(), None);
}

#[test]
fn test_check_pending_traps_halt_overrides_all() {
    let mut psw = ProgramStateWord::new();
    psw.trap_halt = true;
    psw.trap_arith = true;
    psw.int_quantum_err = true;
    psw.int_sync_fail = true;

    assert_eq!(psw.check_pending_traps(), Some(PendingTrap::Halt));
}
