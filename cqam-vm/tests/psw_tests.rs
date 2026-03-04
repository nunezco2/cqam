// cqam-vm/tests/psw_tests.rs
//
// Phase 2: Test PSW including new get_flag() method.

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
    psw.update_from_qmeta(0.9, 0.8, (0.5, 0.5));
    assert!(psw.qf);
    assert!(psw.sf);
    assert!(psw.ef);
    assert!(!psw.int_quantum_err);
}

#[test]
fn test_quantum_interrupt_triggered() {
    let mut psw = ProgramStateWord::new();
    psw.update_from_qmeta(0.3, 0.2, (0.5, 0.5));
    assert!(psw.int_quantum_err);
    assert_eq!(
        psw.check_pending_traps(),
        Some(PendingTrap::QuantumError)
    );
}

#[test]
fn test_get_flag_by_id() {
    let mut psw = ProgramStateWord::new();

    // Set some flags
    psw.zf = true;
    psw.qf = true;
    psw.hf = true;

    assert!(psw.get_flag(0));   // ZF
    assert!(!psw.get_flag(1));  // NF (not set)
    assert!(!psw.get_flag(2));  // OF (not set)
    assert!(!psw.get_flag(3));  // PF (not set)
    assert!(psw.get_flag(4));   // QF
    assert!(!psw.get_flag(5));  // SF (not set)
    assert!(!psw.get_flag(6));  // EF (not set)
    assert!(psw.get_flag(7));   // HF
    assert!(!psw.get_flag(255)); // out of range -> false
}

#[test]
fn test_mark_measured() {
    let mut psw = ProgramStateWord::new();
    psw.mark_measured();
    assert!(psw.df);
    assert!(psw.cf);
}

#[test]
fn test_clear() {
    let mut psw = ProgramStateWord::new();
    psw.zf = true;
    psw.nf = true;
    psw.qf = true;
    psw.trap_halt = true;
    psw.clear();

    assert!(!psw.zf);
    assert!(!psw.nf);
    assert!(!psw.qf);
    assert!(!psw.trap_halt);
}
