use cqam_vm::psw::{ProgramStateWord, Trap};

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
    assert_eq!(psw.check_interrupts(), Some(Trap::QuantumError));
}
