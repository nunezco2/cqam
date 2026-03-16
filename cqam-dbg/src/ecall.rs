//! ECALL interceptor: captures ECALL output for the debugger OUTPUT pane
//! instead of writing to stdout/stderr.

use cqam_core::instruction::ProcId;
use cqam_vm::context::ExecutionContext;

/// Source of an output line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputSource {
    /// Output from an ECALL instruction.
    Ecall,
    /// Debugger diagnostic message (breakpoint hit, etc.).
    Debugger,
    /// Error message.
    Error,
}

/// A single line of captured output.
#[derive(Debug, Clone)]
pub struct OutputLine {
    /// The cycle count when this output was produced.
    pub cycle: usize,
    /// Where this output came from.
    pub source: OutputSource,
    /// The text content.
    pub text: String,
}

/// Interceptor that captures ECALL output into a buffer.
#[derive(Debug, Clone)]
pub struct EcallInterceptor {
    /// Captured output lines.
    pub buffer: Vec<OutputLine>,
}

impl EcallInterceptor {
    /// Create a new empty interceptor.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
        }
    }

    /// Handle an ECALL instruction by capturing its output and advancing PC.
    ///
    /// This reimplements the ECALL dispatch from executor.rs, but writes to
    /// the buffer instead of stdout/stderr.
    pub fn handle_ecall(&mut self, ctx: &mut ExecutionContext, cycle: usize) {
        let ecall_proc_id = if let Some(cqam_core::instruction::Instruction::Ecall { proc_id: pid }) =
            ctx.program.get(ctx.pc)
        {
            *pid
        } else {
            return;
        };

        let text = match ecall_proc_id {
            ProcId::PrintInt => {
                format!("{}", ctx.iregs.regs[0])
            }
            ProcId::PrintFloat => {
                format!("{}", ctx.fregs.regs[0])
            }
            ProcId::PrintChar => {
                let ch = ctx.iregs.regs[0] as u8 as char;
                format!("{}", ch)
            }
            ProcId::PrintStr => {
                // Simplified: just indicate a string print occurred.
                let base = ctx.iregs.regs[0] as u16;
                let len = ctx.iregs.regs[1] as u16;
                let mut s = String::new();
                for i in 0..len {
                    let addr = base.wrapping_add(i);
                    let val = ctx.cmem.load(addr);
                    let ch = (val & 0xFF) as u8 as char;
                    s.push(ch);
                }
                s
            }
            ProcId::DumpRegs => {
                // Simplified dump.
                let mut lines = Vec::new();
                for i in 0..16u8 {
                    let v = ctx.iregs.regs[i as usize];
                    if v != 0 {
                        lines.push(format!("R{}={}", i, v));
                    }
                }
                if lines.is_empty() {
                    "DUMP_REGS: (all zero)".to_string()
                } else {
                    format!("DUMP_REGS: {}", lines.join(" "))
                }
            }
            ProcId::PrintHist => {
                let h_index = ctx.iregs.regs[0] as u8;
                let mode = ctx.iregs.regs[1] as u32;
                let top_k = if ctx.iregs.regs[2] > 0 { ctx.iregs.regs[2] as u32 } else { 5 };
                if let Ok(value) = ctx.hregs.get(h_index) {
                    cqam_vm::histogram_fmt::format_histogram(
                        h_index, value, mode, top_k, ctx.config.default_qubits,
                    )
                } else {
                    format!("H{}: (invalid index)", h_index)
                }
            }
            // All ProcId variants are exhaustively matched above.
        };

        self.buffer.push(OutputLine {
            cycle,
            source: OutputSource::Ecall,
            text,
        });

        // Advance PC (ECALL is a simple fall-through instruction).
        ctx.pc += 1;
    }

    /// Clear all captured output.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for EcallInterceptor {
    fn default() -> Self {
        Self::new()
    }
}
