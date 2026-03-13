//! Declaration phase of the QASM code generator.
//!
//! Emits register declarations and kernel gate stub definitions for the
//! OpenQASM 3.0 output.

use super::types::{EmitMode, EmitConfig, UsedRegisters};
use super::helpers::load_gate_template;

// ---------------------------------------------------------------------------
// Declaration phase
// ---------------------------------------------------------------------------

/// Emit the declaration block for all used registers.
///
/// Returns a string containing one declaration per line, in the order:
/// 1. Integer registers (int[64])
/// 2. Float registers (float[64])
/// 3. Complex register pairs (float[64] for _re and _im)
/// 4. Quantum registers (qubit[16])
/// 5. Hybrid/measurement registers (bit[16])
/// 6. CMEM array (if used)
///
/// Returns an empty string if no registers are used.
pub fn emit_declarations(used: &UsedRegisters) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Integer registers
    for &r in &used.int_regs {
        lines.push(format!("int[64] R{};", r));
    }

    // Float registers
    for &r in &used.float_regs {
        lines.push(format!("float[64] F{};", r));
    }

    // Complex registers (lowered to paired floats)
    for &r in &used.complex_regs {
        lines.push(format!("float[64] Z{}_re;", r));
        lines.push(format!("float[64] Z{}_im;", r));
    }

    // Quantum registers
    for &r in &used.quantum_regs {
        lines.push(format!("qubit[16] q{};", r));
    }

    // Hybrid/measurement registers
    for &r in &used.hybrid_regs {
        lines.push(format!("bit[16] H{};", r));
    }

    // CMEM (no QASM 3.0 equivalent — emit as pragma comment)
    if used.uses_cmem {
        lines.push("// @cqam.cmem: classical memory (65536 x int[64]) -- no QASM equivalent".to_string());
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Kernel gate stubs
// ---------------------------------------------------------------------------

/// Emit gate definitions for all referenced kernels.
///
/// For each unique kernel ID in `used.kernel_ids`, emits a QASM 3.0
/// `gate` definition. If `config.expand_templates` is true, gate stubs
/// are NOT emitted (templates are inlined at call sites instead).
///
/// Returns an empty string if no kernels are used, if the mode is Fragment,
/// or if template expansion is enabled.
pub fn emit_kernel_stubs(
    used: &UsedRegisters,
    config: &EmitConfig,
) -> String {
    if used.kernel_ids.is_empty() || config.mode == EmitMode::Fragment || config.expand_templates {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::new();
    for &kid in &used.kernel_ids {
        let kname = kid.name();
        lines.push(format!("gate {} q {{", kname));
        match load_gate_template(&config.template_dir, kname) {
            Some(body) => {
                for line in body.lines() {
                    if !line.trim().is_empty() {
                        lines.push(format!("    {}", line));
                    }
                }
            }
            None => {
                lines.push(format!("    // {} kernel logic", kname));
            }
        }
        lines.push("}".to_string());
    }

    lines.join("\n")
}
