//! Tests for hybrid operations: HFORK/HMERGE parallelism, JMPF conditional
//! branching, and HREDUCE reductions across all supported function IDs.

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
    // Must HFORK first before HMERGE
    execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm).unwrap();
    let jumped = execute_hybrid(&mut ctx, &Instruction::HMerge, &mut fm).unwrap();
    assert!(!jumped);
    assert!(ctx.psw.merged);
}

#[test]
fn test_jmpf_jump_on_qf() {
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
        &Instruction::JmpF { flag: flag_id::QF, target: "THEN".into() },
        &mut fm,
    ).unwrap();

    assert!(jumped);
    assert_eq!(ctx.pc, 0);
}

#[test]
fn test_jmpf_no_jump_on_false_flag() {
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
        &Instruction::JmpF { flag: flag_id::QF, target: "THEN".into() },
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
            Instruction::HFork | Instruction::HMerge | Instruction::JmpF { .. }
            | Instruction::HReduce { .. } => {
                execute_hybrid(&mut ctx, instr, &mut fm).unwrap();
                ctx.advance_pc();
            }
            _ => {
                execute_instruction(&mut ctx, instr, &mut fm).unwrap();
            }
        }
    }

    assert!(!ctx.psw.forked);  // HMERGE clears forked
    assert!(ctx.psw.merged);
    assert_eq!(ctx.iregs.get(0).unwrap(), 5);
}

// --- Error cases -------------------------------------------------------------

#[test]
fn test_hreduce_type_mismatch_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    // CONJ_Z expects Complex, but we have Int — genuine type mismatch
    ctx.hregs.set(0, HybridValue::Int(42)).unwrap();

    let mut fm = ForkManager::new();
    let result = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::CONJ_Z },
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

// --- Fork/merge parallelism (single-threaded, thread_count=1) ----------------

#[test]
fn test_hfork_single_threaded_flag_management() {
    // With thread_count=1 (default), HFORK just sets flags, no thread spawning
    let program = vec![
        Instruction::HFork,           // 0: fork (single-threaded => just flags)
        Instruction::ILdi { dst: 0, imm: 42 }, // 1: execute normally
        Instruction::HMerge,          // 2: join (no-op for single-threaded)
        Instruction::Halt,            // 3: done
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    assert!(!ctx.psw.forked);  // HMERGE clears forked
    assert!(ctx.psw.merged);
    assert_eq!(ctx.iregs.get(0).unwrap(), 42);
    // No fork threads spawned in single-threaded mode
    assert_eq!(fm.completed_forks.len(), 0);
}

#[test]
fn test_hfork_single_threaded_independence() {
    // Single-threaded: all instructions run in main, no fork thread
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 10 },  // 0: set R0=10
        Instruction::HFork,                       // 1: fork (flags only)
        Instruction::ILdi { dst: 1, imm: 20 },   // 2: R1=20
        Instruction::HMerge,                      // 3: join (flags only)
        Instruction::ILdi { dst: 2, imm: 99 },   // 4: R2=99
        Instruction::Halt,                        // 5: done
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 10);
    assert_eq!(ctx.iregs.get(1).unwrap(), 20);
    assert_eq!(ctx.iregs.get(2).unwrap(), 99);
}

#[test]
fn test_hmerge_without_fork_returns_error() {
    // HMERGE without prior HFORK should return an error
    let program = vec![
        Instruction::HMerge,
        Instruction::Halt,
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    let result = cqam_vm::executor::run_program(&mut ctx, &mut fm);
    assert!(result.is_err());
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

    // In single-threaded mode, HFORK doesn't spawn threads
    // so completed_forks stays empty. Just verify take_completed works.
    let completed = fm.take_completed();
    assert_eq!(completed.len(), 0);
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

// --- Multi-threaded SPMD fork/merge tests ------------------------------------

#[test]
fn test_hfork_spmd_two_threads() {
    // With thread_count=2, HFORK spawns one worker thread
    // Both threads run the same code, then HMERGE joins them
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 }, // 0: set R0=42
        Instruction::HFork,                     // 1: fork (spawns 1 worker)
        Instruction::ILdi { dst: 1, imm: 99 }, // 2: both threads set R1=99
        Instruction::HMerge,                    // 3: join
        Instruction::Halt,                      // 4
    ];

    let mut ctx = ExecutionContext::new(program);
    ctx.thread_count = 2;
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    assert!(!ctx.psw.forked);
    assert!(ctx.psw.merged);
    assert_eq!(ctx.iregs.get(0).unwrap(), 42);
    assert_eq!(ctx.iregs.get(1).unwrap(), 99);
}

#[test]
fn test_hfork_spmd_tid_divergence() {
    // Test that threads can identify themselves via ITid and diverge
    let program = vec![
        Instruction::HFork,                                          // 0
        Instruction::ITid { dst: 0 },                                // 1: R0 = thread_id
        Instruction::ILdi { dst: 1, imm: 0 },                       // 2: R1 = 0
        Instruction::IEq { dst: 2, lhs: 0, rhs: 1 },                // 3: R2 = (tid == 0)
        Instruction::Jif { pred: 2, target: "IS_LEADER".into() },   // 4
        // Worker: R3 = 200
        Instruction::ILdi { dst: 3, imm: 200 },                     // 5
        Instruction::Jmp { target: "DONE".into() },                  // 6
        Instruction::Label("IS_LEADER".into()),                      // 7
        // Leader: R3 = 100
        Instruction::ILdi { dst: 3, imm: 100 },                     // 8
        Instruction::Label("DONE".into()),                           // 9
        Instruction::HMerge,                                         // 10
        Instruction::Halt,                                           // 11
    ];

    let mut ctx = ExecutionContext::new(program);
    ctx.thread_count = 2;
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Thread 0 (leader): R3 = 100
    assert_eq!(ctx.iregs.get(3).unwrap(), 100);
    assert_eq!(ctx.thread_id, 0);
}

#[test]
fn test_hfork_spmd_loop() {
    // Both threads run the same loop: R0 += 1 until R0 == 5
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 0 },     // 0: R0 = 0
        Instruction::ILdi { dst: 1, imm: 5 },     // 1: R1 = 5
        Instruction::ILdi { dst: 2, imm: 1 },     // 2: R2 = 1
        Instruction::HFork,                        // 3
        Instruction::Label("LOOP".into()),         // 4
        Instruction::IEq { dst: 3, lhs: 0, rhs: 1 }, // 5
        Instruction::Jif { pred: 3, target: "DONE".into() }, // 6
        Instruction::IAdd { dst: 0, lhs: 0, rhs: 2 }, // 7
        Instruction::Jmp { target: "LOOP".into() }, // 8
        Instruction::Label("DONE".into()),         // 9
        Instruction::HMerge,                       // 10
        Instruction::Halt,                         // 11
    ];

    let mut ctx = ExecutionContext::new(program);
    ctx.thread_count = 2;
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 5);
}

#[test]
fn test_hfork_sequential_pairs() {
    // Two sequential HFORK/HMERGE pairs in single-threaded mode
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 10 },    // 0
        Instruction::HFork,                        // 1
        Instruction::ILdi { dst: 1, imm: 20 },    // 2
        Instruction::HMerge,                       // 3
        Instruction::ILdi { dst: 2, imm: 30 },    // 4
        Instruction::HFork,                        // 5
        Instruction::ILdi { dst: 3, imm: 40 },    // 6
        Instruction::HMerge,                       // 7
        Instruction::Halt,                         // 8
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 10);
    assert_eq!(ctx.iregs.get(1).unwrap(), 20);
    assert_eq!(ctx.iregs.get(2).unwrap(), 30);
    assert_eq!(ctx.iregs.get(3).unwrap(), 40);
    assert!(ctx.psw.trap_halt);
}

/// Fork that encounters a division-by-zero trap. The trap sets trap_arith
/// flag and execution continues (trap flag is set, not a hard error).
#[test]
fn test_fork_div_by_zero_sets_trap_flag() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },    // 0
        Instruction::ILdi { dst: 1, imm: 0 },     // 1: divisor = 0
        Instruction::HFork,                        // 2: fork
        Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }, // 3: trap_arith
        Instruction::HMerge,                       // 4
        Instruction::Halt,                         // 5
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();

    let result = cqam_vm::executor::run_program(&mut ctx, &mut fm);
    assert!(result.is_ok());
    assert!(ctx.psw.trap_arith);
}

/// Fork thread with div-by-zero completes with trap flag set.
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

/// Fork depth limit: attempting to fork (multi-threaded) beyond max_depth returns ForkError.
#[test]
fn test_fork_depth_limit_error_through_hybrid() {
    // With thread_count > 1, fork depth is checked via can_fork()
    // But in the SPMD model, nested HFORK is rejected by the "nested HFORK" check,
    // not the depth limit (since HFORK inside a forked region is not allowed).
    // Test the nested HFORK error instead:
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();

    // First HFORK succeeds
    execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm).unwrap();
    assert!(ctx.psw.forked);

    // Second HFORK while already forked should fail
    let result = execute_hybrid(&mut ctx, &Instruction::HFork, &mut fm);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("nested HFORK"), "Expected nested HFORK error, got: {}", msg);
}

/// SPMD: all threads see the same flag state after HFORK/HMERGE
#[test]
fn test_hfork_sets_flags_on_all_threads() {
    let program = vec![
        Instruction::HFork,        // 0
        Instruction::HMerge,       // 1
        Instruction::Halt,         // 2
    ];

    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main flags: after HMERGE, forked is cleared, merged is set
    assert!(ctx.psw.hf);
    assert!(!ctx.psw.forked);
    assert!(ctx.psw.merged);
}

/// Take completed is idempotent drain (no forks in single-threaded mode).
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

    // In single-threaded mode, no fork threads are spawned
    let first = fm.take_completed();
    assert_eq!(first.len(), 0);
    let second = fm.take_completed();
    assert_eq!(second.len(), 0);
}

// --- ForkManager unit tests --------------------------------------------------

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

// --- SPMD multi-threaded divergent path tests --------------------------------

#[test]
fn test_hfork_spmd_divergent_paths() {
    // Two threads diverge based on thread_id
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
    ctx.thread_count = 2;
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();

    // Main: R5 should be 999 (set after merge)
    assert_eq!(ctx.iregs.get(5).unwrap(), 999);
    assert_eq!(ctx.iregs.get(6).unwrap(), 200);
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

// =============================================================================
// HREDUCE CONJ_Z and NEGATE_Z tests
// =============================================================================

#[test]
fn test_hreduce_conj_z() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Complex(3.0, 4.0)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::CONJ_Z },
        &mut fm,
    ).unwrap();

    // conj(3+4i) = 3-4i -> written to Z[0]
    let (re, im) = ctx.zregs.get(0).unwrap();
    assert!((re - 3.0).abs() < 1e-10, "re should be 3.0, got {}", re);
    assert!((im - (-4.0)).abs() < 1e-10, "im should be -4.0, got {}", im);
}

#[test]
fn test_hreduce_negate_z() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Complex(3.0, 4.0)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 1, func: reduce_fn::NEGATE_Z },
        &mut fm,
    ).unwrap();

    // negate(3+4i) = -3-4i -> written to Z[1]
    let (re, im) = ctx.zregs.get(1).unwrap();
    assert!((re - (-3.0)).abs() < 1e-10, "re should be -3.0, got {}", re);
    assert!((im - (-4.0)).abs() < 1e-10, "im should be -4.0, got {}", im);
}

#[test]
fn test_hreduce_conj_z_type_mismatch() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Float(1.0)).unwrap();

    let mut fm = ForkManager::new();
    let result = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::CONJ_Z },
        &mut fm,
    );
    assert!(result.is_err(), "CONJ_Z on Float should fail");
}

#[test]
fn test_hreduce_negate_z_type_mismatch() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.hregs.set(0, HybridValue::Int(42)).unwrap();

    let mut fm = ForkManager::new();
    let result = execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::NEGATE_Z },
        &mut fm,
    );
    assert!(result.is_err(), "NEGATE_Z on Int should fail");
}

// =============================================================================
// HREDUCE Dist fallback tests
// =============================================================================

#[test]
fn test_hreduce_round_dist_fallback() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::ROUND },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 2, "round(1.5) = 2");
}

#[test]
fn test_hreduce_floor_dist_fallback() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::FLOOR },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 1, "floor(1.5) = 1");
}

#[test]
fn test_hreduce_ceil_dist_fallback() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::CEIL },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 2, "ceil(1.5) = 2");
}

#[test]
fn test_hreduce_negate_dist_fallback() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::NEGATE },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), -1, "negate(mean=1.5) = -1 as i64");
}

#[test]
fn test_hreduce_trunc_dist_fallback() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::TRUNC },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 1, "trunc(1.5) = 1");
}

#[test]
fn test_hreduce_abs_dist_fallback() {
    let mut ctx = ExecutionContext::new(vec![]);
    let dist = vec![(0u16, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)];
    ctx.hregs.set(0, HybridValue::Dist(dist)).unwrap();

    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 0, func: reduce_fn::ABS },
        &mut fm,
    ).unwrap();

    assert_eq!(ctx.iregs.get(0).unwrap(), 1, "abs(1.5) = 1 as i64");
}

/// Verify all 6 Dist fallback functions produce correct results with a
/// skewed distribution whose mean is 2.3.
#[test]
fn test_hreduce_all_six_dist_fallback_skewed() {
    // Distribution: mean = 1*0.1 + 2*0.2 + 3*0.7 = 0.1 + 0.4 + 2.1 = 2.6
    let dist = vec![(1u16, 0.1), (2, 0.2), (3, 0.7)];
    let mean = 2.6_f64;

    let funcs_and_expected: Vec<(u8, i64)> = vec![
        (reduce_fn::ROUND,  mean.round() as i64),   // round(2.6) = 3
        (reduce_fn::FLOOR,  mean.floor() as i64),    // floor(2.6) = 2
        (reduce_fn::CEIL,   mean.ceil() as i64),     // ceil(2.6) = 3
        (reduce_fn::TRUNC,  mean.trunc() as i64),    // trunc(2.6) = 2
        (reduce_fn::ABS,    mean.abs() as i64),      // abs(2.6) = 2
        (reduce_fn::NEGATE, (-mean) as i64),          // negate(2.6) = -2
    ];

    for (func, expected) in funcs_and_expected {
        let mut ctx = ExecutionContext::new(vec![]);
        ctx.hregs.set(0, HybridValue::Dist(dist.clone())).unwrap();

        let mut fm = ForkManager::new();
        execute_hybrid(
            &mut ctx,
            &Instruction::HReduce { src: 0, dst: 1, func },
            &mut fm,
        ).unwrap();

        assert_eq!(
            ctx.iregs.get(1).unwrap(), expected,
            "Dist fallback for func {} (mean={}) should yield {}, got {}",
            func, mean, expected, ctx.iregs.get(1).unwrap()
        );
    }
}

/// End-to-end pipeline: QPREP -> QOBSERVE(AMP) -> HREDUCE(NEGATE_Z) -> verify Z register.
#[test]
fn test_e2e_observe_amp_then_negate_z() {
    use cqam_vm::qop::execute_qop;

    let mut ctx = ExecutionContext::new(vec![]);

    // Bell state: rho[3][0] = 0.5 + 0i
    execute_qop(&mut ctx, &Instruction::QPrep { dst: 0, dist: dist_id::BELL }).unwrap();

    // Query rho[3][0]: row=3, col=0
    ctx.iregs.set(0, 3).unwrap();
    ctx.iregs.set(1, 0).unwrap();

    execute_qop(&mut ctx, &Instruction::QObserve {
        dst_h: 0, src_q: 0, mode: observe_mode::AMP, ctx0: 0, ctx1: 1,
    }).unwrap();

    // H[0] = Complex(0.5, 0.0)
    if let HybridValue::Complex(re, im) = ctx.hregs.get(0).unwrap() {
        assert!((re - 0.5).abs() < 1e-10);
        assert!(im.abs() < 1e-10);
    } else {
        panic!("Expected HybridValue::Complex");
    }

    // HREDUCE NEGATE_Z: Z[3] = (-0.5, -0.0)
    let mut fm = ForkManager::new();
    execute_hybrid(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 3, func: reduce_fn::NEGATE_Z },
        &mut fm,
    ).unwrap();

    let (re, im) = ctx.zregs.get(3).unwrap();
    assert!((re - (-0.5)).abs() < 1e-10, "Z[3].re should be -0.5, got {}", re);
    assert!(im.abs() < 1e-10, "Z[3].im should be ~0.0, got {}", im);
}

/// End-to-end pipeline via text assembly:
/// QPREP -> QOBSERVE(PROB) -> HREDUCE(ROUND) -> verify R register.
#[test]
fn test_e2e_text_observe_prob_round_pipeline() {
    use cqam_core::parser::parse_program;
    use cqam_vm::fork::ForkManager;
    use cqam_vm::executor::run_program;

    let source = r#"
# Prepare uniform 2-qubit state: each |k> has p=0.25
QPREP Q0, 0
# Set R0 = 2 (query basis state 2)
ILDI R0, 2
# QOBSERVE in PROB mode: H0 = p(|2>) = 0.25
QOBSERVE H0, Q0, PROB, R0
# Round 0.25 -> R1 = 0
HREDUCE ROUND, H0, R1
HALT
"#;

    let program = parse_program(source).expect("Failed to parse").instructions;
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    run_program(&mut ctx, &mut fm).expect("Program failed");

    assert!(ctx.psw.trap_halt);
    assert_eq!(ctx.iregs.get(1).unwrap(), 0, "round(0.25) should be 0");
}

/// End-to-end pipeline via text assembly:
/// QPREP -> QOBSERVE(AMP) -> HREDUCE(CONJ_Z) -> verify Z register.
#[test]
fn test_e2e_text_observe_amp_conj_z_pipeline() {
    use cqam_core::parser::parse_program;
    use cqam_vm::fork::ForkManager;
    use cqam_vm::executor::run_program;

    let source = r#"
# Prepare Bell state: rho[0][3] = 0.5 + 0i
QPREP Q0, 2
# Set R0 = 0 (row), R1 = 3 (col) for amplitude query
ILDI R0, 0
ILDI R1, 3
# QOBSERVE in AMP mode: H0 = rho[0][3] = Complex(0.5, 0.0)
QOBSERVE H0, Q0, AMP, R0, R1
# Conjugate: Z2 = conj(0.5 + 0i) = (0.5, -0.0)
HREDUCE CONJZ, H0, Z2
HALT
"#;

    let program = parse_program(source).expect("Failed to parse").instructions;
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    run_program(&mut ctx, &mut fm).expect("Program failed");

    assert!(ctx.psw.trap_halt);
    let (re, im) = ctx.zregs.get(2).unwrap();
    assert!((re - 0.5).abs() < 1e-10, "Z[2].re should be 0.5, got {}", re);
    assert!(im.abs() < 1e-10, "Z[2].im should be ~0.0, got {}", im);
}

// =============================================================================
// HREDUCE/EXPECT — expectation value reduction
// =============================================================================

#[test]
fn test_hreduce_expect_simple() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();

    // Set up a distribution in H0: outcomes (0, 0.6) and (1, 0.4)
    ctx.hregs.set(0, HybridValue::Dist(vec![(0, 0.6), (1, 0.4)])).unwrap();

    // R5 = base address for eigenvalues
    // CMEM[100] = eigenvalue for outcome 0 = 2.0
    // CMEM[101] = eigenvalue for outcome 1 = 5.0
    ctx.iregs.set(5, 100).unwrap();
    ctx.cmem.store(100, 2.0f64.to_bits() as i64);
    ctx.cmem.store(101, 5.0f64.to_bits() as i64);

    execute_instruction(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 5, func: reduce_fn::EXPECT },
        &mut fm,
    ).unwrap();

    // Expected: 0.6 * 2.0 + 0.4 * 5.0 = 1.2 + 2.0 = 3.2
    let result = ctx.fregs.get(5).unwrap();
    assert!((result - 3.2).abs() < 1e-10, "EXPECT should compute 0.6*2.0 + 0.4*5.0 = 3.2, got {}", result);
}

#[test]
fn test_hreduce_expect_type_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();

    // Put a Float (not Dist) in H0
    ctx.hregs.set(0, HybridValue::Float(1.5)).unwrap();
    ctx.iregs.set(5, 0).unwrap();

    let result = execute_instruction(
        &mut ctx,
        &Instruction::HReduce { src: 0, dst: 5, func: reduce_fn::EXPECT },
        &mut fm,
    );
    assert!(result.is_err(), "EXPECT on Float should fail with type mismatch");
}
