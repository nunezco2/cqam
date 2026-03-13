//! Binary opcode encoding and decoding for the CQAM ISA.
//!
//! Every `Instruction` variant maps to a unique 8-bit opcode. The remaining
//! 24 bits carry operands in one of several fixed formats (N, RR, RRR, RI,
//! RA, J, JR, Q, QP, QO, QS, HR, ZI, L). See `reference/opcodes.md` for
//! the complete bit-level layout of each format.

pub mod constants;
pub mod decode;
pub mod encode;

pub use constants::op;
pub use decode::{decode, decode_with_debug, mnemonic};
pub use encode::{encode, encode_label};

// =============================================================================
// Tests (in-module unit tests)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::instruction::*;

    /// Helper: build a label map for testing.
    fn test_labels() -> HashMap<String, u32> {
        let mut m = HashMap::new();
        m.insert("loop".to_string(), 10);
        m.insert("end".to_string(), 42);
        m.insert("start".to_string(), 0);
        m
    }

    #[test]
    fn test_roundtrip_nop() {
        let labels = HashMap::new();
        let word = encode(&Instruction::Nop, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, Instruction::Nop);
    }

    #[test]
    fn test_roundtrip_iadd() {
        let labels = HashMap::new();
        let instr = Instruction::IAdd { dst: 2, lhs: 3, rhs: 4 };
        let word = encode(&instr, &labels).unwrap();
        assert_eq!(word, 0x01234000);
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, instr);
    }

    #[test]
    fn test_roundtrip_ildi_negative() {
        let labels = HashMap::new();
        let instr = Instruction::ILdi { dst: 5, imm: -100 };
        let word = encode(&instr, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, instr);
    }

    #[test]
    fn test_roundtrip_jmp() {
        let labels = test_labels();
        let instr = Instruction::Jmp { target: "end".to_string() };
        let word = encode(&instr, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, Instruction::Jmp { target: "@42".to_string() });
    }

    #[test]
    fn test_roundtrip_qkernel() {
        let labels = HashMap::new();
        let instr = Instruction::QKernel {
            dst: 1, src: 0, kernel: KernelId::Fourier, ctx0: 3, ctx1: 4,
        };
        let word = encode(&instr, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, instr);
    }

    #[test]
    fn test_roundtrip_zldi() {
        let labels = HashMap::new();
        let instr = Instruction::ZLdi { dst: 3, imm_re: 5, imm_im: -2 };
        let word = encode(&instr, &labels).unwrap();
        let decoded = decode(word).unwrap();
        assert_eq!(decoded, instr);
    }

    #[test]
    fn test_encode_unresolved_label() {
        let labels = HashMap::new();
        let instr = Instruction::Jmp { target: "nonexistent".to_string() };
        let result = encode(&instr, &labels);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_opcode() {
        let word: u32 = 0xFF_000000;
        let result = decode(word);
        assert!(result.is_err());
    }

    #[test]
    fn test_mnemonic_lookup() {
        assert_eq!(mnemonic(op::IADD), Some("IADD"));
        assert_eq!(mnemonic(op::QKERNEL), Some("QKERNEL"));
        assert_eq!(mnemonic(0xFE), None);
    }

    #[test]
    fn test_reg4_overflow() {
        let labels = HashMap::new();
        let instr = Instruction::IAdd { dst: 16, lhs: 0, rhs: 0 };
        let result = encode(&instr, &labels);
        assert!(result.is_err());
    }

    #[test]
    fn test_shift_amount_overflow() {
        let labels = HashMap::new();
        let instr = Instruction::IShl { dst: 0, src: 1, amt: 64 };
        let result = encode(&instr, &labels);
        assert!(result.is_err());
    }
}
