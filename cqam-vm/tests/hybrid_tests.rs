// cqam-vm/tests/hybrid_tests.rs
//
// Phase 4: Test hybrid operations with Result-based error handling.

use cqam_core::instruction::*;
use cqam_core::register::HybridValue;
use cqam_vm::context::ExecutionContext;
use cqam_vm::hybrid::execute_hybrid;
use cqam_vm::executor::execute_instruction;

#[test]
fn test_hfork_sets_flags() {
    let mut ctx = ExecutionContext::new(vec![]);
    let jumped = execute_hybrid(&mut ctx, &Instruction::HFork).unwrap();
    assert!(!jumped);
    assert!(ctx.psw.hf);
    assert!(ctx.psw.forked);
}

#[test]
fn test_hmerge_sets_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    let jumped = execute_hybrid(&mut ctx, &Instruction::HMerge).unwrap();
    assert!(!jumped);
    assert!(ctx.psw.merged);
}

#[test]
fn test_hcexec_jump_on_qf() {
    let program = vec![
        Instruction::Label("THEN".into()),
        Instruction::ILdi { dst: 0, imm: 42 },
    ];
    let mut ctx = ExecutionContext::new(program);
    ctx.psw.qf = true;
    ctx.pc = 1;

    let jumped = execute_hybrid(
        &mut ctx,
        &Instruction::HCExec { flag: flag_id::QF, target: "THEN".into() },
    ).unwrap();

    assert!(jumped);
    assert_eq!(ctx.pc, 0);
}

#[test]
fn test_hcexec_no_jump_on_false_flag() {
    let program = vec![
        Instruction::Label("THEN".into()),
        Instruction::ILdi { dst: 0, imm: 42 },
    ];
    let mut ctx = ExecutionContext::new(program);
    ctx.psw.qf = false;
    ctx.pc = 1;

    let jumped = execute_hybrid(
        &mut ctx,
        &Instruction::HCExec { flag: flag_id::QF, target: "THEN".into() },
    ).unwrap();

    assert!(!jumped);
    assert_eq!(ctx.pc, 1);
}

#[test]
fn test_hreduce_round() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(2.7));

    let jumped = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ROUND },
    ).unwrap();

    assert!(!jumped);
    assert_eq!(ctx.iregs.get(1), 3);
}

#[test]
fn test_hreduce_floor() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(2.7));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::FLOOR },
    ).unwrap();

    assert_eq!(ctx.iregs.get(1), 2);
}

#[test]
fn test_hreduce_ceil() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(2.1));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::CEIL },
    ).unwrap();

    assert_eq!(ctx.iregs.get(1), 3);
}

#[test]
fn test_hreduce_trunc() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(2.9));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::TRUNC },
    ).unwrap();

    assert_eq!(ctx.iregs.get(1), 2);
}

#[test]
fn test_hreduce_abs() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(-5.3));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ABS },
    ).unwrap();

    assert_eq!(ctx.iregs.get(1), 5);
}

#[test]
fn test_hreduce_negate() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(3.0));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::NEGATE },
    ).unwrap();

    assert_eq!(ctx.iregs.get(1), -3);
}

#[test]
fn test_hreduce_magnitude() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Complex(3.0, 4.0));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MAGNITUDE },
    ).unwrap();

    assert!((ctx.fregs.get(0) - 5.0).abs() < 1e-10);
}

#[test]
fn test_hreduce_real_imag() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Complex(3.125, 2.625));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::REAL },
    ).unwrap();
    assert!((ctx.fregs.get(0) - 3.125).abs() < 1e-10);

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::IMAG },
    ).unwrap();
    assert!((ctx.fregs.get(1) - 2.625).abs() < 1e-10);
}

#[test]
fn test_hreduce_mean_of_distribution() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)];
    ctx.hregs.set(0, HybridValue::Dist(dist));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MEAN },
    ).unwrap();

    assert!((ctx.fregs.get(0) - 1.5).abs() < 1e-10);
}

#[test]
fn test_hreduce_mode_of_distribution() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.1), (1, 0.7), (2, 0.2)];
    ctx.hregs.set(0, HybridValue::Dist(dist));

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MODE },
    ).unwrap();

    assert_eq!(ctx.iregs.get(0), 1);
}

#[test]
fn test_hfork_merge_flow_simulation() {
    let program = vec![
        Instruction::HFork,
        Instruction::ILdi { dst: 0, imm: 5 },
        Instruction::HMerge,
    ];

    let mut ctx = ExecutionContext::new(program.clone());

    for instr in &program {
        match instr {
            Instruction::HFork | Instruction::HMerge | Instruction::HCExec { .. }
            | Instruction::HReduce { .. } => {
                execute_hybrid(&mut ctx, instr).unwrap();
                ctx.advance_pc();
            }
            _ => {
                execute_instruction(&mut ctx, instr).unwrap();
            }
        }
    }

    assert!(ctx.psw.forked);
    assert!(ctx.psw.merged);
    assert_eq!(ctx.iregs.get(0), 5);
}

// ===========================================================================
// Error cases (Phase 4)
// ===========================================================================

#[test]
fn test_hreduce_type_mismatch_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    // ROUND expects Float, but we have Int
    ctx.hregs.set(0, HybridValue::Int(42));

    let result = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ROUND },
    );
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Type mismatch"));
}

#[test]
fn test_hreduce_unknown_function_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(1.0));

    let result = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: 99 },
    );
    assert!(result.is_err());
}
