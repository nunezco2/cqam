//! Integration tests: step through quantum algorithm examples using the
//! DebuggerEngine and verify correctness of final state.
//!
//! Each test loads a `.cqam` program, runs it to completion via
//! `engine.step_one()`, then checks CMEM / register values against
//! the expected algorithm outputs.

use std::path::PathBuf;

use cqam_core::instruction::*;
use cqam_core::parser::ParsedProgram;
use cqam_dbg::engine::{DebuggerEngine, StopReason};
use cqam_run::loader::load_program;
use cqam_run::simconfig::SimConfig;

/// Resolve example path relative to workspace root (parent of crate dir).
fn example_path(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = std::path::Path::new(manifest).parent().unwrap();
    workspace.join(name).to_string_lossy().into_owned()
}

/// Load a program and build a DebuggerEngine with the correct pragma/data config.
fn load_and_build(path: &str) -> (DebuggerEngine, ParsedProgram) {
    let parsed = load_program(path).unwrap_or_else(|e| panic!("Failed to load {}: {}", path, e));
    let mut config = SimConfig::default();
    config.max_cycles = Some(500_000);
    let engine = DebuggerEngine::new_with_metadata(
        parsed.instructions.clone(),
        PathBuf::from(path),
        config,
        &parsed.metadata,
        Some(&parsed.data_section),
    );
    (engine, parsed)
}

/// Run an engine to completion (halt or end-of-program).
/// Panics on runtime errors or exceeding max cycles.
fn run_to_completion(engine: &mut DebuggerEngine) {
    loop {
        let result = engine.step_one();
        match result.stopped_reason {
            None => continue,
            Some(StopReason::Halted) | Some(StopReason::EndOfProgram) => break,
            Some(StopReason::MaxCycles) => panic!(
                "Max cycles ({}) exceeded at PC 0x{:04X}",
                engine.max_cycles, engine.ctx.pc
            ),
            Some(StopReason::Error(msg)) => panic!(
                "Runtime error at PC 0x{:04X}: {}",
                engine.ctx.pc, msg
            ),
            Some(StopReason::Breakpoint(id)) => panic!(
                "Unexpected breakpoint {} at PC 0x{:04X}",
                id, engine.ctx.pc
            ),
            Some(StopReason::Watchpoint(regs)) => panic!(
                "Unexpected watchpoint trigger: {:?}",
                regs
            ),
        }
    }
}

// ===================================================================
// GHZ Verification
// ===================================================================
// Expected: P(|0...0>) = 0.5, P(|1...1>) = 0.5, entanglement verified
#[test]
fn ghz_verify() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/ghz_verify.cqam"));
    run_to_completion(&mut engine);

    // CMEM layout from the program (post-QSAMPLE removal):
    // FSTR F5, 0  =>  CMEM[0] = mean of GHZ measurement (f64 bits)
    // FSTR F4, 1  =>  CMEM[1] = variance of GHZ measurement (f64 bits)
    // ISTR R6, 3  =>  CMEM[3] = mode of measurement (0 or 2^N-1)
    // ISTR R7, 4  =>  CMEM[4] = entanglement flag (expect 1)

    let mean = f64::from_bits(engine.ctx.cmem.load(0) as u64);
    let variance = f64::from_bits(engine.ctx.cmem.load(1) as u64);
    let entangled = engine.ctx.cmem.load(4);

    println!("GHZ: mean = {:.4}, variance = {:.4}, entangled = {}",
        mean, variance, entangled);

    // GHZ state has two outcomes (|0...0> and |1...1>) each with P=0.5.
    // For 16 qubits, outcomes are 0 and 65535.
    // Mean = 0.5 * 0 + 0.5 * 65535 = 32767.5
    let expected_mean = (((1u64 << 16) - 1) as f64) / 2.0;
    assert!(
        (mean - expected_mean).abs() < 1.0,
        "Mean should be ~{}, got {}",
        expected_mean, mean
    );

    // Variance should be large (bimodal distribution)
    assert!(
        variance > 1e6,
        "Variance should be large for bimodal GHZ, got {}",
        variance
    );

    println!("  entanglement flag = {} (informational)", entangled);
}

// ===================================================================
// Bernstein-Vazirani: recover secret string s=21
// ===================================================================
#[test]
fn bernstein_vazirani() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/bernstein_vazirani.cqam"));
    run_to_completion(&mut engine);

    // CMEM layout:
    // ISTR R1, 0  =>  CMEM[0] = secret (21)
    // ISTR R6, 1  =>  CMEM[1] = recovered string
    // ISTR R8, 2  =>  CMEM[2] = success flag (1 if recovered == secret)
    let secret = engine.ctx.cmem.load(0);
    let recovered = engine.ctx.cmem.load(1);
    let success = engine.ctx.cmem.load(2);

    println!("BV: secret={}, recovered={}, success={}", secret, recovered, success);

    assert_eq!(secret, 21, "secret should be 21");
    assert_eq!(recovered, 21, "recovered should match secret (21)");
    assert_eq!(success, 1, "success flag should be 1");
}

// ===================================================================
// Superdense Coding: encode and decode 4 messages (00, 01, 10, 11)
// ===================================================================
#[test]
fn superdense_coding() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/superdense_coding.cqam"));
    run_to_completion(&mut engine);

    // CMEM layout:
    // ISTR R3, 0  =>  CMEM[0] = decoded message 00 (expect 0)
    // ISTR R4, 1  =>  CMEM[1] = decoded message 01 (expect 1)
    // ISTR R5, 2  =>  CMEM[2] = decoded message 10 (expect 2)
    // ISTR R6, 3  =>  CMEM[3] = decoded message 11 (expect 3)
    // ISTR R8, 4  =>  CMEM[4] = verify flag for msg 00
    // ISTR R9, 5  =>  CMEM[5] = verify flag for msg 01
    // ISTR R10,6  =>  CMEM[6] = verify flag for msg 10
    // ISTR R11,7  =>  CMEM[7] = verify flag for msg 11
    let msg0 = engine.ctx.cmem.load(0);
    let msg1 = engine.ctx.cmem.load(1);
    let msg2 = engine.ctx.cmem.load(2);
    let msg3 = engine.ctx.cmem.load(3);
    let v0 = engine.ctx.cmem.load(4);
    let v1 = engine.ctx.cmem.load(5);
    let v2 = engine.ctx.cmem.load(6);
    let v3 = engine.ctx.cmem.load(7);

    println!("SDC: msg0={}, msg1={}, msg2={}, msg3={}", msg0, msg1, msg2, msg3);
    println!("     verify: v0={}, v1={}, v2={}, v3={}", v0, v1, v2, v3);

    assert_eq!(msg0, 0, "message 00 should decode to 0");
    assert_eq!(msg1, 1, "message 01 should decode to 1");
    assert_eq!(msg2, 2, "message 10 should decode to 2");
    assert_eq!(msg3, 3, "message 11 should decode to 3");
    assert_eq!(v0, 1, "verification flag for msg 00");
    assert_eq!(v1, 1, "verification flag for msg 01");
    assert_eq!(v2, 1, "verification flag for msg 10");
    assert_eq!(v3, 1, "verification flag for msg 11");
}

// ===================================================================
// Deutsch-Jozsa: constant vs balanced oracle
// ===================================================================
#[test]
fn deutsch_jozsa() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/deutsch_jozsa.cqam"));
    run_to_completion(&mut engine);

    // CMEM layout:
    // ISTR R4, 0  =>  CMEM[0] = constant test verdict (expect 1 = constant)
    // ISTR R5, 1  =>  CMEM[1] = balanced test verdict (expect 0 = balanced)
    // FSTR F0, 2  =>  CMEM[2] = P(|0>) for constant (expect ~1.0)
    // FSTR F1, 3  =>  CMEM[3] = P(|0>) for balanced (expect ~0.0)
    let constant_verdict = engine.ctx.cmem.load(0);
    let balanced_verdict = engine.ctx.cmem.load(1);
    let p0_constant = f64::from_bits(engine.ctx.cmem.load(2) as u64);
    let p0_balanced = f64::from_bits(engine.ctx.cmem.load(3) as u64);

    println!(
        "DJ: constant_verdict={}, balanced_verdict={}, P0_const={:.6}, P0_bal={:.6}",
        constant_verdict, balanced_verdict, p0_constant, p0_balanced
    );

    assert_eq!(
        constant_verdict, 1,
        "constant function should be detected as constant"
    );
    assert!(
        p0_constant > 0.99,
        "P(|0>) for constant oracle should be ~1.0, got {}",
        p0_constant
    );
    assert!(
        p0_balanced < 0.01,
        "P(|0>) for balanced oracle should be ~0.0, got {}",
        p0_balanced
    );
}

// ===================================================================
// Quantum Teleportation: protocol runs without error
// ===================================================================
#[test]
fn quantum_teleportation() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/quantum_teleport.cqam"));
    run_to_completion(&mut engine);

    // The teleportation protocol involves randomness (Alice's measurement),
    // so we can't predict the exact outcome. But we verify:
    // 1. The program halted successfully (no runtime errors)
    // 2. Alice's bits are valid (0, 1, 2, or 3 for 2 qubits)
    // 3. No quantum error was stored
    let alice_bits = engine.ctx.cmem.load(2);
    let error_flag = engine.ctx.cmem.load(999);

    println!("Teleport: alice_bits={}, error_flag={}", alice_bits, error_flag);

    assert!(
        alice_bits >= 0 && alice_bits <= 3,
        "Alice's measurement should be 0..3, got {}",
        alice_bits
    );
    assert_eq!(error_flag, 0, "no quantum error should have occurred");
    assert!(engine.ctx.psw.trap_halt, "program should have halted normally");
}

// ===================================================================
// EF flag: intent-based semantics
// ===================================================================
#[test]
fn test_ef_flag_ghz() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/ghz_verify.cqam"));
    run_to_completion(&mut engine);

    // With intent-based flag semantics, SF/EF/IF reflect the *last* quantum
    // operation's intent, not the dynamic state.  The GHZ-verify program ends
    // with QOBSERVE (measurement), which clears SF, EF, and IF because
    // observation is not a superposition/entanglement/interference operation.
    assert!(
        !engine.ctx.psw.ef,
        "EF should be false after QOBSERVE (observation clears intent flags)"
    );
    assert!(
        !engine.ctx.psw.sf,
        "SF should be false after QOBSERVE"
    );
    assert!(
        !engine.ctx.psw.inf,
        "IF should be false after QOBSERVE"
    );

    // The program itself verified entanglement mid-execution via JMPF on
    // the EF flag and stored the result in CMEM[4].  That should be 1.
    let entangled = engine.ctx.cmem.load(4);
    assert_eq!(
        entangled, 1,
        "Program's own entanglement verification (CMEM[4]) should confirm entanglement"
    );
}

// ===================================================================
// GHZ with stepping: verify quantum state mid-execution
// ===================================================================
// This test uses breakpoints to stop at key points and inspect the quantum state.
#[test]
fn ghz_stepping_with_breakpoints() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/ghz_verify.cqam"));

    // Find the QPREP Q0, GHZS instruction (GHZ preparation).
    let qprep_pc = engine
        .ctx
        .program
        .iter()
        .position(|i| matches!(i, cqam_core::instruction::Instruction::QPrep { dst: 0, dist: DistId::Ghz }))
        .expect("Should find QPREP Q0, GHZS in GHZ program");

    // Set a breakpoint right after QPREP (to inspect the freshly prepared GHZ state).
    let bp_id = engine.breakpoints.add_address(qprep_pc + 1);

    // Run until the breakpoint.
    loop {
        let result = engine.step_one();
        match result.stopped_reason {
            None => continue,
            Some(StopReason::Breakpoint(id)) if id == bp_id => break,
            Some(other) => panic!("Unexpected stop: {:?}", other),
        }
    }

    // At this point, Q0 should contain the GHZ state.
    // Verify Q0 is allocated and is a pure state via the backend.
    use cqam_core::quantum_backend::QuantumBackend;

    let handle = engine.ctx.qregs[0]
        .expect("Q0 should be allocated after QPREP");

    let is_pure = engine.backend.is_pure(handle).expect("should be able to query state");
    assert!(is_pure, "GHZ state should be pure, got density matrix");

    let n = engine.backend.num_qubits(handle).unwrap();
    let dim = engine.backend.dimension(handle).unwrap();
    let probs = engine.backend.diagonal_probabilities(handle).unwrap();

    let p_zero = probs[0];
    let p_ones = probs[dim - 1];
    let p_others: f64 = (1..dim - 1).map(|i| probs[i]).sum();

    println!(
        "GHZ mid-step: n={}, P(|0..0>)={:.6}, P(|1..1>)={:.6}, P(others)={:.6}",
        n, p_zero, p_ones, p_others
    );

    assert!(
        (p_zero - 0.5).abs() < 1e-10,
        "P(|0...0>) should be exactly 0.5, got {}",
        p_zero
    );
    assert!(
        (p_ones - 0.5).abs() < 1e-10,
        "P(|1...1>) should be exactly 0.5, got {}",
        p_ones
    );
    assert!(
        p_others < 1e-20,
        "All other amplitudes should be zero, got total P={}",
        p_others
    );

    // Now continue to completion.
    engine.breakpoints.remove(bp_id);
    loop {
        let result = engine.step_one();
        match result.stopped_reason {
            None => continue,
            Some(StopReason::Halted) | Some(StopReason::EndOfProgram) => break,
            Some(other) => panic!("Unexpected stop: {:?}", other),
        }
    }
}

// ===================================================================
// Bernstein-Vazirani with watchpoints: verify R6 gets the right value
// ===================================================================
#[test]
fn bv_with_watchpoint() {
    use cqam_dbg::engine::watchpoint::Watchpoint;

    let (mut engine, _) = load_and_build(&example_path("examples/basic/bernstein_vazirani.cqam"));

    // Watch R6 (where the recovered secret will be stored).
    engine
        .watchpoints
        .add(Watchpoint::parse("R6").unwrap());

    // Run until the watchpoint triggers (R6 changes from 0 to the recovered value).
    let mut watchpoint_triggered = false;
    loop {
        let result = engine.step_one();
        match result.stopped_reason {
            None => continue,
            Some(StopReason::Watchpoint(regs)) => {
                assert!(regs.contains(&"R6".to_string()), "R6 should be in triggered list");
                // At this point, R6 should contain the recovered secret.
                let r6 = engine.ctx.iregs.regs[6];
                println!("BV watchpoint: R6 changed to {}", r6);
                // The first time R6 changes, it should be set to the mode of the measurement.
                watchpoint_triggered = true;
                // Remove watchpoint and continue.
                engine.watchpoints.remove("R6");
            }
            Some(StopReason::Halted) | Some(StopReason::EndOfProgram) => break,
            Some(other) => panic!("Unexpected stop: {:?}", other),
        }
    }

    assert!(watchpoint_triggered, "Watchpoint on R6 should have triggered");

    // Final verification: the recovered value should be 21.
    let recovered = engine.ctx.cmem.load(1);
    assert_eq!(recovered, 21, "recovered secret should be 21");
}

// ===================================================================
// PSW flag tests: DF, CF, FK, MG
// ===================================================================

/// After Bernstein-Vazirani runs to completion, QOBSERVE has occurred so
/// DF (sticky decoherence flag) must be true. CF may be false because
/// HREDUCE clears it after consuming the measurement result.
#[test]
fn test_df_cf_after_observe() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/bernstein_vazirani.cqam"));
    run_to_completion(&mut engine);

    // QOBSERVE sets DF (sticky). DF is only cleared by QPREP or PSW::clear(),
    // neither of which occurs after QOBSERVE in this program.
    assert!(
        engine.ctx.psw.df,
        "DF should be true after QOBSERVE (sticky decoherence flag)"
    );

    // The program runs HREDUCE after QOBSERVE, which clears CF.
    // No subsequent QOBSERVE/QMEAS re-sets it before HALT.
    assert!(
        !engine.ctx.psw.cf,
        "CF should be false: HREDUCE clears the transient collapsed flag"
    );
}

/// DF is sticky across kernels but cleared by QPREP. Use GHZ with a
/// breakpoint after QPREP to confirm DF is false at that point, then
/// run to completion where QOBSERVE sets DF=true.
#[test]
fn test_df_sticky_across_kernels() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/ghz_verify.cqam"));

    // Find the first QPREP Q0, GHZS (GHZ preparation) instruction.
    let qprep_pc = engine
        .ctx
        .program
        .iter()
        .position(|i| matches!(i, cqam_core::instruction::Instruction::QPrep { dst: 0, dist: DistId::Ghz }))
        .expect("Should find QPREP Q0, GHZS in GHZ program");

    // Set a breakpoint right after QPREP (to inspect flags post-preparation).
    let bp_id = engine.breakpoints.add_address(qprep_pc + 1);

    // Run until the breakpoint.
    loop {
        let result = engine.step_one();
        match result.stopped_reason {
            None => continue,
            Some(StopReason::Breakpoint(id)) if id == bp_id => break,
            Some(other) => panic!("Unexpected stop before breakpoint: {:?}", other),
        }
    }

    // At the breakpoint (right after QPREP), DF should be false because
    // QPREP calls clear_decoherence().
    assert!(
        !engine.ctx.psw.df,
        "DF should be false right after QPREP (QPREP clears decoherence)"
    );
    assert!(
        !engine.ctx.psw.cf,
        "CF should be false right after QPREP"
    );

    // Continue to completion. The GHZ program performs QOBSERVE, which
    // sets DF=true and it should remain set through subsequent operations
    // (QKERNEL may clear CF but DF is sticky).
    engine.breakpoints.remove(bp_id);
    loop {
        let result = engine.step_one();
        match result.stopped_reason {
            None => continue,
            Some(StopReason::Halted) | Some(StopReason::EndOfProgram) => break,
            Some(other) => panic!("Unexpected stop: {:?}", other),
        }
    }

    // After QOBSERVE has run, DF must be true (sticky).
    // Note: the GHZ program runs further QPREPs after the first QOBSERVE
    // (for Q1, Q3 etc.) which clear DF. However, there is a final QOBSERVE
    // on Q3 (line 135) after HMERGE, so DF should be true at the end.
    assert!(
        engine.ctx.psw.df,
        "DF should be true after program completion (final QOBSERVE sets it)"
    );
}

/// Verify FK and MG flags through the GHZ program's HFORK/HMERGE block.
/// After HMERGE completes and the program reaches HALT, FK should be
/// false and MG should be true (HMERGE sets MG and clears FK).
/// However, note that any subsequent HFORK after HMERGE would clear MG.
/// The GHZ program has: HFORK ... HMERGE ... QOBSERVE ... HALT
/// So at HALT: FK=false, MG=true (no subsequent HFORK after HMERGE).
#[test]
fn test_fk_mg_with_hfork_hmerge() {
    let (mut engine, _) = load_and_build(&example_path("examples/basic/ghz_verify.cqam"));
    run_to_completion(&mut engine);

    // The GHZ program executes HFORK then HMERGE. After HMERGE:
    //   FK (forked) = false  (HMERGE clears it)
    //   MG (merged) = true   (HMERGE sets it)
    // No subsequent HFORK occurs before HALT, so these values persist.
    assert!(
        !engine.ctx.psw.forked,
        "FK should be false after HMERGE (HMERGE clears forked)"
    );
    assert!(
        engine.ctx.psw.merged,
        "MG should be true after HMERGE (HMERGE sets merged)"
    );

    // Also verify via get_flag() with numeric IDs.
    assert!(
        !engine.ctx.psw.get_flag(10),
        "get_flag(10) = FK should be false"
    );
    assert!(
        engine.ctx.psw.get_flag(11),
        "get_flag(11) = MG should be true"
    );
}
