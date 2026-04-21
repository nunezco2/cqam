//! Instruction display formatting for the CODE pane.
//!
//! Provides formatting functions to render `Instruction` values as
//! human-readable assembly mnemonics for the debugger's CODE pane.
#![allow(dead_code)]

use cqam_core::instruction::Instruction;

/// Format an instruction for display in the CODE pane.
///
/// Returns a string like "IADD R2, R0, R1" or "QPREP Q0, uniform".
/// Uses the same mnemonic conventions as the CQAM assembler.
pub fn format_instruction(instr: &Instruction) -> String {
    match instr {
        Instruction::Nop => "NOP".to_string(),
        Instruction::Label(name) => format!("{}:", name),

        // Integer arithmetic
        Instruction::IAdd { dst, lhs, rhs } => format!("IADD R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::ISub { dst, lhs, rhs } => format!("ISUB R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IMul { dst, lhs, rhs } => format!("IMUL R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IDiv { dst, lhs, rhs } => format!("IDIV R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IMod { dst, lhs, rhs } => format!("IMOD R{}, R{}, R{}", dst, lhs, rhs),

        // Integer bitwise
        Instruction::IAnd { dst, lhs, rhs } => format!("IAND R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IOr { dst, lhs, rhs } => format!("IOR R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IXor { dst, lhs, rhs } => format!("IXOR R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::INot { dst, src } => format!("INOT R{}, R{}", dst, src),
        Instruction::IInc { dst, src } => format!("IINC R{}, R{}", dst, src),
        Instruction::IDec { dst, src } => format!("IDEC R{}, R{}", dst, src),
        Instruction::IMov { dst, src } => format!("IMOV R{}, R{}", dst, src),
        Instruction::IShl { dst, src, amt } => format!("ISHL R{}, R{}, {}", dst, src, amt),
        Instruction::IShr { dst, src, amt } => format!("ISHR R{}, R{}, {}", dst, src, amt),

        // Integer memory
        Instruction::ILdi { dst, imm } => format!("ILDI R{}, {}", dst, imm),
        Instruction::ILdm { dst, addr } => format!("ILDM R{}, 0x{:04X}", dst, addr),
        Instruction::IStr { src, addr } => format!("ISTR R{}, 0x{:04X}", src, addr),

        // Integer comparison
        Instruction::IEq { dst, lhs, rhs } => format!("IEQ R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::ILt { dst, lhs, rhs } => format!("ILT R{}, R{}, R{}", dst, lhs, rhs),
        Instruction::IGt { dst, lhs, rhs } => format!("IGT R{}, R{}, R{}", dst, lhs, rhs),

        // Float arithmetic
        Instruction::FAdd { dst, lhs, rhs } => format!("FADD F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FSub { dst, lhs, rhs } => format!("FSUB F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FMul { dst, lhs, rhs } => format!("FMUL F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FDiv { dst, lhs, rhs } => format!("FDIV F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FLdi { dst, imm } => format!("FLDI F{}, {}", dst, imm),
        Instruction::FLdm { dst, addr } => format!("FLDM F{}, 0x{:04X}", dst, addr),
        Instruction::FMov { dst, src } => format!("FMOV F{}, F{}", dst, src),
        Instruction::FStr { src, addr } => format!("FSTR F{}, 0x{:04X}", src, addr),

        // Float comparison
        Instruction::FEq { dst, lhs, rhs } => format!("FEQ R{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FLt { dst, lhs, rhs } => format!("FLT R{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FGt { dst, lhs, rhs } => format!("FGT R{}, F{}, F{}", dst, lhs, rhs),

        // Float trig / math
        Instruction::FSin { dst, src } => format!("FSIN F{}, F{}", dst, src),
        Instruction::FCos { dst, src } => format!("FCOS F{}, F{}", dst, src),
        Instruction::FAtan2 { dst, lhs, rhs } => format!("FATAN2 F{}, F{}, F{}", dst, lhs, rhs),
        Instruction::FSqrt { dst, src } => format!("FSQRT F{}, F{}", dst, src),

        // Complex arithmetic
        Instruction::ZAdd { dst, lhs, rhs } => format!("ZADD Z{}, Z{}, Z{}", dst, lhs, rhs),
        Instruction::ZSub { dst, lhs, rhs } => format!("ZSUB Z{}, Z{}, Z{}", dst, lhs, rhs),
        Instruction::ZMul { dst, lhs, rhs } => format!("ZMUL Z{}, Z{}, Z{}", dst, lhs, rhs),
        Instruction::ZDiv { dst, lhs, rhs } => format!("ZDIV Z{}, Z{}, Z{}", dst, lhs, rhs),
        Instruction::ZMov { dst, src } => format!("ZMOV Z{}, Z{}", dst, src),
        Instruction::ZLdi { dst, imm_re, imm_im } => {
            format!("ZLDI Z{}, ({}, {})", dst, imm_re, imm_im)
        }
        Instruction::ZLdm { dst, addr } => format!("ZLDM Z{}, 0x{:04X}", dst, addr),
        Instruction::ZStr { src, addr } => format!("ZSTR Z{}, 0x{:04X}", src, addr),

        // Register-indirect memory
        Instruction::ILdx { dst, addr_reg } => format!("ILDX R{}, R{}", dst, addr_reg),
        Instruction::IStrx { src, addr_reg } => format!("ISTRX R{}, R{}", src, addr_reg),
        Instruction::FLdx { dst, addr_reg } => format!("FLDX F{}, R{}", dst, addr_reg),
        Instruction::FStrx { src, addr_reg } => format!("FSTRX F{}, R{}", src, addr_reg),
        Instruction::ZLdx { dst, addr_reg } => format!("ZLDX Z{}, R{}", dst, addr_reg),
        Instruction::ZStrx { src, addr_reg } => format!("ZSTRX Z{}, R{}", src, addr_reg),

        // Type conversion
        Instruction::CvtIF { dst_f, src_i } => format!("CVTIF F{}, R{}", dst_f, src_i),
        Instruction::CvtFI { dst_i, src_f } => format!("CVTFI R{}, F{}", dst_i, src_f),
        Instruction::CvtFZ { dst_z, src_f } => format!("CVTFZ Z{}, F{}", dst_z, src_f),
        Instruction::CvtZF { dst_f, src_z } => format!("CVTZF F{}, Z{}", dst_f, src_z),

        // Configuration query
        Instruction::IQCfg { dst } => format!("IQCFG R{}", dst),
        Instruction::ICCfg { dst } => format!("ICCFG R{}", dst),
        Instruction::ITid { dst } => format!("ITID R{}", dst),

        // ECALL
        Instruction::Ecall { proc_id } => {
            format!("ECALL {}", proc_id.name())
        }

        // Control flow
        Instruction::Jmp { target } => format!("JMP {}", target),
        Instruction::Jif { pred, target } => format!("JIF R{}, {}", pred, target),
        Instruction::Call { target } => format!("CALL {}", target),
        Instruction::Ret => "RET".to_string(),
        Instruction::Halt => "HALT".to_string(),

        // Quantum operations
        Instruction::QPrep { dst, dist } => {
            format!("QPREP Q{}, {}", dst, dist.name())
        }
        Instruction::QPrepR { dst, dist_reg } => format!("QPREPR Q{}, R{}", dst, dist_reg),
        Instruction::QPrepN { dst, dist, qubit_count_reg } => {
            format!("QPREPN Q{}, {}, R{}", dst, dist.name(), qubit_count_reg)
        }
        Instruction::QKernel { dst, src, kernel, ctx0, ctx1 } => {
            format!(
                "QKERNEL {}, Q{}, Q{}, R{}, R{}",
                kernel.mnemonic(), dst, src, ctx0, ctx1
            )
        }
        Instruction::QKernelF { dst, src, kernel, fctx0, fctx1 } => {
            format!(
                "QKERNELF {}, Q{}, Q{}, F{}, F{}",
                kernel.mnemonic(), dst, src, fctx0, fctx1
            )
        }
        Instruction::QKernelZ { dst, src, kernel, zctx0, zctx1 } => {
            format!(
                "QKERNELZ {}, Q{}, Q{}, Z{}, Z{}",
                kernel.mnemonic(), dst, src, zctx0, zctx1
            )
        }
        Instruction::QObserve { dst_h, src_q, mode, ctx0, ctx1 } => {
            format!(
                "QOBSERVE H{}, Q{}, {}, R{}, R{}",
                dst_h, src_q, mode.name(), ctx0, ctx1
            )
        }
        Instruction::QLoad { dst_q, addr } => format!("QLOAD Q{}, {}", dst_q, addr),
        Instruction::QStore { src_q, addr } => format!("QSTORE Q{}, {}", src_q, addr),
        Instruction::QEncode { dst, src_base, count, file_sel } => {
            format!(
                "QENCODE Q{}, {}[{}..+{}]",
                dst, file_sel.name(), src_base, count
            )
        }
        Instruction::QHadM { dst, src, mask_reg } => {
            format!("QHADM Q{}, Q{}, R{}", dst, src, mask_reg)
        }
        Instruction::QFlip { dst, src, mask_reg } => {
            format!("QFLIP Q{}, Q{}, R{}", dst, src, mask_reg)
        }
        Instruction::QPhase { dst, src, mask_reg } => {
            format!("QPHASE Q{}, Q{}, R{}", dst, src, mask_reg)
        }
        Instruction::QCnot { dst, src, ctrl_qubit_reg, tgt_qubit_reg } => {
            format!("QCNOT Q{}, Q{}, R{}, R{}", dst, src, ctrl_qubit_reg, tgt_qubit_reg)
        }
        Instruction::QCz { dst, src, ctrl_qubit_reg, tgt_qubit_reg } => {
            format!("QCZ Q{}, Q{}, R{}, R{}", dst, src, ctrl_qubit_reg, tgt_qubit_reg)
        }
        Instruction::QSwap { dst, src, qubit_a_reg, qubit_b_reg } => {
            format!("QSWAP Q{}, Q{}, R{}, R{}", dst, src, qubit_a_reg, qubit_b_reg)
        }
        Instruction::QRot { dst, src, qubit_reg, axis, angle_freg } => {
            format!(
                "QROT Q{}, Q{}, R{}, {}, F{}",
                dst, src, qubit_reg, axis.name(), angle_freg
            )
        }
        Instruction::QMeas { dst_r, src_q, qubit_reg } => {
            format!("QMEAS R{}, Q{}, R{}", dst_r, src_q, qubit_reg)
        }
        Instruction::QTensor { dst, src0, src1 } => {
            format!("QTENSOR Q{}, Q{}, Q{}", dst, src0, src1)
        }
        Instruction::QCustom { dst, src, base_addr_reg, dim_reg } => {
            format!("QCUSTOM Q{}, Q{}, R{}, R{}", dst, src, base_addr_reg, dim_reg)
        }
        Instruction::QMixed { dst, base_addr_reg, count_reg } => {
            format!("QMIXED Q{}, R{}, R{}", dst, base_addr_reg, count_reg)
        }
        Instruction::QPtrace { dst, src, num_qubits_a_reg } => {
            format!("QPTRACE Q{}, Q{}, R{}", dst, src, num_qubits_a_reg)
        }
        Instruction::QReset { dst, src, qubit_reg } => {
            format!("QRESET Q{}, Q{}, R{}", dst, src, qubit_reg)
        }
        Instruction::QPreps { dst, z_start, count } => {
            format!("QPREPS Q{}, Z{}, {}", dst, z_start, count)
        }
        Instruction::QPrepsm { dst, r_base, r_count } => {
            format!("QPREPSM Q{}, R{}, R{}", dst, r_base, r_count)
        }

        // Hybrid
        Instruction::HFork => "HFORK".to_string(),
        Instruction::HMerge => "HMERGE".to_string(),
        Instruction::HAtmS => "HATMS".to_string(),
        Instruction::HAtmE => "HATME".to_string(),
        Instruction::JmpF { flag, target } => {
            format!("JMPF {}, {}", flag.mnemonic(), target)
        }
        Instruction::HReduce { src, dst, func } => {
            format!("HREDUCE {}, H{}, R{}", func.mnemonic(), src, dst)
        }

        // Interrupt handling
        Instruction::Reti => "RETI".to_string(),
        Instruction::SetIV { trap_id, target } => {
            format!("SETIV {}, {}", trap_id.name(), target)
        }
    }
}

/// Classify an instruction into a high-level class for breakpoint matching.
pub fn instruction_class(instr: &Instruction) -> Option<&'static str> {
    match instr {
        // Quantum class
        Instruction::QPrep { .. }
        | Instruction::QPrepR { .. }
        | Instruction::QPrepN { .. }
        | Instruction::QKernel { .. }
        | Instruction::QKernelF { .. }
        | Instruction::QKernelZ { .. }
        | Instruction::QObserve { .. }
        | Instruction::QLoad { .. }
        | Instruction::QStore { .. }
        | Instruction::QEncode { .. }
        | Instruction::QHadM { .. }
        | Instruction::QFlip { .. }
        | Instruction::QPhase { .. }
        | Instruction::QCnot { .. }
        | Instruction::QCz { .. }
        | Instruction::QSwap { .. }
        | Instruction::QRot { .. }
        | Instruction::QMeas { .. }
        | Instruction::QTensor { .. }
        | Instruction::QCustom { .. }
        | Instruction::QMixed { .. }
        | Instruction::QPtrace { .. }
        | Instruction::QReset { .. }
        | Instruction::QPreps { .. }
        | Instruction::QPrepsm { .. } => Some("quantum"),

        // Hybrid class
        Instruction::HFork
        | Instruction::HMerge
        | Instruction::HAtmS
        | Instruction::HAtmE
        | Instruction::JmpF { .. }
        | Instruction::HReduce { .. } => Some("hybrid"),

        // Branch class
        Instruction::Jmp { .. }
        | Instruction::Jif { .. }
        | Instruction::Call { .. }
        | Instruction::Ret
        | Instruction::Halt => Some("branch"),

        // Memory class
        Instruction::ILdm { .. }
        | Instruction::IStr { .. }
        | Instruction::FLdm { .. }
        | Instruction::FStr { .. }
        | Instruction::ZLdm { .. }
        | Instruction::ZStr { .. }
        | Instruction::ILdx { .. }
        | Instruction::IStrx { .. }
        | Instruction::FLdx { .. }
        | Instruction::FStrx { .. }
        | Instruction::ZLdx { .. }
        | Instruction::ZStrx { .. } => Some("memory"),

        // Ecall class
        Instruction::Ecall { .. } => Some("ecall"),

        // Float class
        Instruction::FAdd { .. }
        | Instruction::FSub { .. }
        | Instruction::FMul { .. }
        | Instruction::FDiv { .. }
        | Instruction::FLdi { .. }
        | Instruction::FEq { .. }
        | Instruction::FLt { .. }
        | Instruction::FGt { .. }
        | Instruction::FMov { .. }
        | Instruction::FSin { .. }
        | Instruction::FCos { .. }
        | Instruction::FAtan2 { .. }
        | Instruction::FSqrt { .. } => Some("float"),

        // Complex class
        Instruction::ZAdd { .. }
        | Instruction::ZSub { .. }
        | Instruction::ZMul { .. }
        | Instruction::ZDiv { .. }
        | Instruction::ZMov { .. }
        | Instruction::ZLdi { .. } => Some("complex"),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_nop() {
        assert_eq!(format_instruction(&Instruction::Nop), "NOP");
    }

    #[test]
    fn test_format_iadd() {
        let instr = Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 };
        assert_eq!(format_instruction(&instr), "IADD R2, R0, R1");
    }

    #[test]
    fn test_format_halt() {
        assert_eq!(format_instruction(&Instruction::Halt), "HALT");
    }

    #[test]
    fn test_format_label() {
        let instr = Instruction::Label("loop".to_string());
        assert_eq!(format_instruction(&instr), "loop:");
    }

    #[test]
    fn test_instruction_class_quantum() {
        let instr = Instruction::QPrep { dst: 0, dist: cqam_core::instruction::DistId::Uniform };
        assert_eq!(instruction_class(&instr), Some("quantum"));
    }

    #[test]
    fn test_instruction_class_branch() {
        assert_eq!(instruction_class(&Instruction::Halt), Some("branch"));
    }

    #[test]
    fn test_instruction_class_ecall() {
        let instr = Instruction::Ecall { proc_id: cqam_core::instruction::ProcId::PrintInt };
        assert_eq!(instruction_class(&instr), Some("ecall"));
    }
}
