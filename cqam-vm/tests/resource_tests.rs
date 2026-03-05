// cqam-vm/tests/resource_tests.rs
//
// Phase 9.7: Resource tracker accuracy tests.

use cqam_core::instruction::Instruction;
use cqam_vm::resource::{ResourceTracker, resource_cost};

fn accumulate_resources(instructions: &[Instruction]) -> ResourceTracker {
    let mut tracker = ResourceTracker::new();
    for instr in instructions {
        let delta = resource_cost(instr);
        tracker.apply_delta(&delta);
    }
    tracker
}

#[test]
fn test_resource_empty_sequence() {
    let tracker = accumulate_resources(&[]);
    assert_eq!(tracker.total_time, 0);
    assert_eq!(tracker.total_space, 0);
    assert!((tracker.total_superposition).abs() < 1e-10);
    assert!((tracker.total_entanglement).abs() < 1e-10);
    assert!((tracker.total_interference).abs() < 1e-10);
}

#[test]
fn test_resource_int_arithmetic_sequence() {
    let instrs = [
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::ILdi { dst: 1, imm: 2 },
        Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 },
        Instruction::ISub { dst: 3, lhs: 0, rhs: 1 },
        Instruction::Halt,
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 5); // 1+1+1+1+1
    assert_eq!(t.total_space, 4); // 1+1+1+1+0
}

#[test]
fn test_resource_quantum_pipeline() {
    let instrs = [
        Instruction::QPrep { dst: 0, dist: 0 },
        Instruction::QKernel { dst: 1, src: 0, kernel: 1, ctx0: 0, ctx1: 0 },
        Instruction::QObserve { dst_h: 0, src_q: 1 },
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 6); // 2+3+1
    assert_eq!(t.total_space, 5); // 2+2+1
    assert!((t.total_superposition - 1.5).abs() < 1e-10); // 1.0+0.5
    assert!((t.total_entanglement - 0.7).abs() < 1e-10);
    assert!((t.total_interference - 0.3).abs() < 1e-10);
}

#[test]
fn test_resource_nop_and_label_zero_cost() {
    let instrs = [
        Instruction::Nop,
        Instruction::Label("X".into()),
        Instruction::Nop,
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 0);
    assert_eq!(t.total_space, 0);
}

#[test]
fn test_resource_control_flow() {
    let instrs = [
        Instruction::Jmp { target: "L".into() },
        Instruction::Jif { pred: 0, target: "L".into() },
        Instruction::Call { target: "F".into() },
        Instruction::Ret,
        Instruction::Halt,
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 5);
    assert_eq!(t.total_space, 0);
}

#[test]
fn test_resource_hybrid_operations() {
    let instrs = [
        Instruction::HFork,
        Instruction::HCExec { flag: 0, target: "T".into() },
        Instruction::HMerge,
        Instruction::HReduce { src: 0, dst: 0, func: 0 },
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 5); // 1+1+1+2
    assert_eq!(t.total_space, 1); // 0+0+0+1
}

#[test]
fn test_resource_mixed_program() {
    let instrs = [
        Instruction::ILdi { dst: 0, imm: 1 },                           // t=1, s=1
        Instruction::FAdd { dst: 0, lhs: 1, rhs: 2 },                   // t=1, s=1
        Instruction::QPrep { dst: 0, dist: 0 },                         // t=2, s=2, sup=1.0
        Instruction::QKernel { dst: 1, src: 0, kernel: 1, ctx0: 0, ctx1: 1 }, // t=3, s=2, sup=0.5, ent=0.7
        Instruction::QObserve { dst_h: 0, src_q: 1 },                   // t=1, s=1, int=0.3
        Instruction::HFork,                                              // t=1, s=0
        Instruction::HMerge,                                             // t=1, s=0
        Instruction::Nop,                                                // t=0, s=0
        Instruction::Halt,                                               // t=1, s=0
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 11);
    assert_eq!(t.total_space, 7);
    assert!((t.total_superposition - 1.5).abs() < 1e-10);
    assert!((t.total_entanglement - 0.7).abs() < 1e-10);
    assert!((t.total_interference - 0.3).abs() < 1e-10);
}

// ===========================================================================
// Phase 9 debugger: additional resource cost edge cases
// ===========================================================================

#[test]
fn test_resource_qload_qstore() {
    let instrs = [
        Instruction::QLoad { dst_q: 0, addr: 10 },
        Instruction::QStore { src_q: 0, addr: 20 },
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 2); // 1+1
    assert_eq!(t.total_space, 2); // 1+1
    assert!((t.total_superposition).abs() < 1e-10);
}

#[test]
fn test_resource_setiv_reti() {
    let instrs = [
        Instruction::SetIV { trap_id: 0, target: "HANDLER".into() },
        Instruction::Reti,
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 2); // 1+1
    assert_eq!(t.total_space, 0); // 0+0
}

#[test]
fn test_resource_type_conversion() {
    let instrs = [
        Instruction::CvtIF { dst_f: 0, src_i: 0 },
        Instruction::CvtFI { dst_i: 0, src_f: 0 },
        Instruction::CvtFZ { dst_z: 0, src_f: 0 },
        Instruction::CvtZF { dst_f: 0, src_z: 0 },
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 4); // 1+1+1+1
    assert_eq!(t.total_space, 0); // conversions have no space cost
}

#[test]
fn test_resource_complex_arithmetic() {
    let instrs = [
        Instruction::ZAdd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ZSub { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ZMul { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ZDiv { dst: 0, lhs: 1, rhs: 2 },
    ];
    let t = accumulate_resources(&instrs);
    assert_eq!(t.total_time, 12); // 2+2+4+4
    assert_eq!(t.total_space, 4); // 1+1+1+1
}

#[test]
fn test_resource_indirect_memory() {
    let instrs = [
        Instruction::ILdx { dst: 0, addr_reg: 1 },
        Instruction::IStrx { src: 0, addr_reg: 1 },
        Instruction::FLdx { dst: 0, addr_reg: 1 },
        Instruction::FStrx { src: 0, addr_reg: 1 },
        Instruction::ZLdx { dst: 0, addr_reg: 1 },
        Instruction::ZStrx { src: 0, addr_reg: 1 },
    ];
    let t = accumulate_resources(&instrs);
    // ILdx(1) + IStrx(1) + FLdx(1) + FStrx(1) + ZLdx(2) + ZStrx(2) = 8
    assert_eq!(t.total_time, 8);
    // All have space=1
    assert_eq!(t.total_space, 6);
}
