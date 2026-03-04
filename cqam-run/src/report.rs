// cqam-run/src/report.rs
//
// Phase 2: Updated to iterate over fixed-size register file arrays
// instead of HashMap entries.

use cqam_core::register::HybridValue;
use cqam_vm::context::ExecutionContext;

/// Print execution results based on selected report options.
///
/// - `print_state`: Print all non-zero register values, non-zero memory cells,
///   and active quantum/hybrid registers.
/// - `print_psw`: Print the full Program State Word.
/// - `print_resources`: Print cumulative resource usage.
pub fn print_report(
    ctx: &ExecutionContext,
    print_state: bool,
    print_psw: bool,
    print_resources: bool,
) {
    if print_state {
        // -- Integer registers (R0-R15) --
        println!("\n=== Integer Registers (non-zero) ===");
        for i in 0..16u8 {
            let val = ctx.iregs.get(i);
            if val != 0 {
                println!("  R{:2} = {}", i, val);
            }
        }

        // -- Float registers (F0-F15) --
        println!("\n=== Float Registers (non-zero) ===");
        for i in 0..16u8 {
            let val = ctx.fregs.get(i);
            if val != 0.0 {
                println!("  F{:2} = {:.6}", i, val);
            }
        }

        // -- Complex registers (Z0-Z15) --
        println!("\n=== Complex Registers (non-zero) ===");
        for i in 0..16u8 {
            let (re, im) = ctx.zregs.get(i);
            if re != 0.0 || im != 0.0 {
                println!("  Z{:2} = ({:.6}, {:.6}i)", i, re, im);
            }
        }

        // -- Hybrid registers (H0-H7) --
        println!("\n=== Hybrid Registers (non-empty) ===");
        for i in 0..8u8 {
            let val = ctx.hregs.get(i);
            if !matches!(val, HybridValue::Empty) {
                println!("  H{} = {:?}", i, val);
            }
        }

        // -- Quantum registers (Q0-Q7) --
        println!("\n=== Quantum Registers (active) ===");
        for i in 0..8usize {
            if let Some(ref qdist) = ctx.qregs[i] {
                println!("  Q{} = {} ({} states)", i, qdist.label, qdist.domain.len());
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
            if let Some(qdist) = ctx.qmem.load(addr) {
                println!("  QMEM[{:3}] = {} ({} states)", addr, qdist.label, qdist.domain.len());
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
