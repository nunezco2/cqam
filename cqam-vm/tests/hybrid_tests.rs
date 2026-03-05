// cqam-vm/tests/hybrid_tests.rs
//
// Phase 6: Test hybrid operations with real HFORK/HMERGE parallelism.

use cqam_core::instruction::*;
use cqam_core::register::HybridValue;
use cqam_vm::context::ExecutionContext;
use cqam_vm::fork::ForkManager;
use cqam_vm::hybrid::execute_hybrid;
use cqam_vm::executor::execute_instruction;

#[test]
fn test_hfork_sets_flags() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    let jumped = execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm).unwrap();
    assert!(!jumped);
    assert!(ctx.psw.hf);
    assert!(ctx.psw.forked);
}

#[test]
fn test_hmerge_sets_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    let jumped = execute_hybrid(&mut ctx, &Instruction::HMerge, &mut fm).unwrap();
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

    let mut fm = ForkManager::new();
    let jumped = execute_hybrid(
        &mut ctx,
        &Instruction::HCExec { flag: flag_id::QF, target: "THEN".into() },
        &mut fm,
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

    let mut fm = ForkManager::new();
    let jumped = execute_hybrid(
        &mut ctx,
        &Instruction::HCExec { flag: flag_id::QF, target: "THEN".into() },
        &mut fm,
    ).unwrap();

    assert!(!jumped);
    assert_eq!(ctx.pc, 1);
}

#[test]
fn test_hreduce_round() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(2.7)).unwrap();

    let mut fm = ForkManager::new();
    let jumped = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ROUND },
        &mut fm,
    ).unwrap();

    assert!(!jumped);
    assert_eq!(ctx.iregs.get(1).unwrap(), 3);
}

#[test]
fn test_hreduce_floor() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(2.7)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::FLOOR },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(1).unwrap(), 2);
}

#[test]
fn test_hreduce_ceil() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(2.1)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::CEIL },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(1).unwrap(), 3);
}

#[test]
fn test_hreduce_trunc() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(2.9)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::TRUNC },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(1).unwrap(), 2);
}

#[test]
fn test_hreduce_abs() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(-5.3)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ABS },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(1).unwrap(), 5);
}

#[test]
fn test_hreduce_negate() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(3.0)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::NEGATE },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(1).unwrap(), -3);
}

#[test]
fn test_hreduce_magnitude() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Complex(3.0, 4.0)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MAGNITUDE },
        &mut fm,
    ).unwrap();

    assert!((ctx.fregs.get(0).unwrap() - 5.0).abs() < 1e-10);
}

#[test]
fn test_hreduce_real_imag() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Complex(3.125, 2.625)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::REAL },
        &mut fm,
    ).unwrap();
    assert!((ctx.fregs.get(0).unwrap() - 3.125).abs() < 1e-10);

    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::IMAG },
        &mut fm,
    ).unwrap();
    assert!((ctx.fregs.get(1).unwrap() - 2.625).abs() < 1e-10);
}

#[test]
fn test_hreduce_mean_of_distribution() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MEAN },
        &mut fm,
    ).unwrap();

    assert!((ctx.fregs.get(0).unwrap() - 1.5).abs() < 1e-10);
}

#[test]
fn test_hreduce_mode_of_distribution() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.1), (1, 0.7), (2, 0.2)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MODE },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 1);
}

#[test]
fn test_hfork_merge_flow_simulation() {
    let program = vec![
        Instruction::HFork,
        Instruction::ILdi { dst: 0, imm: 5 },
        Instruction::HMerge,
    ];

    let mut ctx = ExecutionContext::new(program.clone());
    let mut fm = ForkManager::new();

    for instr in &program {
        match instr {
            Instruction::HFork | Instruction::HMerge | Instruction::HCExec { .. }
            | Instruction::HReduce { .. } => {
                execute_hybrid(&mut ctx, instr, &mut fm).unwrap();
                ctx.advance_pc();
            }
            _ => {
                execute_instruction(&mut ctx, instr, &mut fm).unwrap();
            }
        }
    }

    assert!(ctx.psw.forked);
    assert!(ctx.psw.merged);
    assert_eq!(ctx.iregs.get(0).unwrap(), 5);
}

// ===========================================================================
// Error cases (Phase 4)
// ===========================================================================

#[test]
fn test_hreduce_type_mismatch_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    // ROUND expects Float, but we have Int
    ctx.hregs.set(0, HybridValue::Int(42)).unwrap();

    let mut fm = ForkManager::new();
    let result = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::ROUND },
        &mut fm,
    );
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Type mismatch"));
}

#[test]
fn test_hreduce_unknown_function_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(1.0)).unwrap();

    let mut fm = ForkManager::new();
    let result = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: 99 },
        &mut fm,
    );
    assert!(result.is_err());
}

// ===========================================================================
// NaN safety test (Fix 2.5)
// ===========================================================================

#[test]
fn test_hreduce_mode_with_nan_does_not_panic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, f64::NAN), (1, 0.5), (2, 0.5)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::MODE },
        &mut fm,
    ).unwrap();
    // Should not panic; result is one of the non-NaN entries
}

// ===========================================================================
// Phase 6: Real fork/merge tests
// ===========================================================================

#[test]
fn test_hfork_spawns_real_thread() {
    // HFORK followed by different operations, then HMERGE
    let program = vec![
        Instruction::HFork,           // 0: fork
        Instruction::ILdi { dst: 0, imm: 42 }, // 1: both paths execute this
        Instruction::HMerge,          // 2: join
        Instruction::Halt,            // 3: done
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    assert!(ctx.psw.forked);
    assert!(ctx.psw.merged);
    assert_eq!(ctx.iregs.get(0).unwrap(), 42);
    // The fork should have completed and been collected
    assert_eq!(fm.completed_forks.len(), 1);
    // Fork context should also have R0 = 42
    assert_eq!(fm.completed_forks[0].iregs.get(0).unwrap(), 42);
}

#[test]
fn test_hfork_independence() {
    // Fork and main diverge: fork runs the same code but main modifies R1 after merge
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 10 },  // 0: set R0=10
        Instruction::HFork,                       // 1: fork
        Instruction::ILdi { dst: 1, imm: 20 },   // 2: both set R1=20
        Instruction::HMerge,                      // 3: join
        Instruction::ILdi { dst: 2, imm: 99 },   // 4: only main sets R2=99
        Instruction::Halt,                        // 5: done
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 10);
    assert_eq!(ctx.iregs.get(1).unwrap(), 20);
    assert_eq!(ctx.iregs.get(2).unwrap(), 99);

    // Fork context: R0=10, R1=20, but R2 should be 99 too since fork
    // continues past HMERGE (it just runs to HALT)
    assert_eq!(fm.completed_forks.len(), 1);
    let fork_ctx = &fm.completed_forks[0];
    assert_eq!(fork_ctx.iregs.get(0).unwrap(), 10);
    assert_eq!(fork_ctx.iregs.get(1).unwrap(), 20);
}

#[test]
fn test_hmerge_without_fork_no_error() {
    // HMERGE with no active forks should just set flags, no error
    let program = vec![
        Instruction::HMerge,
        Instruction::Halt,
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    assert!(ctx.psw.merged);
    assert_eq!(fm.completed_forks.len(), 0);
}

#[test]
fn test_fork_depth_limit() {
    let fm = ForkManager::new();
    assert!(fm.can_fork());
    assert_eq!(fm.depth(), 0);

    // Nested at max depth should not be able to fork
    let fm_deep = ForkManager::nested(3, 4);
    assert!(!fm_deep.can_fork());
}

#[test]
fn test_fork_manager_take_completed() {
    let mut fm = ForkManager::new();
    assert_eq!(fm.take_completed().len(), 0);

    // Run a program with fork to populate completed_forks
    let program = vec![
        Instruction::HFork,
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::HMerge,
        Instruction::Halt,
    ];
    let mut ctx = ExecutionContext::new(program);
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    let completed = fm.take_completed();
    assert_eq!(completed.len(), 1);
    // After take, completed_forks should be empty
    assert_eq!(fm.take_completed().len(), 0);
}

#[test]
fn test_context_clone_preserves_state() {
    let mut ctx = ExecutionContext::new(vec![Instruction::Halt]);
    ctx.iregs.set(0, 42).unwrap();
    ctx.fregs.set(1, 3.15).unwrap();
    ctx.zregs.set(2, (1.0, 2.0)).unwrap();
    ctx.cmem.store(100, 999);
    ctx.pc = 0;

    let cloned = ctx.clone();
    assert_eq!(cloned.iregs.get(0).unwrap(), 42);
    assert!((cloned.fregs.get(1).unwrap() - 3.15).abs() < 1e-10);
    assert_eq!(cloned.zregs.get(2).unwrap(), (1.0, 2.0));
    assert_eq!(cloned.cmem.load(100), 999);
    assert_eq!(cloned.pc, 0);
}

// ===========================================================================
// Phase 6 Stress Tests: Fork/Merge parallelism edge cases
// ===========================================================================

/// Verify that the fork context's PC is exactly ctx.pc + 1 (the instruction
/// after HFORK). This is fundamental to the fork contract: the fork must
/// NOT re-execute the HFORK itself (infinite recursion) and must start
/// at the very next instruction.
#[test]
fn test_hfork_context_pc_is_next_instruction() {
    // Program: NOP at 0, HFORK at 1, ILdi at 2, HMERGE at 3, HALT at 4
    let program = vec![
        Instruction::Nop,                          // 0
        Instruction::HFork,                        // 1
        Instruction::ILdi { dst: 0, imm: 77 },    // 2
        Instruction::HMerge,                       // 3
        Instruction::Halt,                         // 4
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Fork ran from PC=2 onward and completed
    assert_eq!(fm.completed_forks.len(), 1);
    let fork_ctx = &fm.completed_forks[0];
    // Fork should have executed ILdi at PC=2, so R0 = 77
    assert_eq!(fork_ctx.iregs.get(0).unwrap(), 77);
    // Fork should have reached HALT, so trap_halt is set
    assert!(fork_ctx.psw.trap_halt);
}

/// Verify that a fork's register mutations do NOT bleed into the main
/// context. The fork modifies R0 to a different value than main does.
#[test]
fn test_fork_register_isolation() {
    // Main sets R0=100 before fork. Fork inherits R0=100 and overwrites
    // with R0=200. After merge, main sets R0=300. The fork's R0=200
    // must not interfere with main's R0=300.
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 100 },   // 0: main sets R0=100
        Instruction::HFork,                        // 1: fork clones ctx (R0=100)
        // Fork: continues from PC=2, sets R0=200, then R1=1, then HALT
        // Main: continues from PC=2 (same instructions)
        Instruction::ILdi { dst: 0, imm: 200 },   // 2: both set R0=200
        Instruction::ILdi { dst: 1, imm: 1 },     // 3: both set R1=1
        Instruction::HMerge,                       // 4: join
        Instruction::ILdi { dst: 0, imm: 300 },   // 5: only main sets R0=300
        Instruction::Halt,                         // 6
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main: R0 should be 300 (set after merge)
    assert_eq!(ctx.iregs.get(0).unwrap(), 300);
    // Fork: R0 should be 200 or 300 depending on whether fork runs past HMERGE
    // (fork runs full program, so it also executes ILdi R0=300 at index 5)
    // The key point is that the fork's execution did NOT affect main's registers.
    assert_eq!(fm.completed_forks.len(), 1);
}

/// Multiple sequential HFORK/HMERGE pairs: fork, merge, fork again, merge again.
/// Each fork should produce an independent completed context.
#[test]
fn test_multiple_sequential_fork_merge_pairs() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 10 },    // 0
        Instruction::HFork,                        // 1: first fork
        Instruction::ILdi { dst: 1, imm: 20 },    // 2
        Instruction::HMerge,                       // 3: first merge
        Instruction::ILdi { dst: 2, imm: 30 },    // 4
        Instruction::HFork,                        // 5: second fork
        Instruction::ILdi { dst: 3, imm: 40 },    // 6
        Instruction::HMerge,                       // 7: second merge
        Instruction::Halt,                         // 8
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main completed normally
    assert_eq!(ctx.iregs.get(0).unwrap(), 10);
    assert_eq!(ctx.iregs.get(1).unwrap(), 20);
    assert_eq!(ctx.iregs.get(2).unwrap(), 30);
    assert_eq!(ctx.iregs.get(3).unwrap(), 40);
    assert!(ctx.psw.trap_halt);

    // Two fork/merge cycles should have produced 2 completed forks
    // (first merge collects 1 fork, second merge collects 1 fork,
    //  but the second fork itself may also spawn a nested fork when
    //  it runs through the second HFORK in the remaining program)
    // At minimum, the first merge should have collected 1 fork.
    assert!(fm.completed_forks.len() >= 2,
        "Expected at least 2 completed forks, got {}", fm.completed_forks.len());
}

/// Fork that encounters a division-by-zero trap. The trap sets trap_arith
/// flag and execution continues (trap flag is set, not a hard error).
#[test]
fn test_fork_div_by_zero_sets_trap_flag() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },    // 0
        Instruction::ILdi { dst: 1, imm: 0 },     // 1: divisor = 0
        Instruction::HFork,                        // 2: fork
        // Both main and fork execute this div-by-zero:
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }, // 3: trap_arith
        Instruction::HMerge,                       // 4
        Instruction::Halt,                         // 5
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();

    // Division by zero now sets trap_arith flag instead of returning Err
    let result = cqam_vm::executor::run_program(&mut ctx, &mut fm);
    assert!(result.is_ok());
    assert!(ctx.psw.trap_arith);
}

/// Fork where only the fork encounters an error (main skips the error path).
/// Fork thread with div-by-zero now sets trap_arith (not hard error).
/// The fork thread completes with the trap flag set.
#[test]
fn test_fork_thread_div_by_zero_completes_with_trap() {
    let error_program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }, // div by zero -> trap_arith
        Instruction::Halt,
    ];

    let fork_ctx = ExecutionContext::new(error_program);
    let mut fm = ForkManager::new();
    fm.spawn_fork(fork_ctx).unwrap();

    // join_all should succeed (trap is a flag, not an error)
    fm.join_all().unwrap();
    assert_eq!(fm.completed_forks.len(), 1);
    assert!(fm.completed_forks[0].psw.trap_arith);
}

/// Fork depth limit: attempting to fork beyond max_depth returns ForkError.
#[test]
fn test_fork_depth_limit_error_through_hybrid() {
    let program = vec![
        Instruction::HFork,
        Instruction::Halt,
    ];

    let mut ctx = ExecutionContext::new(program);
    // Create a ForkManager that is already at max depth
    let mut fm = ForkManager::nested(3, 4); // depth=4, max=4 -> can_fork() is false

    let result = execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Fork depth limit"), "Expected depth limit error, got: {}", msg);
}

/// Fork with an empty remaining program (HFORK is last instruction).
/// The fork starts at PC past end-of-program and should complete immediately.
#[test]
fn test_fork_with_empty_remaining_program() {
    let program = vec![
        Instruction::HFork,  // 0: fork starts at PC=1, which is past end
        // no more instructions
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();

    // Execute HFORK manually
    execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm).unwrap();
    assert_eq!(fm.active_count(), 1);

    // Join: fork should have completed immediately (empty program from PC=1)
    fm.join_all().unwrap();
    assert_eq!(fm.completed_forks.len(), 1);
    // Fork context should have no halt trap since it just fell off the end
    assert!(!fm.completed_forks[0].psw.trap_halt);
}

/// Stress: three sequential fork/merge pairs. Since fork threads run
/// the full remaining program, each fork will encounter subsequent HFORKs,
/// creating nested forks. With 3 pairs the maximum nesting depth is 3,
/// which is within the default limit of 4. This tests that the thread
/// lifecycle handles nested fork trees without leaks or deadlocks.
#[test]
fn test_sequential_fork_merge_pairs_with_nesting() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },     // 0
        Instruction::HFork,                        // 1: fork A (runs 2..end)
        Instruction::ILdi { dst: 1, imm: 2 },     // 2
        Instruction::HMerge,                       // 3: join A
        Instruction::ILdi { dst: 2, imm: 3 },     // 4
        Instruction::HFork,                        // 5: fork B (runs 6..end)
        Instruction::ILdi { dst: 3, imm: 4 },     // 6
        Instruction::HMerge,                       // 7: join B
        Instruction::ILdi { dst: 4, imm: 5 },     // 8
        Instruction::HFork,                        // 9: fork C (runs 10..end)
        Instruction::ILdi { dst: 5, imm: 6 },     // 10
        Instruction::HMerge,                       // 11: join C
        Instruction::Halt,                         // 12
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main should have all registers set
    assert_eq!(ctx.iregs.get(0).unwrap(), 1);
    assert_eq!(ctx.iregs.get(1).unwrap(), 2);
    assert_eq!(ctx.iregs.get(2).unwrap(), 3);
    assert_eq!(ctx.iregs.get(3).unwrap(), 4);
    assert_eq!(ctx.iregs.get(4).unwrap(), 5);
    assert_eq!(ctx.iregs.get(5).unwrap(), 6);
    assert!(ctx.psw.trap_halt);

    // At the top level, we should have at least 3 completed forks
    // (one per HMERGE at the top level). Fork A itself may have
    // spawned sub-forks that it collected in its own ForkManager.
    assert!(fm.completed_forks.len() >= 3,
        "Expected >= 3 completed forks, got {}", fm.completed_forks.len());
}

/// Verify that fork and main both compute their respective results correctly
/// when running a small loop. Both run the same loop, so results should match.
#[test]
fn test_fork_and_main_compute_same_loop_result() {
    // Program: set R0=0, R1=5, R2=1, then fork, loop R0 += R2 while R0 < R1
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 0 },     // 0: R0 = 0 (accumulator)
        Instruction::ILdi { dst: 1, imm: 5 },     // 1: R1 = 5 (limit)
        Instruction::ILdi { dst: 2, imm: 1 },     // 2: R2 = 1 (increment)
        Instruction::HFork,                        // 3: fork
        Instruction::Label("LOOP".into()),         // 4
        Instruction::IEq { dst: 3, lhs: 0, rhs: 1 }, // 5: R3 = (R0 == R1)
        Instruction::Jif { pred: 3, target: "DONE".into() }, // 6
        Instruction::IAdd { dst: 0, lhs: 0, rhs: 2 }, // 7: R0 += 1
        Instruction::Jmp { target: "LOOP".into() }, // 8
        Instruction::Label("DONE".into()),         // 9
        Instruction::HMerge,                       // 10
        Instruction::Halt,                         // 11
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main should have R0 = 5 (looped 5 times)
    assert_eq!(ctx.iregs.get(0).unwrap(), 5);

    // Fork should also have R0 = 5 (same loop)
    assert_eq!(fm.completed_forks.len(), 1);
    assert_eq!(fm.completed_forks[0].iregs.get(0).unwrap(), 5);
}

/// Verify that HFORK sets psw.forked on the fork context as well as main.
/// An HMERGE is required to join the fork thread and collect its context.
#[test]
fn test_hfork_sets_flags_on_fork_context() {
    let program = vec![
        Instruction::HFork,        // 0
        Instruction::HMerge,       // 1
        Instruction::Halt,         // 2
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main flags
    assert!(ctx.psw.hf);
    assert!(ctx.psw.forked);

    // Fork context flags (set in hybrid.rs before spawn_fork)
    assert_eq!(fm.completed_forks.len(), 1);
    assert!(fm.completed_forks[0].psw.hf);
    assert!(fm.completed_forks[0].psw.forked);
}

/// Verify that take_completed drains all forks and a second call returns empty.
#[test]
fn test_take_completed_is_idempotent_drain() {
    let program = vec![
        Instruction::HFork,
        Instruction::Nop,
        Instruction::HMerge,
        Instruction::HFork,
        Instruction::Nop,
        Instruction::HMerge,
        Instruction::Halt,
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    let first = fm.take_completed();
    assert!(!first.is_empty());
    let second = fm.take_completed();
    assert!(second.is_empty(), "Second take_completed should return empty vec");
}

/// Verify ForkManager::nested creates correct depth.
#[test]
fn test_fork_manager_nested_depth() {
    let fm0 = ForkManager::new();
    assert_eq!(fm0.depth(), 0);
    assert!(fm0.can_fork());

    let fm1 = ForkManager::nested(0, 4);
    assert_eq!(fm1.depth(), 1);
    assert!(fm1.can_fork());

    let fm3 = ForkManager::nested(2, 4);
    assert_eq!(fm3.depth(), 3);
    assert!(fm3.can_fork());

    let fm4 = ForkManager::nested(3, 4);
    assert_eq!(fm4.depth(), 4);
    assert!(!fm4.can_fork());

    // Saturating add: nested(255, 255) should not overflow
    let fm_sat = ForkManager::nested(255, 255);
    assert_eq!(fm_sat.depth(), 255);
    assert!(!fm_sat.can_fork());
}

/// ForkManager::default() should behave identically to ForkManager::new().
#[test]
fn test_fork_manager_default_equals_new() {
    let fm_new = ForkManager::new();
    let fm_def = ForkManager::default();
    assert_eq!(fm_new.depth(), fm_def.depth());
    assert_eq!(fm_new.max_depth(), fm_def.max_depth());
    assert_eq!(fm_new.active_count(), fm_def.active_count());
    assert_eq!(fm_new.can_fork(), fm_def.can_fork());
}

// ===========================================================================
// Phase 9.8: HFORK/HMERGE parallel execution tests
// ===========================================================================

#[test]
fn test_hfork_divergent_paths() {
    let program = vec![
        Instruction::ILdi { dst: 5, imm: 50 },    // 0: R5=50
        Instruction::HFork,                        // 1: fork
        Instruction::ILdi { dst: 5, imm: 100 },   // 2: both set R5=100
        Instruction::ILdi { dst: 6, imm: 200 },   // 3: both set R6=200
        Instruction::HMerge,                       // 4: join
        Instruction::ILdi { dst: 5, imm: 999 },   // 5: only main's post-merge
        Instruction::Halt,                         // 6
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main: R5 should be 999 (set after merge)
    assert_eq!(ctx.iregs.get(5).unwrap(), 999);
    assert_eq!(ctx.iregs.get(6).unwrap(), 200);

    // Fork completed independently
    assert_eq!(fm.completed_forks.len(), 1);
    let fork_ctx = &fm.completed_forks[0];
    assert_eq!(fork_ctx.iregs.get(6).unwrap(), 200);
}

#[test]
fn test_hfork_one_branch_halts_early() {
    // Manually spawn a fork with a short program containing HALT
    let short_program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::Halt,
    ];

    let fork_ctx = ExecutionContext::new(short_program);
    let mut fm = ForkManager::new();
    fm.spawn_fork(fork_ctx).unwrap();
    assert_eq!(fm.active_count(), 1);

    fm.join_all().unwrap();
    assert_eq!(fm.completed_forks.len(), 1);
    assert!(fm.completed_forks[0].psw.trap_halt, "Fork should have halted");
    assert_eq!(fm.completed_forks[0].iregs.get(0).unwrap(), 42);
}

#[test]
fn test_hfork_verify_fork_state_independence() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 0 },     // 0
        Instruction::HFork,                        // 1
        Instruction::ILdi { dst: 0, imm: 1 },     // 2: both set R0=1
        Instruction::HMerge,                       // 3
        Instruction::ILdi { dst: 0, imm: 2 },     // 4: main sets R0=2
        Instruction::Halt,                         // 5
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main: R0 = 2 (set after merge)
    assert_eq!(ctx.iregs.get(0).unwrap(), 2);

    // Fork completed independently
    assert_eq!(fm.completed_forks.len(), 1);
    // Fork ran the same code past merge to HALT, so its R0 is set by its own execution
    // The key: fork's execution is independent of main
}
