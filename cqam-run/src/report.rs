use cqam_vm::context::ExecutionContext;

pub fn print_report(ctx: &ExecutionContext, print_state: bool, print_psw: bool, print_resources: bool) {
    if print_state {
        println!("\n=== Final Register State ===");

        println!("--- Classical Registers ---");
        for (k, v) in &ctx.registers.c {
            println!("C[{}] = {:?}", k, v);
        }

        println!("--- Hybrid Registers ---");
        for (k, v) in &ctx.registers.h {
            println!("H[{}] = {:?}", k, v);
        }

        println!("\n=== Final Classical Memory ===");
        for (k, v) in &ctx.cmem.cells {
            println!("mem[{}] = {:?}", k, v);
        }

        println!("\n=== Final Quantum Memory ===");
        for (k, v) in &ctx.qmem.qdists {
            println!("q[{}] = {:?}", k, v.label);
        }
    }

    if print_psw {
        println!("\n=== Program State Word ===\n{:?}", ctx.psw);
    }

    if print_resources {
        println!("\n=== Resource Tracker ===\n{:?}", ctx.resource_tracker);
    }
}
