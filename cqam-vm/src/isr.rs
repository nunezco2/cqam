// cqam-vm/src/isr.rs

use crate::psw::Trap;
use crate::context::ExecutionContext;

pub fn handle_trap(trap: Trap, ctx: &mut ExecutionContext) {
    match trap {
        Trap::Arithmetic => {
            eprintln!("TRAP: Arithmetic fault - halting.");
            ctx.psw.trap_halt = true;
        }
        Trap::QuantumError => {
            eprintln!("INTERRUPT: Quantum fidelity failure - aborting kernel.");
            ctx.psw.trap_halt = true;
        }
        Trap::SyncFailure => {
            eprintln!("INTERRUPT: Hybrid sync failure - branch desynchronized.");
        }
        Trap::Halt => {
            eprintln!("TRAP: Explicit halt encountered.");
        }
    }
}
