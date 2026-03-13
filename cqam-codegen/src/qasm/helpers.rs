//! Helper utilities for QASM code generation.
//!
//! Contains template loading (`load_template`, `load_gate_template`),
//! HReduce destination file lookup, and comparison emission.

use std::fs;
use std::path::Path;
use cqam_core::instruction::ReduceFn;

// ---------------------------------------------------------------------------
// Template loading
// ---------------------------------------------------------------------------

/// Load and substitute a QASM template file.
///
/// Reads `{template_dir}/{kernel_name}.qasm`, performs variable substitution:
///   {{DST}}    -> q{dst}
///   {{SRC}}    -> q{src}
///   {{PARAM0}} -> R{ctx0}
///   {{PARAM1}} -> R{ctx1}
///
/// Returns None if the template file does not exist or cannot be read.
pub fn load_template(
    template_dir: &str,
    kernel_name: &str,
    dst: u8,
    src: u8,
    ctx0: u8,
    ctx1: u8,
) -> Option<String> {
    let path = format!("{}/{}.qasm", template_dir, kernel_name);
    let content = fs::read_to_string(Path::new(&path)).ok()?;
    let substituted = content
        .replace("{{DST}}", &format!("q{}", dst))
        .replace("{{SRC}}", &format!("q{}", src))
        .replace("{{PARAM0}}", &format!("R{}", ctx0))
        .replace("{{PARAM1}}", &format!("R{}", ctx1));
    Some(substituted)
}

// ---------------------------------------------------------------------------
// Helper: determine HReduce target register file
// ---------------------------------------------------------------------------

/// Returns "R" for int-producing reduction functions (func 0-5),
/// "Z" for complex-to-Z reductions (func 14-15),
/// "F" for float-producing reduction functions (func 6-13).
pub(super) fn hreduce_dst_file(func: ReduceFn) -> &'static str {
    match func.output_file() {
        cqam_core::instruction::ReduceOutput::IntReg => "R",
        cqam_core::instruction::ReduceOutput::FloatReg => "F",
        cqam_core::instruction::ReduceOutput::ComplexReg => "Z",
    }
}

// ---------------------------------------------------------------------------
// Helper: emit comparison as if/else (valid QASM 3.0)
// ---------------------------------------------------------------------------

/// Emit an if/else comparison block for comparison instructions.
///
/// Produces valid OpenQASM 3.0 (no ternary `?:` operator).
pub(super) fn emit_comparison(dst: u8, lhs_prefix: &str, lhs: u8, op: &str, rhs_prefix: &str, rhs: u8) -> Vec<String> {
    vec![format!(
        "if ({}{} {} {}{}) {{ R{} = 1; }} else {{ R{} = 0; }}",
        lhs_prefix, lhs, op, rhs_prefix, rhs, dst, dst
    )]
}

// ---------------------------------------------------------------------------
// Helper: load kernel template for gate body
// ---------------------------------------------------------------------------

/// Load a kernel template for use inside a `gate` definition.
///
/// Replaces `{{DST}}` and `{{SRC}}` with the gate qubit parameter `q`.
/// Strips `{{PARAM0}}` and `{{PARAM1}}` (classical registers cannot appear
/// inside QASM 3.0 gate bodies).
///
/// Returns None if the template file does not exist.
pub fn load_gate_template(template_dir: &str, kernel_name: &str) -> Option<String> {
    let path = format!("{}/{}.qasm", template_dir, kernel_name);
    let content = fs::read_to_string(Path::new(&path)).ok()?;
    let substituted = content
        .replace("{{DST}}", "q")
        .replace("{{SRC}}", "q")
        .replace("{{PARAM0}}", "/* ctx0 */")
        .replace("{{PARAM1}}", "/* ctx1 */");
    Some(substituted)
}
