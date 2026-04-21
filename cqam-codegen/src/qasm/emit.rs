//! Instruction-level QASM emission and program-level pipeline orchestrator.
//!
//! Contains the `QasmFormat` trait implementation for `Instruction` (the giant
//! `to_qasm()` match) and the `emit_qasm_program()` entry point that
//! orchestrates the scan-declare-emit pipeline.

use super::types::{EmitMode, EmitConfig, QasmFormat};
use super::scan::scan_registers;
use super::declare::{emit_declarations, emit_kernel_stubs};
use super::helpers::{load_template, emit_comparison, hreduce_dst_file};
use cqam_core::instruction::{Instruction, RotAxis, ObserveMode};

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
                vec![format!("R{} = R{} ^ -1;", dst, src)]
            }
            Instruction::IInc { dst, src } => {
                vec![format!("R{} = R{} + 1;", dst, src)]
            }
            Instruction::IDec { dst, src } => {
                vec![format!("R{} = R{} - 1;", dst, src)]
            }
            Instruction::IMov { dst, src } => {
                vec![format!("R{} = R{};", dst, src)]
            }
            Instruction::FMov { dst, src } => {
                vec![format!("F{} = F{};", dst, src)]
            }
            Instruction::ZMov { dst, src } => {
                vec![
                    format!("Z{}_re = Z{}_re;", dst, src),
                    format!("Z{}_im = Z{}_im;", dst, src),
                ]
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
                vec![format!("// @cqam.ldm R{}, CMEM[{}]", dst, addr)]
            }
            Instruction::IStr { src, addr } => {
                vec![format!("// @cqam.str CMEM[{}], R{}", addr, src)]
            }

            // -- Integer comparison ------------------------------------------

            Instruction::IEq { dst, lhs, rhs } => {
                emit_comparison(*dst, "R", *lhs, "==", "R", *rhs)
            }
            Instruction::ILt { dst, lhs, rhs } => {
                emit_comparison(*dst, "R", *lhs, "<", "R", *rhs)
            }
            Instruction::IGt { dst, lhs, rhs } => {
                emit_comparison(*dst, "R", *lhs, ">", "R", *rhs)
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
                vec![format!("// @cqam.ldm F{}, CMEM[{}]", dst, addr)]
            }
            Instruction::FStr { src, addr } => {
                vec![format!("// @cqam.str CMEM[{}], F{}", addr, src)]
            }

            // -- Float comparison (result to int register) -------------------

            Instruction::FEq { dst, lhs, rhs } => {
                emit_comparison(*dst, "F", *lhs, "==", "F", *rhs)
            }
            Instruction::FLt { dst, lhs, rhs } => {
                emit_comparison(*dst, "F", *lhs, "<", "F", *rhs)
            }
            Instruction::FGt { dst, lhs, rhs } => {
                emit_comparison(*dst, "F", *lhs, ">", "F", *rhs)
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
                        "float[64] _tmp_re = Z{}_re * Z{}_re - Z{}_im * Z{}_im;",
                        lhs, rhs, lhs, rhs
                    ),
                    format!(
                        "float[64] _tmp_im = Z{}_re * Z{}_im + Z{}_im * Z{}_re;",
                        lhs, rhs, lhs, rhs
                    ),
                    format!("Z{}_re = _tmp_re;", dst),
                    format!("Z{}_im = _tmp_im;", dst),
                ]
            }
            Instruction::ZDiv { dst, lhs, rhs } => {
                vec![
                    format!("// ZDIV: Z{} = Z{} / Z{}", dst, lhs, rhs),
                    format!(
                        "float[64] _denom = Z{}_re * Z{}_re + Z{}_im * Z{}_im;",
                        rhs, rhs, rhs, rhs
                    ),
                    format!(
                        "float[64] _tmp_re = (Z{}_re * Z{}_re + Z{}_im * Z{}_im) / _denom;",
                        lhs, rhs, lhs, rhs
                    ),
                    format!(
                        "float[64] _tmp_im = (Z{}_im * Z{}_re - Z{}_re * Z{}_im) / _denom;",
                        lhs, rhs, lhs, rhs
                    ),
                    format!("Z{}_re = _tmp_re;", dst),
                    format!("Z{}_im = _tmp_im;", dst),
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
                    format!("// @cqam.ldm Z{}_re, CMEM[{}]", dst, addr),
                    format!("// @cqam.ldm Z{}_im, CMEM[{}]", dst, addr + 1),
                ]
            }
            Instruction::ZStr { src, addr } => {
                vec![
                    format!("// @cqam.str CMEM[{}], Z{}_re", addr, src),
                    format!("// @cqam.str CMEM[{}], Z{}_im", addr + 1, src),
                ]
            }

            // -- Register-indirect memory ------------------------------------

            Instruction::ILdx { dst, addr_reg } => {
                vec![format!("// @cqam.ldx R{}, CMEM[R{}]", dst, addr_reg)]
            }
            Instruction::IStrx { src, addr_reg } => {
                vec![format!("// @cqam.strx CMEM[R{}], R{}", addr_reg, src)]
            }
            Instruction::FLdx { dst, addr_reg } => {
                vec![format!("// @cqam.ldx F{}, CMEM[R{}]", dst, addr_reg)]
            }
            Instruction::FStrx { src, addr_reg } => {
                vec![format!("// @cqam.strx CMEM[R{}], F{}", addr_reg, src)]
            }
            Instruction::ZLdx { dst, addr_reg } => {
                vec![
                    format!("// @cqam.ldx Z{}_re, CMEM[R{}]", dst, addr_reg),
                    format!("// @cqam.ldx Z{}_im, CMEM[R{} + 1]", dst, addr_reg),
                ]
            }
            Instruction::ZStrx { src, addr_reg } => {
                vec![
                    format!("// @cqam.strx CMEM[R{}], Z{}_re", addr_reg, src),
                    format!("// @cqam.strx CMEM[R{} + 1], Z{}_im", addr_reg, src),
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

            // -- Configuration query -----------------------------------------
            Instruction::IQCfg { dst } => {
                vec![format!("// R{} = IQCFG (qubit count from pragma)", dst)]
            }

            Instruction::Ecall { proc_id } => {
                vec![format!("// ECALL {} (host I/O)", proc_id.name())]
            }

            // -- Control flow ------------------------------------------------

            Instruction::Jmp { target } => {
                vec![format!("// @cqam.jmp {}", target)]
            }
            Instruction::Jif { pred, target } => {
                vec![format!("if (bool(R{})) {{ }} // @cqam.branch {}", pred, target)]
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
                    format!("// QPrep: initialize q{} with distribution '{}'", dst, dist.name()),
                ]
            }
            Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
                let kname = kernel.name();
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
            Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 } => {
                let kname = kernel.name();
                vec![format!(
                    "// QKernelF: q{} = {}(q{}, ctx=[F{}, F{}])",
                    dst, kname, src, fctx0, fctx1
                )]
            }
            Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 } => {
                let kname = kernel.name();
                vec![format!(
                    "// QKernelZ: q{} = {}(q{}, ctx=[Z{}, Z{}])",
                    dst, kname, src, zctx0, zctx1
                )]
            }
            Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1: _ } => {
                match *mode {
                    ObserveMode::Dist => vec![format!("H{} = measure q{};", dst_h, src_q)],
                    ObserveMode::Prob => vec![format!("// @cqam.observe_prob H{} = prob(q{}, R{});", dst_h, src_q, ctx0)],
                    ObserveMode::Sample => vec![format!("// @cqam.observe H{} = observe(q{}, mode={});", dst_h, src_q, mode)],
                }
            }
            Instruction::QLoad { dst_q, addr } => {
                vec![format!("// QLOAD q{} from QMEM[{}] [no QASM equivalent]", dst_q, addr)]
            }
            Instruction::QStore { src_q, addr } => {
                vec![format!("// QSTORE q{} to QMEM[{}] [no QASM equivalent]", src_q, addr)]
            }
            Instruction::QHadM { dst, src: _, mask_reg } => {
                vec![format!(
                    "// QHADM Q{} mask=R{}: h on qubits where R{} bits are set",
                    dst, mask_reg, mask_reg
                )]
            }
            Instruction::QFlip { dst, src: _, mask_reg } => {
                vec![format!(
                    "// QFLIP Q{} mask=R{}: x on qubits where R{} bits are set",
                    dst, mask_reg, mask_reg
                )]
            }
            Instruction::QPhase { dst, src: _, mask_reg } => {
                vec![format!(
                    "// QPHASE Q{} mask=R{}: z on qubits where R{} bits are set",
                    dst, mask_reg, mask_reg
                )]
            }
            Instruction::QCnot { dst, src: _, ctrl_qubit_reg, tgt_qubit_reg } => {
                vec![format!(
                    "cx q{}[R{}], q{}[R{}]; // QCNOT Q{} ctrl=R{} tgt=R{}",
                    dst, ctrl_qubit_reg, dst, tgt_qubit_reg, dst, ctrl_qubit_reg, tgt_qubit_reg
                )]
            }
            Instruction::QRot { dst, src: _, qubit_reg, axis, angle_freg } => {
                let gate_name = match *axis {
                    RotAxis::X => "rx",
                    RotAxis::Y => "ry",
                    RotAxis::Z => "rz",
                };
                vec![format!(
                    "{}(F{}) q{}[R{}]; // QROT axis={} angle=F{}",
                    gate_name, angle_freg, dst, qubit_reg,
                    axis.name(), angle_freg
                )]
            }
            Instruction::QMeas { dst_r, src_q, qubit_reg } => {
                vec![format!(
                    "R{} = measure q{}[R{}]; // QMEAS partial measurement",
                    dst_r, src_q, qubit_reg
                )]
            }
            Instruction::QTensor { dst, src0, src1 } => {
                vec![format!(
                    "// @cqam.qtensor Q{} = Q{} tensor Q{} [no QASM equivalent]",
                    dst, src0, src1
                )]
            }
            Instruction::QCustom { dst, src, base_addr_reg, dim_reg } => {
                vec![format!(
                    "// @cqam.qcustom Q{} = custom_unitary(Q{}, CMEM[R{}], dim=R{})",
                    dst, src, base_addr_reg, dim_reg
                )]
            }
            Instruction::QCz { dst, src: _, ctrl_qubit_reg, tgt_qubit_reg } => {
                vec![format!(
                    "cz q{}[R{}], q{}[R{}]; // QCZ",
                    dst, ctrl_qubit_reg, dst, tgt_qubit_reg
                )]
            }
            Instruction::QSwap { dst, src: _, qubit_a_reg, qubit_b_reg } => {
                vec![format!(
                    "swap q{}[R{}], q{}[R{}]; // QSWAP",
                    dst, qubit_a_reg, dst, qubit_b_reg
                )]
            }
            Instruction::QPrepR { dst, dist_reg } => {
                vec![format!("// @cqam.qprepr Q{} = prep(R[{}]);", dst, dist_reg)]
            }
            Instruction::QEncode { dst, src_base, count, file_sel } => {
                let file = file_sel.name();
                vec![format!(
                    "// @cqam.qencode Q{} = encode({}, base={}, count={});",
                    dst, file, src_base, count
                )]
            }

            // -- QASM emission for mixed-state and partial-trace instructions --

            Instruction::QMixed { dst, base_addr_reg, count_reg } => {
                vec![format!(
                    "// @cqam.qmixed Q{} = mixed_state(CMEM[R{}], count=R{}) [no QASM equivalent]",
                    dst, base_addr_reg, count_reg
                )]
            }
            Instruction::QPrepN { dst, dist, qubit_count_reg } => {
                vec![format!(
                    "// @cqam.qprepn Q{} = prep(dist={}, qubits=R{})",
                    dst, dist.name(), qubit_count_reg
                )]
            }
            Instruction::FSin { dst, src } => {
                vec![format!("F{} = sin(F{});", dst, src)]
            }
            Instruction::FCos { dst, src } => {
                vec![format!("F{} = cos(F{});", dst, src)]
            }
            Instruction::FAtan2 { dst, lhs, rhs } => {
                vec![format!("F{} = arctan(F{}, F{});", dst, lhs, rhs)]
            }
            Instruction::FSqrt { dst, src } => {
                vec![format!("F{} = sqrt(F{});", dst, src)]
            }
            Instruction::QPtrace { dst, src, num_qubits_a_reg } => {
                vec![format!(
                    "// @cqam.qptrace Q{} = Tr_B(Q{}, qubits_a=R{}) [no QASM equivalent]",
                    dst, src, num_qubits_a_reg
                )]
            }
            Instruction::QReset { dst, src: _, qubit_reg } => {
                vec![format!(
                    "reset q{}[R{}]; // QRESET qubit R{}",
                    dst, qubit_reg, qubit_reg
                )]
            }

            // -- Hybrid operations (CQAM-specific annotations) ---------------

            Instruction::HFork => {
                vec!["// @cqam.hfork: begin parallel execution region".to_string()]
            }
            Instruction::HMerge => {
                vec!["// @cqam.hmerge: end parallel execution region, merge results".to_string()]
            }
            Instruction::JmpF { flag, target } => {
                vec![format!(
                    "// @cqam.jmpf: if PSW.{} goto {}",
                    flag.mnemonic(), target
                )]
            }
            Instruction::HReduce { src, dst, func } => {
                let dst_file = hreduce_dst_file(*func);
                let fname = func.name();
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
                vec![format!("// @cqam.label {}", name)]
            }

            // -- Interrupt handling (no QASM equivalent) ---------------------

            Instruction::Reti => {
                vec!["// @cqam.reti".to_string()]
            }
            Instruction::SetIV { trap_id, target } => {
                vec![format!("// @cqam.setiv trap={}, target={}", trap_id, target)]
            }

            // -- Thread configuration (no QASM equivalent) --------------------

            Instruction::ICCfg { dst } => {
                vec![format!("// @cqam.iccfg R{}", dst)]
            }
            Instruction::ITid { dst } => {
                vec![format!("// @cqam.itid R{}", dst)]
            }
            Instruction::HAtmS => {
                vec!["// @cqam.hatms".to_string()]
            }
            Instruction::HAtmE => {
                vec!["// @cqam.hatme".to_string()]
            }

            // -- Product state preparation -----------------------------------

            Instruction::QPreps { dst, z_start, count } => {
                vec![format!(
                    "// @cqam.qpreps Q{} from Z{} count {}",
                    dst, z_start, count
                )]
            }
            Instruction::QPrepsm { dst, r_base, r_count } => {
                vec![format!(
                    "// @cqam.qprepsm Q{} base R{} count R{}",
                    dst, r_base, r_count
                )]
            }
            Instruction::QXch { qa, qb } => {
                vec![format!(
                    "// @cqam.qxch Q{} <-> Q{} [handle swap, no gates]",
                    qa, qb
                )]
            }
        }
    }
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
