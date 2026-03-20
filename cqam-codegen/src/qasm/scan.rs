//! Register-use scanning phase of the QASM code generator.
//!
//! Walks every instruction once, extracting read and write register indices
//! into the appropriate sets in `UsedRegisters`. Also detects CMEM/QMEM
//! usage and collects kernel IDs and label names.

use super::types::UsedRegisters;
use cqam_core::instruction::{Instruction, FileSel, ReduceFn, ObserveMode, ProcId};

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

        // -- Register-indirect memory --
        Instruction::ILdx { dst, addr_reg } => {
            used.int_regs.insert(*dst);
            used.int_regs.insert(*addr_reg);
            used.uses_cmem = true;
        }
        Instruction::IStrx { src, addr_reg } => {
            used.int_regs.insert(*src);
            used.int_regs.insert(*addr_reg);
            used.uses_cmem = true;
        }
        Instruction::FLdx { dst, addr_reg } => {
            used.float_regs.insert(*dst);
            used.int_regs.insert(*addr_reg);
            used.uses_cmem = true;
        }
        Instruction::FStrx { src, addr_reg } => {
            used.float_regs.insert(*src);
            used.int_regs.insert(*addr_reg);
            used.uses_cmem = true;
        }
        Instruction::ZLdx { dst, addr_reg } => {
            used.complex_regs.insert(*dst);
            used.int_regs.insert(*addr_reg);
            used.uses_cmem = true;
        }
        Instruction::ZStrx { src, addr_reg } => {
            used.complex_regs.insert(*src);
            used.int_regs.insert(*addr_reg);
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

        // -- Configuration query --
        Instruction::IQCfg { dst } => {
            used.int_regs.insert(*dst);
        }

        // -- Environment call: reads registers per calling convention --
        Instruction::Ecall { proc_id } => {
            match *proc_id {
                ProcId::PrintInt | ProcId::PrintChar => { used.int_regs.insert(0); }
                ProcId::PrintFloat => { used.float_regs.insert(0); }
                ProcId::PrintStr => { used.int_regs.insert(0); used.int_regs.insert(1); }
                ProcId::DumpRegs => {}
                ProcId::PrintHist => {
                    used.int_regs.insert(0);
                    used.int_regs.insert(1);
                    used.int_regs.insert(2);
                }
            }
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
        Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.float_regs.insert(*fctx0);
            used.float_regs.insert(*fctx1);
            used.kernel_ids.insert(*kernel);
        }
        Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.complex_regs.insert(*zctx0);
            used.complex_regs.insert(*zctx1);
            used.kernel_ids.insert(*kernel);
        }
        Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 } => {
            used.quantum_regs.insert(*src_q);
            used.hybrid_regs.insert(*dst_h);
            if matches!(mode, ObserveMode::Prob | ObserveMode::Amp | ObserveMode::Sample) { used.int_regs.insert(*ctx0); }
            if matches!(mode, ObserveMode::Amp) { used.int_regs.insert(*ctx1); }
        }
        Instruction::QLoad { dst_q, .. } => {
            used.quantum_regs.insert(*dst_q);
            used.uses_qmem = true;
        }
        Instruction::QStore { src_q, .. } => {
            used.quantum_regs.insert(*src_q);
            used.uses_qmem = true;
        }
        Instruction::QHadM { dst, src, .. }
        | Instruction::QFlip { dst, src, .. }
        | Instruction::QPhase { dst, src, .. } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
        }
        Instruction::QCnot { dst, src, ctrl_qubit_reg, tgt_qubit_reg } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.int_regs.insert(*ctrl_qubit_reg);
            used.int_regs.insert(*tgt_qubit_reg);
        }
        Instruction::QRot { dst, src, qubit_reg, axis: _, angle_freg } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.int_regs.insert(*qubit_reg);
            used.float_regs.insert(*angle_freg);
        }
        Instruction::QMeas { dst_r, src_q, qubit_reg } => {
            used.int_regs.insert(*dst_r);
            used.quantum_regs.insert(*src_q);
            used.int_regs.insert(*qubit_reg);
        }
        Instruction::QTensor { dst, src0, src1 } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src0);
            used.quantum_regs.insert(*src1);
        }
        Instruction::QCustom { dst, src, base_addr_reg, dim_reg } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.int_regs.insert(*base_addr_reg);
            used.int_regs.insert(*dim_reg);
            used.uses_cmem = true;
        }
        Instruction::QCz { dst, src, ctrl_qubit_reg, tgt_qubit_reg } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.int_regs.insert(*ctrl_qubit_reg);
            used.int_regs.insert(*tgt_qubit_reg);
        }
        Instruction::QSwap { dst, src, qubit_a_reg, qubit_b_reg } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.int_regs.insert(*qubit_a_reg);
            used.int_regs.insert(*qubit_b_reg);
        }
        Instruction::QPrepR { dst, dist_reg } => {
            used.quantum_regs.insert(*dst);
            used.int_regs.insert(*dist_reg);
        }
        Instruction::QEncode { dst, src_base, count, file_sel } => {
            used.quantum_regs.insert(*dst);
            let n = *count;
            match *file_sel {
                FileSel::RFile => {
                    for i in 0..n { used.int_regs.insert(*src_base + i); }
                }
                FileSel::FFile => {
                    for i in 0..n { used.float_regs.insert(*src_base + i); }
                }
                FileSel::ZFile => {
                    for i in 0..n { used.complex_regs.insert(*src_base + i); }
                }
            }
        }

        // -- Register-use scan for mixed-state and partial-trace instructions --
        Instruction::QMixed { dst, base_addr_reg, count_reg } => {
            used.quantum_regs.insert(*dst);
            used.int_regs.insert(*base_addr_reg);
            used.int_regs.insert(*count_reg);
            used.uses_cmem = true;
        }
        Instruction::QPrepN { dst, dist: _, qubit_count_reg } => {
            used.quantum_regs.insert(*dst);
            used.int_regs.insert(*qubit_count_reg);
        }
        Instruction::FSin { dst, src }
        | Instruction::FCos { dst, src }
        | Instruction::FSqrt { dst, src } => {
            used.float_regs.insert(*dst);
            used.float_regs.insert(*src);
        }
        Instruction::FAtan2 { dst, lhs, rhs } => {
            used.float_regs.insert(*dst);
            used.float_regs.insert(*lhs);
            used.float_regs.insert(*rhs);
        }
        Instruction::QPtrace { dst, src, num_qubits_a_reg } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.int_regs.insert(*num_qubits_a_reg);
        }
        Instruction::QReset { dst, src, qubit_reg } => {
            used.quantum_regs.insert(*dst);
            used.quantum_regs.insert(*src);
            used.int_regs.insert(*qubit_reg);
        }

        // -- Hybrid operations --
        Instruction::HFork | Instruction::HMerge => {}
        Instruction::JmpF { .. } => {}
        Instruction::HReduce { src, dst, func } => {
            used.hybrid_regs.insert(*src);
            match func.output_file() {
                cqam_core::instruction::ReduceOutput::IntReg => {
                    used.int_regs.insert(*dst);
                }
                cqam_core::instruction::ReduceOutput::FloatReg => {
                    used.float_regs.insert(*dst);
                }
                cqam_core::instruction::ReduceOutput::ComplexReg => {
                    used.complex_regs.insert(*dst);
                }
            }
            if *func == ReduceFn::Expect {
                // EXPECT also reads int reg for base_addr and writes float reg
                used.int_regs.insert(*dst);
                used.uses_cmem = true;
            }
        }

        // -- Pseudo-instructions --
        Instruction::Nop => {}
        Instruction::Label(name) => {
            used.labels.push(name.clone());
        }

        // -- Interrupt handling --
        Instruction::Reti => {}
        Instruction::SetIV { .. } => {}

        // -- Thread configuration --
        Instruction::ICCfg { dst } => {
            used.int_regs.insert(*dst);
        }
        Instruction::ITid { dst } => {
            used.int_regs.insert(*dst);
        }
        Instruction::HAtmS => {}
        Instruction::HAtmE => {}

        // -- Product state preparation --
        Instruction::QPreps { dst, .. } => {
            used.quantum_regs.insert(*dst);
        }
        Instruction::QPrepsm { dst, r_base, r_count } => {
            used.quantum_regs.insert(*dst);
            used.int_regs.insert(*r_base);
            used.int_regs.insert(*r_count);
            used.uses_cmem = true;
        }
    }
}
