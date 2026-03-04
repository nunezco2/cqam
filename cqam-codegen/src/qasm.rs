// cqam-codegen/src/qasm.rs
//
// Phase 7: Rewritten QASM codegen with proper declarations, emit modes,
// and valid OpenQASM 3.0 output.
//
// The emitter follows a three-phase pipeline:
//   1. SCAN  -- walk all instructions, collect used register indices
//   2. DECLARE -- emit one declaration per used register (standalone only)
//   3. EMIT  -- translate each instruction to QASM body lines

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use cqam_core::instruction::{Instruction, dist_name, kernel_name, flag_name, reduce_fn_name};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Controls how QASM output is structured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitMode {
    /// Full program: OPENQASM header, includes, declarations, body, footer.
    Standalone,
    /// Body only: no header, no includes, no declarations, no gate stubs.
    /// Suitable for embedding in a larger QASM program.
    Fragment,
}

/// Configuration for QASM emission.
#[derive(Debug, Clone)]
pub struct EmitConfig {
    /// Standalone or fragment mode.
    pub mode: EmitMode,
    /// Whether to expand kernel templates from disk.
    pub expand_templates: bool,
    /// Base directory for template file lookup.
    /// Default: "kernels/qasm_templates"
    pub template_dir: String,
}

impl Default for EmitConfig {
    fn default() -> Self {
        EmitConfig {
            mode: EmitMode::Standalone,
            expand_templates: false,
            template_dir: "kernels/qasm_templates".to_string(),
        }
    }
}

impl EmitConfig {
    /// Create a standalone config with template expansion enabled.
    pub fn standalone() -> Self {
        EmitConfig {
            mode: EmitMode::Standalone,
            expand_templates: true,
            ..Default::default()
        }
    }

    /// Create a fragment config with template expansion disabled.
    pub fn fragment() -> Self {
        EmitConfig {
            mode: EmitMode::Fragment,
            expand_templates: false,
            ..Default::default()
        }
    }
}

/// Tracks which registers are used across a program.
///
/// Populated by `scan_registers()` during the scan phase. Each field is a
/// sorted set of register indices that appear as operands (read or write)
/// in at least one instruction.
#[derive(Debug, Clone, Default)]
pub struct UsedRegisters {
    /// Integer registers R0-R15 that appear in instructions.
    pub int_regs: BTreeSet<u8>,
    /// Float registers F0-F15 that appear in instructions.
    pub float_regs: BTreeSet<u8>,
    /// Complex registers Z0-Z15 that appear in instructions.
    /// Each entry generates two float declarations (re + im).
    pub complex_regs: BTreeSet<u8>,
    /// Quantum registers Q0-Q7 that appear in instructions.
    pub quantum_regs: BTreeSet<u8>,
    /// Hybrid registers H0-H7 that appear in instructions.
    pub hybrid_regs: BTreeSet<u8>,
    /// Whether any instruction accesses CMEM (ILdm, IStr, FLdm, FStr, ZLdm, ZStr).
    pub uses_cmem: bool,
    /// Whether any instruction accesses QMEM (QLoad, QStore).
    pub uses_qmem: bool,
    /// Set of kernel IDs referenced by QKernel instructions.
    pub kernel_ids: BTreeSet<u8>,
    /// Label names in program order (from Label instructions).
    pub labels: Vec<String>,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Trait for converting CQAM instructions into OpenQASM 3.0 strings.
pub trait QasmFormat {
    /// Convert a single instruction to its QASM body representation.
    ///
    /// Returns a Vec of QASM lines (possibly empty for Nop). Each line is
    /// a complete QASM statement without trailing newline.
    ///
    /// Body lines do NOT include type declarations -- those are emitted
    /// separately by `emit_declarations()`.
    fn to_qasm(&self, config: &EmitConfig) -> Vec<String>;
}

// ---------------------------------------------------------------------------
// QasmFormat implementation for Instruction
// ---------------------------------------------------------------------------

impl QasmFormat for Instruction {
    fn to_qasm(&self, config: &EmitConfig) -> Vec<String> {
        match self {
            // -- Integer arithmetic ------------------------------------------

            Instruction::IAdd { dst, lhs, rhs } => {
                vec![format!("R{} = R{} + R{};", dst, lhs, rhs)]
            }
            Instruction::ISub { dst, lhs, rhs } => {
                vec![format!("R{} = R{} - R{};", dst, lhs, rhs)]
            }
            Instruction::IMul { dst, lhs, rhs } => {
                vec![format!("R{} = R{} * R{};", dst, lhs, rhs)]
            }
            Instruction::IDiv { dst, lhs, rhs } => {
                vec![format!("R{} = R{} / R{};", dst, lhs, rhs)]
            }
            Instruction::IMod { dst, lhs, rhs } => {
                vec![format!("R{} = R{} % R{};", dst, lhs, rhs)]
            }

            // -- Integer bitwise ---------------------------------------------

            Instruction::IAnd { dst, lhs, rhs } => {
                vec![format!("R{} = R{} & R{};", dst, lhs, rhs)]
            }
            Instruction::IOr { dst, lhs, rhs } => {
                vec![format!("R{} = R{} | R{};", dst, lhs, rhs)]
            }
            Instruction::IXor { dst, lhs, rhs } => {
                vec![format!("R{} = R{} ^ R{};", dst, lhs, rhs)]
            }
            Instruction::INot { dst, src } => {
                vec![format!("R{} = ~R{};", dst, src)]
            }
            Instruction::IShl { dst, src, amt } => {
                vec![format!("R{} = R{} << {};", dst, src, amt)]
            }
            Instruction::IShr { dst, src, amt } => {
                vec![format!("R{} = R{} >> {};", dst, src, amt)]
            }

            // -- Integer memory ----------------------------------------------

            Instruction::ILdi { dst, imm } => {
                vec![format!("R{} = {};", dst, imm)]
            }
            Instruction::ILdm { dst, addr } => {
                vec![format!("R{} = CMEM[{}];", dst, addr)]
            }
            Instruction::IStr { src, addr } => {
                vec![format!("CMEM[{}] = R{};", addr, src)]
            }

            // -- Integer comparison ------------------------------------------

            Instruction::IEq { dst, lhs, rhs } => {
                vec![format!("R{} = (R{} == R{}) ? 1 : 0;", dst, lhs, rhs)]
            }
            Instruction::ILt { dst, lhs, rhs } => {
                vec![format!("R{} = (R{} < R{}) ? 1 : 0;", dst, lhs, rhs)]
            }
            Instruction::IGt { dst, lhs, rhs } => {
                vec![format!("R{} = (R{} > R{}) ? 1 : 0;", dst, lhs, rhs)]
            }

            // -- Float arithmetic --------------------------------------------

            Instruction::FAdd { dst, lhs, rhs } => {
                vec![format!("F{} = F{} + F{};", dst, lhs, rhs)]
            }
            Instruction::FSub { dst, lhs, rhs } => {
                vec![format!("F{} = F{} - F{};", dst, lhs, rhs)]
            }
            Instruction::FMul { dst, lhs, rhs } => {
                vec![format!("F{} = F{} * F{};", dst, lhs, rhs)]
            }
            Instruction::FDiv { dst, lhs, rhs } => {
                vec![format!("F{} = F{} / F{};", dst, lhs, rhs)]
            }
            Instruction::FLdi { dst, imm } => {
                vec![format!("F{} = {}.0;", dst, imm)]
            }
            Instruction::FLdm { dst, addr } => {
                vec![format!("F{} = CMEM[{}];", dst, addr)]
            }
            Instruction::FStr { src, addr } => {
                vec![format!("CMEM[{}] = F{};", addr, src)]
            }

            // -- Float comparison (result to int register) -------------------

            Instruction::FEq { dst, lhs, rhs } => {
                vec![format!("R{} = (F{} == F{}) ? 1 : 0;", dst, lhs, rhs)]
            }
            Instruction::FLt { dst, lhs, rhs } => {
                vec![format!("R{} = (F{} < F{}) ? 1 : 0;", dst, lhs, rhs)]
            }
            Instruction::FGt { dst, lhs, rhs } => {
                vec![format!("R{} = (F{} > F{}) ? 1 : 0;", dst, lhs, rhs)]
            }

            // -- Complex arithmetic (lowered to paired floats) ---------------

            Instruction::ZAdd { dst, lhs, rhs } => {
                vec![
                    format!("Z{}_re = Z{}_re + Z{}_re;", dst, lhs, rhs),
                    format!("Z{}_im = Z{}_im + Z{}_im;", dst, lhs, rhs),
                ]
            }
            Instruction::ZSub { dst, lhs, rhs } => {
                vec![
                    format!("Z{}_re = Z{}_re - Z{}_re;", dst, lhs, rhs),
                    format!("Z{}_im = Z{}_im - Z{}_im;", dst, lhs, rhs),
                ]
            }
            Instruction::ZMul { dst, lhs, rhs } => {
                vec![
                    format!("// ZMUL: Z{} = Z{} * Z{}", dst, lhs, rhs),
                    format!(
                        "Z{}_re = Z{}_re * Z{}_re - Z{}_im * Z{}_im;",
                        dst, lhs, rhs, lhs, rhs
                    ),
                    format!(
                        "Z{}_im = Z{}_re * Z{}_im + Z{}_im * Z{}_re;",
                        dst, lhs, rhs, lhs, rhs
                    ),
                ]
            }
            Instruction::ZDiv { dst, lhs, rhs } => {
                vec![
                    format!("// ZDIV: Z{} = Z{} / Z{}", dst, lhs, rhs),
                    format!("// denom = Z{}_re^2 + Z{}_im^2", rhs, rhs),
                    format!(
                        "Z{}_re = (Z{}_re * Z{}_re + Z{}_im * Z{}_im) / (Z{}_re * Z{}_re + Z{}_im * Z{}_im);",
                        dst, lhs, rhs, lhs, rhs, rhs, rhs, rhs, rhs
                    ),
                    format!(
                        "Z{}_im = (Z{}_im * Z{}_re - Z{}_re * Z{}_im) / (Z{}_re * Z{}_re + Z{}_im * Z{}_im);",
                        dst, lhs, rhs, lhs, rhs, rhs, rhs, rhs, rhs
                    ),
                ]
            }

            // -- Complex memory ----------------------------------------------

            Instruction::ZLdi { dst, imm_re, imm_im } => {
                vec![
                    format!("Z{}_re = {}.0;", dst, imm_re),
                    format!("Z{}_im = {}.0;", dst, imm_im),
                ]
            }
            Instruction::ZLdm { dst, addr } => {
                vec![
                    format!("Z{}_re = CMEM[{}];", dst, addr),
                    format!("Z{}_im = CMEM[{}];", dst, addr + 1),
                ]
            }
            Instruction::ZStr { src, addr } => {
                vec![
                    format!("CMEM[{}] = Z{}_re;", addr, src),
                    format!("CMEM[{}] = Z{}_im;", addr + 1, src),
                ]
            }

            // -- Type conversion ---------------------------------------------

            Instruction::CvtIF { dst_f, src_i } => {
                vec![format!("F{} = float[64](R{});", dst_f, src_i)]
            }
            Instruction::CvtFI { dst_i, src_f } => {
                vec![format!("R{} = int[64](F{});", dst_i, src_f)]
            }
            Instruction::CvtFZ { dst_z, src_f } => {
                vec![
                    format!("Z{}_re = F{};", dst_z, src_f),
                    format!("Z{}_im = 0.0;", dst_z),
                ]
            }
            Instruction::CvtZF { dst_f, src_z } => {
                vec![format!("F{} = Z{}_re;", dst_f, src_z)]
            }

            // -- Control flow ------------------------------------------------

            Instruction::Jmp { target } => {
                vec![format!("goto {};", target)]
            }
            Instruction::Jif { pred, target } => {
                vec![format!("if (R{} != 0) goto {};", pred, target)]
            }
            Instruction::Call { target } => {
                vec![format!("// CALL {} [no QASM equivalent]", target)]
            }
            Instruction::Ret => {
                vec!["// RET [no QASM equivalent]".to_string()]
            }
            Instruction::Halt => {
                vec!["// HALT".to_string()]
            }

            // -- Quantum operations ------------------------------------------

            Instruction::QPrep { dst, dist } => {
                vec![
                    format!("reset q{};", dst),
                    format!("// QPrep: initialize q{} with distribution '{}'", dst, dist_name(*dist)),
                ]
            }
            Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
                let kname = kernel_name(*kernel);
                let header = format!(
                    "// QKernel: q{} = {}(q{}, ctx=[R{}, R{}])",
                    dst, kname, src, ctx0, ctx1
                );

                if config.expand_templates {
                    match load_template(&config.template_dir, kname, *dst, *src, *ctx0, *ctx1) {
                        Some(expanded) => {
                            let mut lines = vec![header];
                            for line in expanded.lines() {
                                lines.push(line.to_string());
                            }
                            lines
                        }
                        None => {
                            vec![
                                header,
                                format!("// [Missing QASM template for {}]", kname),
                            ]
                        }
                    }
                } else {
                    vec![
                        header,
                        format!("{} q{};", kname, dst),
                    ]
                }
            }
            Instruction::QObserve { dst_h, src_q } => {
                vec![format!("H{} = measure q{};", dst_h, src_q)]
            }
            Instruction::QLoad { dst_q, addr } => {
                vec![format!("// QLOAD q{} from QMEM[{}] [no QASM equivalent]", dst_q, addr)]
            }
            Instruction::QStore { src_q, addr } => {
                vec![format!("// QSTORE q{} to QMEM[{}] [no QASM equivalent]", src_q, addr)]
            }

            // -- Hybrid operations (CQAM-specific annotations) ---------------

            Instruction::HFork => {
                vec!["// @cqam.hfork: begin parallel execution region".to_string()]
            }
            Instruction::HMerge => {
                vec!["// @cqam.hmerge: end parallel execution region, merge results".to_string()]
            }
            Instruction::HCExec { flag, target } => {
                vec![format!(
                    "// @cqam.hcexec: if PSW.{} goto {}",
                    flag_name(*flag), target
                )]
            }
            Instruction::HReduce { src, dst, func } => {
                let dst_file = hreduce_dst_file(*func);
                let fname = reduce_fn_name(*func);
                vec![format!(
                    "// @cqam.hreduce: {}{} = {}(H{})",
                    dst_file, dst, fname, src
                )]
            }

            // -- Pseudo-instructions -----------------------------------------

            Instruction::Nop => {
                vec![]
            }
            Instruction::Label(name) => {
                vec![format!("{}:", name)]
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Scan phase
// ---------------------------------------------------------------------------

/// Scan a program and collect all register indices that appear as operands.
///
/// Walks every instruction once, extracting read and write register indices
/// into the appropriate sets in `UsedRegisters`. Also detects CMEM/QMEM
/// usage and collects kernel IDs and label names.
///
/// Complexity: O(n) where n is the number of instructions.
pub fn scan_registers(program: &[Instruction]) -> UsedRegisters {
    let mut used = UsedRegisters::default();
    for instr in program {
        scan_instruction(instr, &mut used);
    }
    used
}

/// Internal helper: extract register operands from a single instruction
/// and insert them into the UsedRegisters sets.
fn scan_instruction(instr: &Instruction, used: &mut UsedRegisters) {
    match instr {
        // -- Integer arithmetic (all three-register forms) --
        Instruction::IAdd { dst, lhs, rhs }
        | Instruction::ISub { dst, lhs, rhs }
        | Instruction::IMul { dst, lhs, rhs }
        | Instruction::IDiv { dst, lhs, rhs }
        | Instruction::IMod { dst, lhs, rhs } => {
            used.int_regs.insert(*dst);
            used.int_regs.insert(*lhs);
            used.int_regs.insert(*rhs);
        }

        // -- Integer bitwise (three-register forms) --
        Instruction::IAnd { dst, lhs, rhs }
        | Instruction::IOr { dst, lhs, rhs }
        | Instruction::IXor { dst, lhs, rhs } => {
            used.int_regs.insert(*dst);
            used.int_regs.insert(*lhs);
            used.int_regs.insert(*rhs);
        }

        Instruction::INot { dst, src } => {
            used.int_regs.insert(*dst);
            used.int_regs.insert(*src);
        }

        Instruction::IShl { dst, src, .. }
        | Instruction::IShr { dst, src, .. } => {
            used.int_regs.insert(*dst);
            used.int_regs.insert(*src);
        }

        // -- Integer memory --
        Instruction::ILdi { dst, .. } => {
            used.int_regs.insert(*dst);
        }
        Instruction::ILdm { dst, .. } => {
            used.int_regs.insert(*dst);
            used.uses_cmem = true;
        }
        Instruction::IStr { src, .. } => {
            used.int_regs.insert(*src);
            used.uses_cmem = true;
        }

        // -- Integer comparison --
        Instruction::IEq { dst, lhs, rhs }
        | Instruction::ILt { dst, lhs, rhs }
        | Instruction::IGt { dst, lhs, rhs } => {
            used.int_regs.insert(*dst);
            used.int_regs.insert(*lhs);
            used.int_regs.insert(*rhs);
        }

        // -- Float arithmetic --
        Instruction::FAdd { dst, lhs, rhs }
        | Instruction::FSub { dst, lhs, rhs }
        | Instruction::FMul { dst, lhs, rhs }
        | Instruction::FDiv { dst, lhs, rhs } => {
            used.float_regs.insert(*dst);
            used.float_regs.insert(*lhs);
            used.float_regs.insert(*rhs);
        }

        Instruction::FLdi { dst, .. } => {
            used.float_regs.insert(*dst);
        }
        Instruction::FLdm { dst, .. } => {
            used.float_regs.insert(*dst);
            used.uses_cmem = true;
        }
        Instruction::FStr { src, .. } => {
            used.float_regs.insert(*src);
            used.uses_cmem = true;
        }

        // -- Float comparison (cross-file: dst -> int, lhs/rhs -> float) --
        Instruction::FEq { dst, lhs, rhs }
        | Instruction::FLt { dst, lhs, rhs }
        | Instruction::FGt { dst, lhs, rhs } => {
            used.int_regs.insert(*dst);
            used.float_regs.insert(*lhs);
            used.float_regs.insert(*rhs);
        }

        // -- Complex arithmetic --
        Instruction::ZAdd { dst, lhs, rhs }
        | Instruction::ZSub { dst, lhs, rhs }
        | Instruction::ZMul { dst, lhs, rhs }
        | Instruction::ZDiv { dst, lhs, rhs } => {
            used.complex_regs.insert(*dst);
            used.complex_regs.insert(*lhs);
            used.complex_regs.insert(*rhs);
        }

        Instruction::ZLdi { dst, .. } => {
            used.complex_regs.insert(*dst);
        }
        Instruction::ZLdm { dst, .. } => {
            used.complex_regs.insert(*dst);
            used.uses_cmem = true;
        }
        Instruction::ZStr { src, .. } => {
            used.complex_regs.insert(*src);
            used.uses_cmem = true;
        }

        // -- Type conversion (cross-file) --
        Instruction::CvtIF { dst_f, src_i } => {
            used.int_regs.insert(*src_i);
            used.float_regs.insert(*dst_f);
        }
        Instruction::CvtFI { dst_i, src_f } => {
            used.float_regs.insert(*src_f);
            used.int_regs.insert(*dst_i);
        }
        Instruction::CvtFZ { dst_z, src_f } => {
            used.float_regs.insert(*src_f);
            used.complex_regs.insert(*dst_z);
        }
        Instruction::CvtZF { dst_f, src_z } => {
            used.complex_regs.insert(*src_z);
            used.float_regs.insert(*dst_f);
        }

        // -- Control flow --
        Instruction::Jmp { .. } => {}
        Instruction::Jif { pred, .. } => {
            used.int_regs.insert(*pred);
        }
        Instruction::Call { .. } | Instruction::Ret | Instruction::Halt => {}

        // -- Quantum operations --
        Instruction::QPrep { dst, .. } => {
            used.quantum_regs.insert(*dst);
        }
        Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.int_regs.insert(*ctx0);
            used.int_regs.insert(*ctx1);
            used.kernel_ids.insert(*kernel);
        }
        Instruction::QObserve { dst_h, src_q } => {
            used.quantum_regs.insert(*src_q);
            used.hybrid_regs.insert(*dst_h);
        }
        Instruction::QLoad { dst_q, .. } => {
            used.quantum_regs.insert(*dst_q);
            used.uses_qmem = true;
        }
        Instruction::QStore { src_q, .. } => {
            used.quantum_regs.insert(*src_q);
            used.uses_qmem = true;
        }

        // -- Hybrid operations --
        Instruction::HFork | Instruction::HMerge => {}
        Instruction::HCExec { .. } => {}
        Instruction::HReduce { src, dst, func } => {
            used.hybrid_regs.insert(*src);
            if *func <= 5 {
                used.int_regs.insert(*dst);
            } else {
                used.float_regs.insert(*dst);
            }
        }

        // -- Pseudo-instructions --
        Instruction::Nop => {}
        Instruction::Label(name) => {
            used.labels.push(name.clone());
        }
    }
}

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

    // CMEM array
    if used.uses_cmem {
        lines.push("array[int[64], 65536] CMEM;".to_string());
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
        let kname = kernel_name(kid);
        lines.push(format!("gate {} q {{", kname));
        lines.push(format!("    // {} kernel logic", kname));
        lines.push("}".to_string());
    }

    lines.join("\n")
}

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
// Program emission (pipeline orchestrator)
// ---------------------------------------------------------------------------

/// Emit a full OpenQASM 3.0 program from a CQAM instruction sequence.
///
/// Orchestrates the three-phase pipeline:
///   1. Scan: collect used registers, kernels, labels
///   2. Declare: emit type declarations (standalone mode only)
///   3. Emit: translate each instruction to QASM body lines
///
/// In Standalone mode, the output includes:
///   - OPENQASM 3.0 header and include
///   - Register declarations
///   - Kernel gate definitions (when not expanding templates)
///   - Program body
///   - Footer comment
///
/// In Fragment mode, the output contains only the program body lines.
pub fn emit_qasm_program(
    program: &[Instruction],
    config: &EmitConfig,
) -> String {
    // 1. Scan
    let used = scan_registers(program);

    // 2. Build output
    let mut lines: Vec<String> = Vec::new();

    // 2a. Header (standalone only)
    if config.mode == EmitMode::Standalone {
        lines.push("OPENQASM 3.0;".to_string());
        lines.push("include \"stdgates.inc\";".to_string());
        lines.push(String::new());

        // Declarations
        let decls = emit_declarations(&used);
        if !decls.is_empty() {
            lines.push("// === CQAM Register Declarations ===".to_string());
            lines.push(decls);
            lines.push(String::new());
        }

        // Kernel gate stubs (only when not expanding templates inline)
        if !config.expand_templates {
            let stubs = emit_kernel_stubs(&used, config);
            if !stubs.is_empty() {
                lines.push("// === Kernel Gate Definitions ===".to_string());
                lines.push(stubs);
                lines.push(String::new());
            }
        }

        lines.push("// === Program Body ===".to_string());
    }

    // 2b. Body
    for instr in program {
        let qasm_lines = instr.to_qasm(config);
        lines.extend(qasm_lines);
    }

    // 2c. Footer (standalone only)
    if config.mode == EmitMode::Standalone {
        lines.push(String::new());
        lines.push("// === End CQAM Generated QASM ===".to_string());
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Helper: determine HReduce target register file
// ---------------------------------------------------------------------------

/// Returns "R" for int-producing reduction functions (func 0-5),
/// "F" for float-producing reduction functions (func 6-13).
fn hreduce_dst_file(func: u8) -> &'static str {
    if func <= 5 { "R" } else { "F" }
}
