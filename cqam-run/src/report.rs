//! Post-execution report formatting for the CQAM runner.
//!
//! Prints the final state of classical registers, quantum registers,
//! and classical memory after a program completes.

use cqam_core::register::HybridValue;
use crate::shot::RunResult;

/// Print execution results based on selected report options.
///
/// - `print_state`: Print all non-zero register values, non-zero memory cells,
///   and active quantum/hybrid registers.
/// - `print_psw`: Print the full Program State Word.
/// - `print_resources`: Print cumulative resource usage.
pub fn print_report(
    result: &RunResult,
    print_state: bool,
    print_psw: bool,
    print_resources: bool,
) {
    let ctx = result.ctx();
    let is_shots = matches!(result, RunResult::Shots(_));

    if print_state {
        // -- Integer registers (R0-R15) --
        println!("\n=== Integer Registers (non-zero) ===");
        for i in 0..16u8 {
            let val = ctx.iregs.get(i).unwrap_or(0);
            if val != 0 {
                println!("  R{:2} = {}", i, val);
            }
        }

        // -- Float registers (F0-F15) --
        println!("\n=== Float Registers (non-zero) ===");
        for i in 0..16u8 {
            let val = ctx.fregs.get(i).unwrap_or(0.0);
            if val != 0.0 {
                println!("  F{:2} = {:.6}", i, val);
            }
        }

        // -- Complex registers (Z0-Z15) --
        println!("\n=== Complex Registers (non-zero) ===");
        for i in 0..16u8 {
            let (re, im) = ctx.zregs.get(i).unwrap_or((0.0, 0.0));
            if re != 0.0 || im != 0.0 {
                println!("  Z{:2} = ({:.6}, {:.6}i)", i, re, im);
            }
        }

        // -- Hybrid registers (H0-H7) --
        if is_shots {
            println!("\n=== Hybrid Registers (shot histograms) ===");
        } else {
            println!("\n=== Hybrid Registers (non-empty) ===");
        }
        for i in 0..8u8 {
            if let Ok(val) = ctx.hregs.get(i) {
                match val {
                    HybridValue::Empty => {}
                    HybridValue::Hist(hist) => {
                        println!("  H{} = ShotHistogram ({} shots, {} outcomes):", i, hist.total_shots, hist.num_outcomes());
                        let max_state = hist.counts.keys().last().copied().unwrap_or(0);
                        let width = if max_state == 0 {
                            1
                        } else {
                            1 + (max_state as f64).log2().ceil() as usize
                        };
                        for (&state, &count) in &hist.counts {
                            let prob = count as f64 / hist.total_shots as f64;
                            println!("    |{:0>width$}> : {} ({:.4})",
                                state,
                                count,
                                prob,
                                width = width,
                            );
                        }
                    }
                    _ => {
                        println!("  H{} = {:?}", i, val);
                    }
                }
            }
        }

        // -- Quantum registers (Q0-Q7) --
        println!("\n=== Quantum Registers (active) ===");
        for i in 0..8usize {
            if let Some(handle) = ctx.qregs[i] {
                println!("  Q{} = QRegHandle({})", i, handle.0);
            }
        }

        // -- Classical memory (non-zero cells) --
        println!("\n=== Classical Memory (non-zero) ===");
        for (addr, val) in ctx.cmem.non_zero_entries() {
            println!("  CMEM[{:5}] = {}", addr, val);
        }

        // -- Quantum memory (occupied slots) --
        println!("\n=== Quantum Memory (occupied slots) ===");
        for addr in 0..=255u8 {
            if let Some(handle) = ctx.qmem.load(addr) {
                println!("  QMEM[{:3}] = QRegHandle({})", addr, handle.0);
            }
        }
    }

    if print_psw {
        println!("\n=== Program State Word ===");
        println!("{:?}", ctx.psw);
    }

    if print_resources {
        println!("\n=== Resource Tracker ===");
        println!("{:?}", ctx.resource_tracker);
    }
}
