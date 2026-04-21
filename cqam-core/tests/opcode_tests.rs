//! Integration tests for opcode encoding and decoding.
//!
//! Covers round-trip encode->decode for every instruction variant, error
//! cases, label resolution, and boundary conditions (max immediate, max
//! address, field overflow).

use std::collections::HashMap;

use cqam_core::instruction::*;
use cqam_core::opcode::{decode, decode_with_debug, encode, mnemonic, op};

// =============================================================================
// Helper functions
// =============================================================================

/// Build a standard label map for control-flow tests.
fn test_labels() -> HashMap<String, u32> {
    let mut m = HashMap::new();
    m.insert("loop".to_string(), 10);
    m.insert("end".to_string(), 42);
    m.insert("start".to_string(), 0);
    m.insert("far".to_string(), 0x00FF_FFFF); // max 24-bit address
    m.insert("near_max".to_string(), 0xFFFF); // max 16-bit address
    m
}

/// Encode-then-decode round-trip for instructions that don't involve labels.
fn roundtrip(instr: &Instruction) -> Instruction {
    let labels = HashMap::new();
    let word = encode(instr, &labels).expect("encode failed");
    decode(word).expect("decode failed")
}

/// Encode-then-decode round-trip for instructions that require labels.
fn roundtrip_with_labels(instr: &Instruction, labels: &HashMap<String, u32>) -> Instruction {
    let word = encode(instr, labels).expect("encode failed");
    decode(word).expect("decode failed")
}

// =============================================================================
// Round-trip tests: N-format (no operands)
// =============================================================================

#[test]
fn roundtrip_nop() {
    assert_eq!(roundtrip(&Instruction::Nop), Instruction::Nop);
}

#[test]
fn roundtrip_ret() {
    assert_eq!(roundtrip(&Instruction::Ret), Instruction::Ret);
}

#[test]
fn roundtrip_halt() {
    assert_eq!(roundtrip(&Instruction::Halt), Instruction::Halt);
}

#[test]
fn roundtrip_hfork() {
    assert_eq!(roundtrip(&Instruction::HFork), Instruction::HFork);
}

#[test]
fn roundtrip_hmerge() {
    assert_eq!(roundtrip(&Instruction::HMerge), Instruction::HMerge);
}

// =============================================================================
// Round-trip tests: RRR-format (3-register)
// =============================================================================

#[test]
fn roundtrip_iadd() {
    let instr = Instruction::IAdd { dst: 2, lhs: 3, rhs: 4 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_isub() {
    let instr = Instruction::ISub { dst: 0, lhs: 15, rhs: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_imul() {
    let instr = Instruction::IMul { dst: 10, lhs: 5, rhs: 7 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_idiv() {
    let instr = Instruction::IDiv { dst: 1, lhs: 2, rhs: 3 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_imod() {
    let instr = Instruction::IMod { dst: 4, lhs: 5, rhs: 6 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_iand() {
    let instr = Instruction::IAnd { dst: 7, lhs: 8, rhs: 9 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ior() {
    let instr = Instruction::IOr { dst: 10, lhs: 11, rhs: 12 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ixor() {
    let instr = Instruction::IXor { dst: 13, lhs: 14, rhs: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ieq() {
    let instr = Instruction::IEq { dst: 0, lhs: 1, rhs: 2 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ilt() {
    let instr = Instruction::ILt { dst: 3, lhs: 4, rhs: 5 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_igt() {
    let instr = Instruction::IGt { dst: 6, lhs: 7, rhs: 8 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fadd() {
    let instr = Instruction::FAdd { dst: 0, lhs: 1, rhs: 2 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fsub() {
    let instr = Instruction::FSub { dst: 3, lhs: 4, rhs: 5 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fmul() {
    let instr = Instruction::FMul { dst: 6, lhs: 7, rhs: 8 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fdiv() {
    let instr = Instruction::FDiv { dst: 9, lhs: 10, rhs: 11 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_feq() {
    let instr = Instruction::FEq { dst: 12, lhs: 13, rhs: 14 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_flt() {
    let instr = Instruction::FLt { dst: 15, lhs: 0, rhs: 1 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fgt() {
    let instr = Instruction::FGt { dst: 2, lhs: 3, rhs: 4 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zadd() {
    let instr = Instruction::ZAdd { dst: 5, lhs: 6, rhs: 7 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zsub() {
    let instr = Instruction::ZSub { dst: 8, lhs: 9, rhs: 10 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zmul() {
    let instr = Instruction::ZMul { dst: 11, lhs: 12, rhs: 13 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zdiv() {
    let instr = Instruction::ZDiv { dst: 14, lhs: 15, rhs: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: RR-format (2-register)
// =============================================================================

#[test]
fn roundtrip_inot() {
    let instr = Instruction::INot { dst: 3, src: 7 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_cvtif() {
    let instr = Instruction::CvtIF { dst_f: 0, src_i: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_cvtfi() {
    let instr = Instruction::CvtFI { dst_i: 5, src_f: 10 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_cvtfz() {
    let instr = Instruction::CvtFZ { dst_z: 8, src_f: 3 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_cvtzf() {
    let instr = Instruction::CvtZF { dst_f: 12, src_z: 1 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: Configuration query (R1-format)
// =============================================================================

#[test]
fn roundtrip_iqcfg() {
    let instr = Instruction::IQCfg { dst: 7 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_iqcfg_all_regs() {
    for dst in 0..=15 {
        let instr = Instruction::IQCfg { dst };
        assert_eq!(roundtrip(&instr), instr);
    }
}

// =============================================================================
// Round-trip tests: RRS-format (2-register + shift)
// =============================================================================

#[test]
fn roundtrip_ishl() {
    let instr = Instruction::IShl { dst: 0, src: 1, amt: 32 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ishr() {
    let instr = Instruction::IShr { dst: 15, src: 14, amt: 63 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ishl_zero_shift() {
    let instr = Instruction::IShl { dst: 5, src: 3, amt: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: RI-format (register + 16-bit immediate)
// =============================================================================

#[test]
fn roundtrip_ildi_positive() {
    let instr = Instruction::ILdi { dst: 0, imm: 1000 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ildi_negative() {
    let instr = Instruction::ILdi { dst: 5, imm: -100 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ildi_max() {
    let instr = Instruction::ILdi { dst: 15, imm: i16::MAX };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ildi_min() {
    let instr = Instruction::ILdi { dst: 0, imm: i16::MIN };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ildi_zero() {
    let instr = Instruction::ILdi { dst: 7, imm: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fldi() {
    let instr = Instruction::FLdi { dst: 3, imm: -500 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: ZI-format (complex immediate)
// =============================================================================

#[test]
fn roundtrip_zldi_positive() {
    let instr = Instruction::ZLdi { dst: 0, imm_re: 5, imm_im: 10 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zldi_negative() {
    let instr = Instruction::ZLdi { dst: 3, imm_re: -128, imm_im: 127 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zldi_zero() {
    let instr = Instruction::ZLdi { dst: 15, imm_re: 0, imm_im: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zldi_both_negative() {
    let instr = Instruction::ZLdi { dst: 7, imm_re: -1, imm_im: -1 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: RA-format (register + 16-bit address)
// =============================================================================

#[test]
fn roundtrip_ildm() {
    let instr = Instruction::ILdm { dst: 3, addr: 1000 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_istr() {
    let instr = Instruction::IStr { src: 5, addr: 2000 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fldm() {
    let instr = Instruction::FLdm { dst: 0, addr: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fstr() {
    let instr = Instruction::FStr { src: 15, addr: 65535 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zldm() {
    let instr = Instruction::ZLdm { dst: 8, addr: 32768 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zstr() {
    let instr = Instruction::ZStr { src: 12, addr: 100 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: J-format (24-bit jump address)
// =============================================================================

#[test]
fn roundtrip_jmp() {
    let labels = test_labels();
    let instr = Instruction::Jmp { target: "end".to_string() };
    let decoded = roundtrip_with_labels(&instr, &labels);
    assert_eq!(decoded, Instruction::Jmp { target: "@42".to_string() });
}

#[test]
fn roundtrip_jmp_max_address() {
    let labels = test_labels();
    let instr = Instruction::Jmp { target: "far".to_string() };
    let decoded = roundtrip_with_labels(&instr, &labels);
    assert_eq!(
        decoded,
        Instruction::Jmp {
            target: format!("@{}", 0x00FF_FFFF)
        }
    );
}

#[test]
fn roundtrip_call() {
    let labels = test_labels();
    let instr = Instruction::Call { target: "loop".to_string() };
    let decoded = roundtrip_with_labels(&instr, &labels);
    assert_eq!(decoded, Instruction::Call { target: "@10".to_string() });
}

#[test]
fn roundtrip_call_address_zero() {
    let labels = test_labels();
    let instr = Instruction::Call { target: "start".to_string() };
    let decoded = roundtrip_with_labels(&instr, &labels);
    assert_eq!(decoded, Instruction::Call { target: "@0".to_string() });
}

// =============================================================================
// Round-trip tests: JR-format (predicate + 16-bit address)
// =============================================================================

#[test]
fn roundtrip_jif() {
    let labels = test_labels();
    let instr = Instruction::Jif { pred: 3, target: "loop".to_string() };
    let decoded = roundtrip_with_labels(&instr, &labels);
    assert_eq!(
        decoded,
        Instruction::Jif {
            pred: 3,
            target: "@10".to_string()
        }
    );
}

#[test]
fn roundtrip_jif_max_address() {
    let labels = test_labels();
    let instr = Instruction::Jif {
        pred: 0,
        target: "near_max".to_string(),
    };
    let decoded = roundtrip_with_labels(&instr, &labels);
    assert_eq!(
        decoded,
        Instruction::Jif {
            pred: 0,
            target: "@65535".to_string()
        }
    );
}

#[test]
fn roundtrip_jmpf() {
    let labels = test_labels();
    let instr = Instruction::JmpF { flag: FlagId::Sf, target: "end".to_string() };
    let decoded = roundtrip_with_labels(&instr, &labels);
    assert_eq!(
        decoded,
        Instruction::JmpF {
            flag: FlagId::Sf,
            target: "@42".to_string()
        }
    );
}

// =============================================================================
// Round-trip tests: QP-format (quantum prepare)
// =============================================================================

#[test]
fn roundtrip_qprep_uniform() {
    let instr = Instruction::QPrep { dst: 0, dist: DistId::Uniform };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qprep_ghz() {
    let instr = Instruction::QPrep { dst: 7, dist: DistId::Ghz };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qprep_max_dist() {
    let instr = Instruction::QPrep { dst: 4, dist: DistId::Ghz };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: Q-format (quantum kernel)
// =============================================================================

#[test]
fn roundtrip_qkernel() {
    let instr = Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: KernelId::Fourier,
        ctx0: 3,
        ctx1: 4,
    };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qkernel_max_values() {
    let instr = Instruction::QKernel {
        dst: 7,
        src: 7,
        kernel: KernelId::Permutation,
        ctx0: 15,
        ctx1: 15,
    };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qkernel_zero_values() {
    let instr = Instruction::QKernel {
        dst: 0,
        src: 0,
        kernel: KernelId::Init,
        ctx0: 0,
        ctx1: 0,
    };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: QO-format (quantum observe)
// =============================================================================

#[test]
fn roundtrip_qobserve() {
    let instr = Instruction::QObserve { dst_h: 2, src_q: 5, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qobserve_max() {
    let instr = Instruction::QObserve { dst_h: 7, src_q: 7, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

// (QSAMPLE round-trip tests removed: QSAMPLE was removed from the ISA.)

// =============================================================================
// Round-trip tests: QO-format with mode (QOBSERVE mode dispatch)
// =============================================================================

#[test]
fn roundtrip_qobserve_mode_prob() {
    let instr = Instruction::QObserve { dst_h: 0, src_q: 1, mode: ObserveMode::Prob, ctx0: 3, ctx1: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

// (roundtrip_qobserve_mode_amp removed: AMP mode was removed from the ISA.)

#[test]
fn roundtrip_qobserve_backward_compat() {
    // mode=0, ctx0=0, ctx1=0 should round-trip identically to legacy format
    let instr = Instruction::QObserve { dst_h: 3, src_q: 6, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qobserve_mode_max_ctx() {
    // Max ctx0/ctx1 values (4 bits each = 15) — uses PROB (mode=1) now that AMP is removed.
    let instr = Instruction::QObserve { dst_h: 7, src_q: 7, mode: ObserveMode::Prob, ctx0: 15, ctx1: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: QS-format (quantum memory)
// =============================================================================

#[test]
fn roundtrip_qload() {
    let instr = Instruction::QLoad { dst_q: 3, addr: 100 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qload_max() {
    let instr = Instruction::QLoad { dst_q: 7, addr: 255 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qstore() {
    let instr = Instruction::QStore { src_q: 0, addr: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qstore_max() {
    let instr = Instruction::QStore { src_q: 7, addr: 255 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: HR-format (hybrid reduce)
// =============================================================================

#[test]
fn roundtrip_hreduce() {
    let instr = Instruction::HReduce { src: 2, dst: 5, func: ReduceFn::Mean };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_hreduce_max_func() {
    let instr = Instruction::HReduce { src: 7, dst: 15, func: ReduceFn::NegateZ };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_hreduce_zero() {
    let instr = Instruction::HReduce { src: 0, dst: 0, func: ReduceFn::Round };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: L-format (label)
// =============================================================================

#[test]
fn roundtrip_label() {
    let mut labels = HashMap::new();
    labels.insert("my_label".to_string(), 42u32);

    let instr = Instruction::Label("my_label".to_string());
    let word = encode(&instr, &labels).unwrap();
    // Without debug map, we get a synthetic name _L{id}
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, Instruction::Label("_L42".to_string()));
}

#[test]
fn roundtrip_label_with_debug() {
    let mut labels = HashMap::new();
    labels.insert("my_label".to_string(), 42u32);

    let instr = Instruction::Label("my_label".to_string());
    let word = encode(&instr, &labels).unwrap();

    // With debug map, the original name is restored
    let mut debug_map = HashMap::new();
    debug_map.insert(42u16, "my_label".to_string());

    let decoded = decode_with_debug(word, &debug_map).unwrap();
    assert_eq!(decoded, Instruction::Label("my_label".to_string()));
}

// =============================================================================
// Specific bit pattern tests (verifying exact encoding)
// =============================================================================

#[test]
fn exact_encoding_iadd() {
    // IADD R2, R3, R4 -> 0x01234000
    let labels = HashMap::new();
    let instr = Instruction::IAdd { dst: 2, lhs: 3, rhs: 4 };
    let word = encode(&instr, &labels).unwrap();
    assert_eq!(word, 0x01234000);
}

#[test]
fn exact_encoding_ildi_negative() {
    // ILDI R5, -100 -> 0x0C50FF9C
    let labels = HashMap::new();
    let instr = Instruction::ILdi { dst: 5, imm: -100 };
    let word = encode(&instr, &labels).unwrap();
    assert_eq!(word, 0x0C50FF9C);
}

#[test]
fn exact_encoding_jmp_256() {
    // JMP @256 -> 0x27000100
    let mut labels = HashMap::new();
    labels.insert("target".to_string(), 256u32);
    let instr = Instruction::Jmp { target: "target".to_string() };
    let word = encode(&instr, &labels).unwrap();
    assert_eq!(word, 0x27000100);
}

#[test]
fn exact_encoding_qkernel() {
    // QKERNEL QFFT, Q1, Q0, R3, R4 -> 0x31204680
    let labels = HashMap::new();
    let instr = Instruction::QKernel {
        dst: 1,
        src: 0,
        kernel: KernelId::Fourier,
        ctx0: 3,
        ctx1: 4,
    };
    let word = encode(&instr, &labels).unwrap();
    assert_eq!(word, 0x31204680);
}

#[test]
fn exact_encoding_zldi() {
    // ZLDI Z3, 5, -2 -> 0x203005FE
    let labels = HashMap::new();
    let instr = Instruction::ZLdi { dst: 3, imm_re: 5, imm_im: -2 };
    let word = encode(&instr, &labels).unwrap();
    assert_eq!(word, 0x203005FE);
}

#[test]
fn exact_encoding_nop() {
    let labels = HashMap::new();
    let word = encode(&Instruction::Nop, &labels).unwrap();
    assert_eq!(word, 0x00000000);
}

#[test]
fn exact_encoding_halt() {
    let labels = HashMap::new();
    let word = encode(&Instruction::Halt, &labels).unwrap();
    assert_eq!(word, 0x2B000000);
}

#[test]
fn exact_encoding_ret() {
    let labels = HashMap::new();
    let word = encode(&Instruction::Ret, &labels).unwrap();
    assert_eq!(word, 0x2A000000);
}

// =============================================================================
// Error case tests
// =============================================================================

#[test]
fn error_unresolved_label_jmp() {
    let labels = HashMap::new();
    let instr = Instruction::Jmp { target: "nonexistent".to_string() };
    let result = encode(&instr, &labels);
    assert!(result.is_err());
}

#[test]
fn error_unresolved_label_call() {
    let labels = HashMap::new();
    let instr = Instruction::Call { target: "missing".to_string() };
    let result = encode(&instr, &labels);
    assert!(result.is_err());
}

#[test]
fn error_unresolved_label_jif() {
    let labels = HashMap::new();
    let instr = Instruction::Jif { pred: 0, target: "missing".to_string() };
    let result = encode(&instr, &labels);
    assert!(result.is_err());
}

#[test]
fn error_unresolved_label_jmpf() {
    let labels = HashMap::new();
    let instr = Instruction::JmpF { flag: FlagId::Zf, target: "missing".to_string() };
    let result = encode(&instr, &labels);
    assert!(result.is_err());
}

#[test]
fn error_decode_invalid_opcode_0xff() {
    let word: u32 = 0xFF_000000;
    let result = decode(word);
    assert!(result.is_err());
}

#[test]
fn error_decode_invalid_opcode_0xfe() {
    let word: u32 = 0xFE_000000;
    let result = decode(word);
    assert!(result.is_err());
}

// (decode_qsample_opcode_0x40 removed: 0x40 is now reserved, QSAMPLE removed from ISA.)

#[test]
fn decode_reti_opcode_0x2d() {
    // 0x2D is now assigned to RETI
    let word: u32 = 0x2D_000000;
    let result = decode(word).unwrap();
    assert_eq!(result, Instruction::Reti);
}

#[test]
fn error_decode_invalid_opcode_in_gap_0x3f() {
    // 0x3F is in the reserved gap (0x35-0x3E now assigned to indirect memory)
    let word: u32 = 0x3F_000000;
    let result = decode(word);
    assert!(result.is_err());
}

#[test]
fn error_reg4_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::IAdd { dst: 16, lhs: 0, rhs: 0 };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_reg4_overflow_lhs() {
    let labels = HashMap::new();
    let instr = Instruction::IAdd { dst: 0, lhs: 16, rhs: 0 };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_reg4_overflow_rhs() {
    let labels = HashMap::new();
    let instr = Instruction::IAdd { dst: 0, lhs: 0, rhs: 16 };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_reg3_overflow_qprep() {
    let labels = HashMap::new();
    let instr = Instruction::QPrep { dst: 8, dist: DistId::Uniform };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_dist_overflow() {
    // With type-safe enums, invalid dist values are caught at TryFrom time
    assert!(DistId::try_from(8u8).is_err());
}

#[test]
fn error_kernel_overflow() {
    // With type-safe enums, invalid kernel values are caught at TryFrom time
    assert!(KernelId::try_from(32u8).is_err());
}

#[test]
fn error_shift_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::IShl { dst: 0, src: 0, amt: 64 };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_reduce_func_overflow() {
    // With type-safe enums, invalid func values are caught at TryFrom time
    assert!(ReduceFn::try_from(32u8).is_err());
}

#[test]
fn error_jif_address_overflow() {
    // JIF targets must fit in 16 bits. If the label address exceeds 0xFFFF,
    // encoding should fail.
    let mut labels = HashMap::new();
    labels.insert("too_far".to_string(), 0x10000u32);
    let instr = Instruction::Jif {
        pred: 0,
        target: "too_far".to_string(),
    };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_jmpf_address_overflow() {
    let mut labels = HashMap::new();
    labels.insert("too_far".to_string(), 0x10000u32);
    let instr = Instruction::JmpF {
        flag: FlagId::Zf,
        target: "too_far".to_string(),
    };
    assert!(encode(&instr, &labels).is_err());
}

// =============================================================================
// Label resolution tests
// =============================================================================

#[test]
fn label_resolution_forward_reference() {
    let mut labels = HashMap::new();
    labels.insert("forward".to_string(), 100u32);

    let instr = Instruction::Jmp { target: "forward".to_string() };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(
        decoded,
        Instruction::Jmp {
            target: "@100".to_string()
        }
    );
}

#[test]
fn label_resolution_backward_reference() {
    let mut labels = HashMap::new();
    labels.insert("backward".to_string(), 5u32);

    let instr = Instruction::Jmp { target: "backward".to_string() };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(
        decoded,
        Instruction::Jmp {
            target: "@5".to_string()
        }
    );
}

#[test]
fn label_resolution_address_zero() {
    let mut labels = HashMap::new();
    labels.insert("zero_addr".to_string(), 0u32);

    let instr = Instruction::Jmp { target: "zero_addr".to_string() };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(
        decoded,
        Instruction::Jmp {
            target: "@0".to_string()
        }
    );
}

// =============================================================================
// Edge case tests
// =============================================================================

#[test]
fn edge_max_reg4_values() {
    // All fields at max (15)
    let instr = Instruction::IAdd { dst: 15, lhs: 15, rhs: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn edge_max_reg3_values_qobserve() {
    let instr = Instruction::QObserve { dst_h: 7, src_q: 7, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn edge_max_shift_amount() {
    let instr = Instruction::IShl { dst: 0, src: 0, amt: 63 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn edge_max_address_ra() {
    let instr = Instruction::ILdm { dst: 0, addr: 0xFFFF };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn edge_max_address_qs() {
    let instr = Instruction::QLoad { dst_q: 0, addr: 255 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn edge_max_24bit_address() {
    let mut labels = HashMap::new();
    labels.insert("max24".to_string(), 0x00FF_FFFFu32);
    let instr = Instruction::Jmp { target: "max24".to_string() };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(
        decoded,
        Instruction::Jmp {
            target: format!("@{}", 0x00FF_FFFF)
        }
    );
}

#[test]
fn edge_ildi_minus_one() {
    let instr = Instruction::ILdi { dst: 0, imm: -1 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn edge_zldi_min_max_i8() {
    let instr = Instruction::ZLdi { dst: 0, imm_re: i8::MIN, imm_im: i8::MAX };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Mnemonic tests
// =============================================================================

#[test]
fn mnemonic_all_assigned_opcodes() {
    // Every assigned opcode should have a mnemonic
    let assigned = vec![
        (op::NOP, "NOP"),
        (op::IADD, "IADD"),
        (op::ISUB, "ISUB"),
        (op::IMUL, "IMUL"),
        (op::IDIV, "IDIV"),
        (op::IMOD, "IMOD"),
        (op::IAND, "IAND"),
        (op::IOR, "IOR"),
        (op::IXOR, "IXOR"),
        (op::INOT, "INOT"),
        (op::ISHL, "ISHL"),
        (op::ISHR, "ISHR"),
        (op::ILDI, "ILDI"),
        (op::ILDM, "ILDM"),
        (op::ISTR, "ISTR"),
        (op::IEQ, "IEQ"),
        (op::ILT, "ILT"),
        (op::IGT, "IGT"),
        (op::FADD, "FADD"),
        (op::FSUB, "FSUB"),
        (op::FMUL, "FMUL"),
        (op::FDIV, "FDIV"),
        (op::FLDI, "FLDI"),
        (op::FLDM, "FLDM"),
        (op::FSTR, "FSTR"),
        (op::FEQ, "FEQ"),
        (op::FLT, "FLT"),
        (op::FGT, "FGT"),
        (op::ZADD, "ZADD"),
        (op::ZSUB, "ZSUB"),
        (op::ZMUL, "ZMUL"),
        (op::ZDIV, "ZDIV"),
        (op::ZLDI, "ZLDI"),
        (op::ZLDM, "ZLDM"),
        (op::ZSTR, "ZSTR"),
        (op::CVTIF, "CVTIF"),
        (op::CVTFI, "CVTFI"),
        (op::CVTFZ, "CVTFZ"),
        (op::CVTZF, "CVTZF"),
        (op::JMP, "JMP"),
        (op::JIF, "JIF"),
        (op::CALL, "CALL"),
        (op::RET, "RET"),
        (op::HALT, "HALT"),
        (op::LABEL, "LABEL"),
        (op::QPREP, "QPREP"),
        (op::QKERNEL, "QKERNEL"),
        (op::QOBSERVE, "QOBSERVE"),
        (op::QLOAD, "QLOAD"),
        (op::QSTORE, "QSTORE"),
        (op::QKERNELF, "QKERNELF"),
        (op::QKERNELZ, "QKERNELZ"),
        (op::ILDX, "ILDX"),
        (op::ISTRX, "ISTRX"),
        (op::FLDX, "FLDX"),
        (op::FSTRX, "FSTRX"),
        (op::ZLDX, "ZLDX"),
        (op::ZSTRX, "ZSTRX"),
        (op::HFORK, "HFORK"),
        (op::HMERGE, "HMERGE"),
        (op::JMPF, "JMPF"),
        (op::HREDUCE, "HREDUCE"),
    ];

    for (code, expected_name) in &assigned {
        assert_eq!(
            mnemonic(*code),
            Some(*expected_name),
            "Mnemonic mismatch for opcode 0x{:02X}",
            code
        );
    }
}

#[test]
fn mnemonic_unassigned_returns_none() {
    // Test several unassigned opcode values
    for code in &[0x3F_u8, 0x80, 0xFE, 0xFF] {
        assert_eq!(
            mnemonic(*code),
            None,
            "Expected None for unassigned opcode 0x{:02X}",
            code
        );
    }
}

// =============================================================================
// Round-trip tests: RR-format (register-indirect memory)
// =============================================================================

#[test]
fn roundtrip_ildx() {
    let instr = Instruction::ILdx { dst: 3, addr_reg: 5 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_istrx() {
    let instr = Instruction::IStrx { src: 7, addr_reg: 2 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fldx() {
    let instr = Instruction::FLdx { dst: 0, addr_reg: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_fstrx() {
    let instr = Instruction::FStrx { src: 14, addr_reg: 1 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zldx() {
    let instr = Instruction::ZLdx { dst: 10, addr_reg: 4 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zstrx() {
    let instr = Instruction::ZStrx { src: 6, addr_reg: 8 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_ildx_max_regs() {
    let instr = Instruction::ILdx { dst: 15, addr_reg: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_zstrx_zero_regs() {
    let instr = Instruction::ZStrx { src: 0, addr_reg: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Comprehensive round-trip test: encode every variant, decode, verify
// =============================================================================

#[test]
fn roundtrip_all_variants_comprehensive() {
    let mut labels = HashMap::new();
    labels.insert("target".to_string(), 100u32);

    // N-format
    let nop_w = encode(&Instruction::Nop, &labels).unwrap();
    assert_eq!(decode(nop_w).unwrap(), Instruction::Nop);

    let ret_w = encode(&Instruction::Ret, &labels).unwrap();
    assert_eq!(decode(ret_w).unwrap(), Instruction::Ret);

    let halt_w = encode(&Instruction::Halt, &labels).unwrap();
    assert_eq!(decode(halt_w).unwrap(), Instruction::Halt);

    let hfork_w = encode(&Instruction::HFork, &labels).unwrap();
    assert_eq!(decode(hfork_w).unwrap(), Instruction::HFork);

    let hmerge_w = encode(&Instruction::HMerge, &labels).unwrap();
    assert_eq!(decode(hmerge_w).unwrap(), Instruction::HMerge);

    // All RRR variants
    let rrr_variants: Vec<Instruction> = vec![
        Instruction::IAdd { dst: 1, lhs: 2, rhs: 3 },
        Instruction::ISub { dst: 4, lhs: 5, rhs: 6 },
        Instruction::IMul { dst: 7, lhs: 8, rhs: 9 },
        Instruction::IDiv { dst: 10, lhs: 11, rhs: 12 },
        Instruction::IMod { dst: 13, lhs: 14, rhs: 15 },
        Instruction::IAnd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::IOr { dst: 3, lhs: 4, rhs: 5 },
        Instruction::IXor { dst: 6, lhs: 7, rhs: 8 },
        Instruction::IEq { dst: 9, lhs: 10, rhs: 11 },
        Instruction::ILt { dst: 12, lhs: 13, rhs: 14 },
        Instruction::IGt { dst: 15, lhs: 0, rhs: 1 },
        Instruction::FAdd { dst: 2, lhs: 3, rhs: 4 },
        Instruction::FSub { dst: 5, lhs: 6, rhs: 7 },
        Instruction::FMul { dst: 8, lhs: 9, rhs: 10 },
        Instruction::FDiv { dst: 11, lhs: 12, rhs: 13 },
        Instruction::FEq { dst: 14, lhs: 15, rhs: 0 },
        Instruction::FLt { dst: 1, lhs: 2, rhs: 3 },
        Instruction::FGt { dst: 4, lhs: 5, rhs: 6 },
        Instruction::ZAdd { dst: 7, lhs: 8, rhs: 9 },
        Instruction::ZSub { dst: 10, lhs: 11, rhs: 12 },
        Instruction::ZMul { dst: 13, lhs: 14, rhs: 15 },
        Instruction::ZDiv { dst: 0, lhs: 1, rhs: 2 },
    ];

    for instr in &rrr_variants {
        let w = encode(instr, &labels).unwrap();
        assert_eq!(decode(w).unwrap(), *instr, "RRR round-trip failed for {:?}", instr);
    }

    // RR variants
    let rr_variants: Vec<Instruction> = vec![
        Instruction::INot { dst: 3, src: 7 },
        Instruction::CvtIF { dst_f: 0, src_i: 15 },
        Instruction::CvtFI { dst_i: 5, src_f: 10 },
        Instruction::CvtFZ { dst_z: 8, src_f: 3 },
        Instruction::CvtZF { dst_f: 12, src_z: 1 },
    ];

    for instr in &rr_variants {
        let w = encode(instr, &labels).unwrap();
        assert_eq!(decode(w).unwrap(), *instr, "RR round-trip failed for {:?}", instr);
    }

    // RRS variants
    let rrs_variants: Vec<Instruction> = vec![
        Instruction::IShl { dst: 0, src: 1, amt: 32 },
        Instruction::IShr { dst: 15, src: 14, amt: 63 },
    ];

    for instr in &rrs_variants {
        let w = encode(instr, &labels).unwrap();
        assert_eq!(decode(w).unwrap(), *instr, "RRS round-trip failed for {:?}", instr);
    }

    // RI variants
    let ri_variants: Vec<Instruction> = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 15, imm: -1 },
        Instruction::FLdi { dst: 7, imm: 100 },
    ];

    for instr in &ri_variants {
        let w = encode(instr, &labels).unwrap();
        assert_eq!(decode(w).unwrap(), *instr, "RI round-trip failed for {:?}", instr);
    }

    // ZI variant
    let zi = Instruction::ZLdi { dst: 3, imm_re: 5, imm_im: -2 };
    let w = encode(&zi, &labels).unwrap();
    assert_eq!(decode(w).unwrap(), zi);

    // RA variants
    let ra_variants: Vec<Instruction> = vec![
        Instruction::ILdm { dst: 3, addr: 1000 },
        Instruction::IStr { src: 5, addr: 2000 },
        Instruction::FLdm { dst: 0, addr: 0 },
        Instruction::FStr { src: 15, addr: 65535 },
        Instruction::ZLdm { dst: 8, addr: 32768 },
        Instruction::ZStr { src: 12, addr: 100 },
    ];

    for instr in &ra_variants {
        let w = encode(instr, &labels).unwrap();
        assert_eq!(decode(w).unwrap(), *instr, "RA round-trip failed for {:?}", instr);
    }

    // Quantum variants
    let q_variants: Vec<Instruction> = vec![
        Instruction::QPrep { dst: 0, dist: DistId::Uniform },
        Instruction::QPrep { dst: 7, dist: DistId::Ghz },
        Instruction::QKernel { dst: 1, src: 0, kernel: KernelId::Fourier, ctx0: 3, ctx1: 4 },
        Instruction::QObserve { dst_h: 2, src_q: 5, mode: ObserveMode::Dist, ctx0: 0, ctx1: 0 },
        Instruction::QLoad { dst_q: 3, addr: 100 },
        Instruction::QStore { src_q: 0, addr: 0 },
    ];

    for instr in &q_variants {
        let w = encode(instr, &labels).unwrap();
        assert_eq!(decode(w).unwrap(), *instr, "Q round-trip failed for {:?}", instr);
    }

    // Hybrid reduce
    let hr = Instruction::HReduce { src: 2, dst: 5, func: ReduceFn::Mean };
    let w = encode(&hr, &labels).unwrap();
    assert_eq!(decode(w).unwrap(), hr);

    // Register-indirect memory variants
    let indirect_variants: Vec<Instruction> = vec![
        Instruction::ILdx { dst: 0, addr_reg: 1 },
        Instruction::IStrx { src: 2, addr_reg: 3 },
        Instruction::FLdx { dst: 4, addr_reg: 5 },
        Instruction::FStrx { src: 6, addr_reg: 7 },
        Instruction::ZLdx { dst: 8, addr_reg: 9 },
        Instruction::ZStrx { src: 10, addr_reg: 11 },
    ];

    for instr in &indirect_variants {
        let w = encode(instr, &labels).unwrap();
        assert_eq!(decode(w).unwrap(), *instr, "Indirect round-trip failed for {:?}", instr);
    }
}

// =============================================================================
// Round-trip tests: Q-format (QKERNELF, QKERNELZ)
// =============================================================================

#[test]
fn roundtrip_qkernelf() {
    let instr = Instruction::QKernelF {
        dst: 1, src: 0, kernel: KernelId::Rotate, fctx0: 3, fctx1: 4,
    };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qkernelf_max_values() {
    let instr = Instruction::QKernelF {
        dst: 7, src: 7, kernel: KernelId::Permutation, fctx0: 15, fctx1: 15,
    };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qkernelf_zero_values() {
    let instr = Instruction::QKernelF {
        dst: 0, src: 0, kernel: KernelId::Init, fctx0: 0, fctx1: 0,
    };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qkernelz() {
    let instr = Instruction::QKernelZ {
        dst: 1, src: 0, kernel: KernelId::PhaseShift, zctx0: 2, zctx1: 3,
    };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qkernelz_max_values() {
    let instr = Instruction::QKernelZ {
        dst: 7, src: 7, kernel: KernelId::Permutation, zctx0: 15, zctx1: 15,
    };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qkernelz_zero_values() {
    let instr = Instruction::QKernelZ {
        dst: 0, src: 0, kernel: KernelId::Init, zctx0: 0, zctx1: 0,
    };
    assert_eq!(roundtrip(&instr), instr);
}

// =============================================================================
// Round-trip tests: QR-format (QPrepR)
// =============================================================================

#[test]
fn roundtrip_qprepr() {
    let instr = Instruction::QPrepR { dst: 0, dist_reg: 3 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qprepr_max_values() {
    let instr = Instruction::QPrepR { dst: 7, dist_reg: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qprepr_zero_values() {
    let instr = Instruction::QPrepR { dst: 0, dist_reg: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn test_qprepr_mnemonic() {
    assert_eq!(mnemonic(op::QPREPR), Some("QPREPR"));
}

// =============================================================================
// Round-trip tests: QE-format (QEncode)
// =============================================================================

#[test]
fn roundtrip_qencode_r_file() {
    let instr = Instruction::QEncode { dst: 0, src_base: 0, count: 4, file_sel: FileSel::RFile };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qencode_f_file() {
    let instr = Instruction::QEncode { dst: 1, src_base: 2, count: 2, file_sel: FileSel::FFile };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qencode_z_file() {
    let instr = Instruction::QEncode { dst: 3, src_base: 4, count: 8, file_sel: FileSel::ZFile };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qencode_max_values() {
    let instr = Instruction::QEncode { dst: 7, src_base: 15, count: 15, file_sel: FileSel::ZFile };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qencode_zero_values() {
    let instr = Instruction::QEncode { dst: 0, src_base: 0, count: 0, file_sel: FileSel::RFile };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn test_qencode_mnemonic() {
    assert_eq!(mnemonic(op::QENCODE), Some("QENCODE"));
}

#[test]
fn error_qencode_file_sel_overflow() {
    // With type-safe enums, invalid file_sel values are caught at TryFrom time
    assert!(FileSel::try_from(3u8).is_err());
}

#[test]
fn error_qencode_count_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::QEncode { dst: 0, src_base: 0, count: 16, file_sel: FileSel::RFile };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_qprepr_dst_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::QPrepR { dst: 8, dist_reg: 0 };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_qprepr_dist_reg_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::QPrepR { dst: 0, dist_reg: 16 };
    assert!(encode(&instr, &labels).is_err());
}

// =============================================================================
// Round-trip tests: masked register-level gate operations (QHADM, QFLIP, QPHASE)
// =============================================================================

#[test]
fn roundtrip_qhadm() {
    let instr = Instruction::QHadM { dst: 3, src: 2, mask_reg: 7 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qflip() {
    let instr = Instruction::QFlip { dst: 0, src: 1, mask_reg: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qphase() {
    let instr = Instruction::QPhase { dst: 7, src: 7, mask_reg: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qhadm_boundary_values() {
    let instr = Instruction::QHadM { dst: 7, src: 7, mask_reg: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn error_qhadm_dst_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::QHadM { dst: 8, src: 0, mask_reg: 0 };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_qhadm_mask_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::QHadM { dst: 0, src: 0, mask_reg: 16 };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn mnemonic_masked_ops() {
    assert_eq!(mnemonic(op::QHADM), Some("QHADM"));
    assert_eq!(mnemonic(op::QFLIP), Some("QFLIP"));
    assert_eq!(mnemonic(op::QPHASE), Some("QPHASE"));
}

// =============================================================================
// Round-trip tests: qubit-level gate operations (QCNOT, QROT, QMEAS)
// =============================================================================

#[test]
fn roundtrip_qcnot() {
    let instr = Instruction::QCnot { dst: 0, src: 1, ctrl_qubit_reg: 2, tgt_qubit_reg: 3 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qcnot_max_values() {
    let instr = Instruction::QCnot { dst: 7, src: 7, ctrl_qubit_reg: 15, tgt_qubit_reg: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qcnot_zero_values() {
    let instr = Instruction::QCnot { dst: 0, src: 0, ctrl_qubit_reg: 0, tgt_qubit_reg: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qrot_x() {
    let instr = Instruction::QRot { dst: 0, src: 1, qubit_reg: 2, axis: RotAxis::X, angle_freg: 3 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qrot_y() {
    let instr = Instruction::QRot { dst: 3, src: 4, qubit_reg: 5, axis: RotAxis::Y, angle_freg: 6 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qrot_z() {
    let instr = Instruction::QRot { dst: 7, src: 7, qubit_reg: 15, axis: RotAxis::Z, angle_freg: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qrot_zero_values() {
    let instr = Instruction::QRot { dst: 0, src: 0, qubit_reg: 0, axis: RotAxis::X, angle_freg: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qmeas() {
    let instr = Instruction::QMeas { dst_r: 0, src_q: 1, qubit_reg: 2 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qmeas_max_values() {
    let instr = Instruction::QMeas { dst_r: 15, src_q: 7, qubit_reg: 15 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn roundtrip_qmeas_zero_values() {
    let instr = Instruction::QMeas { dst_r: 0, src_q: 0, qubit_reg: 0 };
    assert_eq!(roundtrip(&instr), instr);
}

#[test]
fn mnemonic_phase5a_ops() {
    assert_eq!(mnemonic(op::QCNOT), Some("QCNOT"));
    assert_eq!(mnemonic(op::QROT), Some("QROT"));
    assert_eq!(mnemonic(op::QMEAS), Some("QMEAS"));
}

#[test]
fn error_qcnot_dst_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::QCnot { dst: 8, src: 0, ctrl_qubit_reg: 0, tgt_qubit_reg: 0 };
    assert!(encode(&instr, &labels).is_err());
}

#[test]
fn error_qrot_axis_overflow() {
    // With type-safe enums, invalid axis values are caught at TryFrom time
    assert!(RotAxis::try_from(3u8).is_err());
}

#[test]
fn error_qmeas_dst_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::QMeas { dst_r: 16, src_q: 0, qubit_reg: 0 };
    assert!(encode(&instr, &labels).is_err());
}

// ===========================================================================
// Round-trip tests: extended quantum operations (QTENSOR, QCUSTOM, QCZ, QSWAP)
// ===========================================================================

#[test]
fn roundtrip_qtensor() {
    let labels = HashMap::new();
    let instr = Instruction::QTensor { dst: 3, src0: 5, src1: 7 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_qcustom() {
    let labels = HashMap::new();
    let instr = Instruction::QCustom { dst: 2, src: 4, base_addr_reg: 8, dim_reg: 12 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_qcz() {
    let labels = HashMap::new();
    let instr = Instruction::QCz { dst: 1, src: 3, ctrl_qubit_reg: 5, tgt_qubit_reg: 9 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_qswap() {
    let labels = HashMap::new();
    let instr = Instruction::QSwap { dst: 0, src: 6, qubit_a_reg: 10, qubit_b_reg: 14 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn mnemonic_new_opcodes() {
    assert_eq!(mnemonic(op::QTENSOR), Some("QTENSOR"));
    assert_eq!(mnemonic(op::QCUSTOM), Some("QCUSTOM"));
    assert_eq!(mnemonic(op::QCZ), Some("QCZ"));
    assert_eq!(mnemonic(op::QSWAP), Some("QSWAP"));
}

#[test]
fn error_qtensor_q_overflow() {
    let labels = HashMap::new();
    let instr = Instruction::QTensor { dst: 8, src0: 0, src1: 0 };
    assert!(encode(&instr, &labels).is_err());
}

// =============================================================================
// Round-trip tests: mixed-state, partial-trace, reset, and float math opcodes
// =============================================================================

#[test]
fn roundtrip_qmixed() {
    let labels = HashMap::new();
    let instr = Instruction::QMixed { dst: 3, base_addr_reg: 7, count_reg: 12 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_qprepn() {
    let labels = HashMap::new();
    let instr = Instruction::QPrepN { dst: 5, dist: DistId::Ghz, qubit_count_reg: 10 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_fsin() {
    let labels = HashMap::new();
    let instr = Instruction::FSin { dst: 4, src: 9 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_fcos() {
    let labels = HashMap::new();
    let instr = Instruction::FCos { dst: 0, src: 15 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_fatan2() {
    let labels = HashMap::new();
    let instr = Instruction::FAtan2 { dst: 2, lhs: 6, rhs: 11 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_fsqrt() {
    let labels = HashMap::new();
    let instr = Instruction::FSqrt { dst: 7, src: 3 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_qptrace() {
    let labels = HashMap::new();
    let instr = Instruction::QPtrace { dst: 2, src: 4, num_qubits_a_reg: 8 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_qreset() {
    let labels = HashMap::new();
    let instr = Instruction::QReset { dst: 1, src: 6, qubit_reg: 14 };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn mnemonic_p2_opcodes() {
    assert_eq!(mnemonic(op::QMIXED), Some("QMIXED"));
    assert_eq!(mnemonic(op::QPREPN), Some("QPREPN"));
    assert_eq!(mnemonic(op::FSIN), Some("FSIN"));
    assert_eq!(mnemonic(op::FCOS), Some("FCOS"));
    assert_eq!(mnemonic(op::FATAN2), Some("FATAN2"));
    assert_eq!(mnemonic(op::FSQRT), Some("FSQRT"));
    assert_eq!(mnemonic(op::QPTRACE), Some("QPTRACE"));
    assert_eq!(mnemonic(op::QRESET), Some("QRESET"));
}

#[test]
fn roundtrip_hreduce_expect() {
    let labels = HashMap::new();
    let instr = Instruction::HReduce { src: 3, dst: 5, func: cqam_core::instruction::ReduceFn::Expect };
    let word = encode(&instr, &labels).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}
