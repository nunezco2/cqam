//! Integration tests for the two-pass assembler.

use std::io::Cursor;

use cqam_as::assembler::{
    assemble, assemble_source, assemble_source_with_options, assemble_with_options,
    AssemblyOptions, AssemblyResult,
};
use cqam_as::binary::{read_cqb, write_cqb};
use cqam_core::instruction::Instruction;
use cqam_core::opcode;

// =============================================================================
// Helper functions
// =============================================================================

/// Assemble a program and assert it succeeds.
fn assemble_ok(instrs: Vec<Instruction>) -> AssemblyResult {
    assemble(&instrs).expect("assembly should succeed")
}

/// Assemble source text and assert it succeeds.
fn assemble_source_ok(source: &str) -> AssemblyResult {
    assemble_source(source).expect("assembly should succeed")
}

/// Decode a word and assert it succeeds.
fn decode_ok(word: u32) -> Instruction {
    opcode::decode(word).expect("decode should succeed")
}

// =============================================================================
// Single-instruction round-trip tests
// =============================================================================

#[test]
fn test_roundtrip_each_integer_op() {
    let cases: Vec<Instruction> = vec![
        Instruction::IAdd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ISub { dst: 3, lhs: 4, rhs: 5 },
        Instruction::IMul { dst: 6, lhs: 7, rhs: 8 },
        Instruction::IDiv { dst: 9, lhs: 10, rhs: 11 },
        Instruction::IMod { dst: 12, lhs: 13, rhs: 14 },
        Instruction::IAnd { dst: 0, lhs: 15, rhs: 1 },
        Instruction::IOr { dst: 2, lhs: 3, rhs: 4 },
        Instruction::IXor { dst: 5, lhs: 6, rhs: 7 },
        Instruction::INot { dst: 8, src: 9 },
        Instruction::IShl { dst: 10, src: 11, amt: 5 },
        Instruction::IShr { dst: 12, src: 13, amt: 63 },
        Instruction::ILdi { dst: 0, imm: 0 },
        Instruction::ILdi { dst: 15, imm: -32768 },
        Instruction::ILdi { dst: 1, imm: 32767 },
        Instruction::ILdm { dst: 2, addr: 0 },
        Instruction::ILdm { dst: 3, addr: 65535 },
        Instruction::IStr { src: 4, addr: 1024 },
        Instruction::IEq { dst: 5, lhs: 6, rhs: 7 },
        Instruction::ILt { dst: 8, lhs: 9, rhs: 10 },
        Instruction::IGt { dst: 11, lhs: 12, rhs: 13 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_each_float_op() {
    let cases: Vec<Instruction> = vec![
        Instruction::FAdd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::FSub { dst: 3, lhs: 4, rhs: 5 },
        Instruction::FMul { dst: 6, lhs: 7, rhs: 8 },
        Instruction::FDiv { dst: 9, lhs: 10, rhs: 11 },
        Instruction::FLdi { dst: 0, imm: 100 },
        Instruction::FLdm { dst: 1, addr: 256 },
        Instruction::FStr { src: 2, addr: 512 },
        Instruction::FEq { dst: 3, lhs: 4, rhs: 5 },
        Instruction::FLt { dst: 6, lhs: 7, rhs: 8 },
        Instruction::FGt { dst: 9, lhs: 10, rhs: 11 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_each_complex_op() {
    let cases: Vec<Instruction> = vec![
        Instruction::ZAdd { dst: 0, lhs: 1, rhs: 2 },
        Instruction::ZSub { dst: 3, lhs: 4, rhs: 5 },
        Instruction::ZMul { dst: 6, lhs: 7, rhs: 8 },
        Instruction::ZDiv { dst: 9, lhs: 10, rhs: 11 },
        Instruction::ZLdi { dst: 0, imm_re: 127, imm_im: -128 },
        Instruction::ZLdm { dst: 1, addr: 100 },
        Instruction::ZStr { src: 2, addr: 200 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_conversions() {
    let cases: Vec<Instruction> = vec![
        Instruction::CvtIF { dst_f: 0, src_i: 1 },
        Instruction::CvtFI { dst_i: 2, src_f: 3 },
        Instruction::CvtFZ { dst_z: 4, src_f: 5 },
        Instruction::CvtZF { dst_f: 6, src_z: 7 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_iqcfg() {
    let instr = Instruction::IQCfg { dst: 3 };
    let result = assemble_ok(vec![instr.clone()]);
    let decoded = decode_ok(result.code[0]);
    assert_eq!(decoded, instr);
}

#[test]
fn test_roundtrip_no_operand_instructions() {
    let cases: Vec<Instruction> = vec![
        Instruction::Nop,
        Instruction::Ret,
        Instruction::Halt,
        Instruction::HFork,
        Instruction::HMerge,
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_quantum_ops() {
    let cases: Vec<Instruction> = vec![
        Instruction::QPrep { dst: 0, dist: 0 },
        Instruction::QPrep { dst: 7, dist: 3 },
        Instruction::QKernel { dst: 1, src: 2, kernel: 4, ctx0: 3, ctx1: 4 },
        Instruction::QObserve { dst_h: 0, src_q: 1, mode: 0, ctx0: 0, ctx1: 0 },
        Instruction::QLoad { dst_q: 3, addr: 255 },
        Instruction::QStore { src_q: 7, addr: 0 },
    ];

    for instr in cases {
        let result = assemble_ok(vec![instr.clone()]);
        let decoded = decode_ok(result.code[0]);
        assert_eq!(decoded, instr, "Round-trip failed for {:?}", instr);
    }
}

#[test]
fn test_roundtrip_hreduce() {
    let instr = Instruction::HReduce { src: 2, dst: 5, func: 10 };
    let result = assemble_ok(vec![instr.clone()]);
    let decoded = decode_ok(result.code[0]);
    assert_eq!(decoded, instr);
}

// =============================================================================
// Label resolution tests
// =============================================================================

#[test]
fn test_forward_label_reference() {
    let program = vec![
        Instruction::Jmp { target: "end".to_string() },
        Instruction::Nop,
        Instruction::Label("end".to_string()),
        Instruction::Halt,
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["end"], 2);
    // The JMP word should encode address 2
    let decoded = decode_ok(result.code[0]);
    assert_eq!(decoded, Instruction::Jmp { target: "@2".to_string() });
}

#[test]
fn test_backward_label_reference() {
    let program = vec![
        Instruction::Label("loop".to_string()),
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Jmp { target: "loop".to_string() },
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["loop"], 0);
    let decoded = decode_ok(result.code[2]);
    assert_eq!(decoded, Instruction::Jmp { target: "@0".to_string() });
}

#[test]
fn test_nested_call_ret() {
    let program = vec![
        Instruction::Call { target: "sub".to_string() },
        Instruction::Halt,
        Instruction::Label("sub".to_string()),
        Instruction::ILdi { dst: 0, imm: 99 },
        Instruction::Ret,
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["sub"], 2);
}

#[test]
fn test_jif_label_resolution() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Jif { pred: 0, target: "taken".to_string() },
        Instruction::Halt,
        Instruction::Label("taken".to_string()),
        Instruction::ILdi { dst: 1, imm: 42 },
        Instruction::Halt,
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["taken"], 3);
}

#[test]
fn test_jmpf_label_resolution() {
    let program = vec![
        Instruction::JmpF { flag: 0, target: "branch".to_string() },
        Instruction::Halt,
        Instruction::Label("branch".to_string()),
        Instruction::HMerge,
    ];
    let result = assemble_ok(program);
    assert_eq!(result.labels["branch"], 2);
}

// =============================================================================
// Entry point tests
// =============================================================================

#[test]
fn test_entry_point_no_labels() {
    let result = assemble_ok(vec![Instruction::Halt]);
    assert_eq!(result.entry_point, 0);
}

#[test]
fn test_entry_point_leading_labels() {
    let result = assemble_ok(vec![
        Instruction::Label("a".to_string()),
        Instruction::Label("b".to_string()),
        Instruction::Halt,
    ]);
    assert_eq!(result.entry_point, 2);
}

// =============================================================================
// Error case tests
// =============================================================================

#[test]
fn test_error_undefined_label() {
    let program = vec![Instruction::Jmp { target: "missing".to_string() }];
    let result = assemble(&program);
    assert!(result.is_err(), "Expected error for undefined label");
}

#[test]
fn test_error_duplicate_label() {
    let program = vec![
        Instruction::Label("dup".to_string()),
        Instruction::Label("dup".to_string()),
    ];
    let result = assemble(&program);
    assert!(result.is_err(), "Expected error for duplicate label");
}

// =============================================================================
// Source-level assembly tests
// =============================================================================

#[test]
fn test_assemble_source_basic() {
    let source = "\
ILDI R0, 42
ILDI R1, 10
IADD R2, R0, R1
HALT
";
    let result = assemble_source_ok(source);
    assert_eq!(result.code.len(), 4);
}

#[test]
fn test_assemble_source_with_labels_and_comments() {
    let source = "\
# This is a comment
LABEL: start
ILDI R0, 1       // load counter
LABEL: loop
ISUB R0, R0, R0  # decrement
JIF R0, loop
HALT
";
    let result = assemble_source_ok(source);
    assert!(result.labels.contains_key("start"));
    assert!(result.labels.contains_key("loop"));
}

#[test]
fn test_assemble_source_all_quantum() {
    let source = "\
QPREP Q0, 0
QKERNEL Q1, Q0, 2, R0, R1
QOBSERVE H0, Q1
QLOAD Q2, 10
QSTORE Q3, 20
HALT
";
    let result = assemble_source_ok(source);
    assert_eq!(result.code.len(), 6);
}

// --- Configurable label stripping stress tests ---

/// Assemble with strip_labels=true and assert success.
fn assemble_stripped(instrs: Vec<Instruction>) -> AssemblyResult {
    let opts = AssemblyOptions { strip_labels: true };
    assemble_with_options(&instrs, &opts).expect("stripped assembly should succeed")
}

/// Write an AssemblyResult to .cqb bytes in memory, then read it back.
fn cqb_roundtrip(result: &AssemblyResult, include_debug: bool) -> cqam_as::binary::CqbImage {
    let mut buf = Vec::new();
    write_cqb(&mut buf, result, include_debug).expect("write_cqb should succeed");
    read_cqb(&mut Cursor::new(&buf)).expect("read_cqb should succeed")
}

// -- Round-trip tests: strip -> write_cqb -> read_cqb -> verify ---------------

#[test]
fn test_strip_cqb_roundtrip_no_label_opcodes() {
    // Assemble with stripping, write to .cqb, read back, verify no LABEL opcodes
    let program = vec![
        Instruction::Label("start".to_string()),
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::Label("mid".to_string()),
        Instruction::ILdi { dst: 1, imm: 10 },
        Instruction::Jmp { target: "start".to_string() },
        Instruction::Halt,
    ];
    let result = assemble_stripped(program);
    let image = cqb_roundtrip(&result, false);

    assert_eq!(image.code, result.code);
    assert_eq!(image.entry_point, 0);

    // No word should decode to a Label instruction
    for (i, &word) in image.code.iter().enumerate() {
        let decoded = opcode::decode(word).expect("decode should succeed");
        assert!(
            !matches!(decoded, Instruction::Label(_)),
            "Stripped binary should not contain LABEL opcodes, but word {} decoded to {:?}",
            i,
            decoded,
        );
    }
}

#[test]
fn test_strip_cqb_roundtrip_with_debug_symbols() {
    let program = vec![
        Instruction::Label("alpha".to_string()),
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Label("beta".to_string()),
        Instruction::Halt,
    ];
    let result = assemble_stripped(program);
    let image = cqb_roundtrip(&result, true);

    // Code should match
    assert_eq!(image.code, result.code);
    // Debug symbols should be present even though labels are stripped from code
    let debug = image.debug_symbols.expect("debug symbols should be present");
    assert_eq!(debug.get(&0), Some(&"alpha".to_string()));
    assert_eq!(debug.get(&1), Some(&"beta".to_string()));
}

#[test]
fn test_strip_cqb_roundtrip_code_words_exact() {
    // Verify the stripped code words match between direct assembly and .cqb round-trip
    let program = vec![
        Instruction::Label("loop".to_string()),
        Instruction::ILdi { dst: 0, imm: 5 },
        Instruction::ISub { dst: 0, lhs: 0, rhs: 0 },
        Instruction::Jif { pred: 0, target: "loop".to_string() },
        Instruction::Halt,
    ];
    let result = assemble_stripped(program);
    let image = cqb_roundtrip(&result, false);

    // Every code word should be identical
    assert_eq!(result.code.len(), image.code.len());
    for (i, (&orig, &read)) in result.code.iter().zip(image.code.iter()).enumerate() {
        assert_eq!(orig, read, "Code word mismatch at index {}", i);
    }
}

// -- Verify decoded JMP/CALL/JIF/JMPF targets in stripped output ------------

#[test]
fn test_strip_jmp_decoded_target_correct() {
    // LABEL start  (idx 0, stripped=0)
    // ILDI R0, 1   (idx 1, stripped pos 0)
    // ILDI R1, 2   (idx 2, stripped pos 1)
    // JMP start    (idx 3, stripped pos 2) -- should encode target=0
    let program = vec![
        Instruction::Label("start".to_string()),
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::ILdi { dst: 1, imm: 2 },
        Instruction::Jmp { target: "start".to_string() },
    ];
    let result = assemble_stripped(program);
    assert_eq!(result.code.len(), 3);
    let decoded = decode_ok(result.code[2]);
    assert_eq!(decoded, Instruction::Jmp { target: "@0".to_string() });
}

#[test]
fn test_strip_call_decoded_target_correct() {
    // CALL sub     (idx 0, stripped pos 0) -- should encode target=2
    // HALT         (idx 1, stripped pos 1)
    // LABEL sub    (idx 2, stripped=2)
    // ILDI R0, 99  (idx 3, stripped pos 2)
    // RET          (idx 4, stripped pos 3)
    let program = vec![
        Instruction::Call { target: "sub".to_string() },
        Instruction::Halt,
        Instruction::Label("sub".to_string()),
        Instruction::ILdi { dst: 0, imm: 99 },
        Instruction::Ret,
    ];
    let result = assemble_stripped(program);
    assert_eq!(result.code.len(), 4);
    let decoded = decode_ok(result.code[0]);
    assert_eq!(decoded, Instruction::Call { target: "@2".to_string() });
}

#[test]
fn test_strip_jif_decoded_target_correct() {
    // ILDI R0, 1    (idx 0, stripped pos 0)
    // JIF R0, skip  (idx 1, stripped pos 1) -- target should be stripped addr of "skip"
    // ILDI R1, 0    (idx 2, stripped pos 2)
    // LABEL skip    (idx 3, labels_before=0, stripped=3)
    // HALT          (idx 4, stripped pos 3)
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Jif { pred: 0, target: "skip".to_string() },
        Instruction::ILdi { dst: 1, imm: 0 },
        Instruction::Label("skip".to_string()),
        Instruction::Halt,
    ];
    let result = assemble_stripped(program);
    assert_eq!(result.labels["skip"], 3);
    assert_eq!(result.code.len(), 4);
    let decoded = decode_ok(result.code[1]);
    assert_eq!(
        decoded,
        Instruction::Jif { pred: 0, target: "@3".to_string() }
    );
}

#[test]
fn test_strip_jmpf_decoded_target_correct() {
    // JMPF 0, branch  (idx 0, stripped pos 0) -- target should be stripped addr
    // HALT              (idx 1, stripped pos 1)
    // LABEL branch      (idx 2, labels_before=0, stripped=2)
    // HMERGE            (idx 3, stripped pos 2)
    let program = vec![
        Instruction::JmpF { flag: 0, target: "branch".to_string() },
        Instruction::Halt,
        Instruction::Label("branch".to_string()),
        Instruction::HMerge,
    ];
    let result = assemble_stripped(program);
    assert_eq!(result.labels["branch"], 2);
    assert_eq!(result.code.len(), 3);
    let decoded = decode_ok(result.code[0]);
    assert_eq!(
        decoded,
        Instruction::JmpF { flag: 0, target: "@2".to_string() }
    );
}

// -- Edge case: program with no labels (strip is a no-op) ---------------------

#[test]
fn test_strip_no_labels_is_noop() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::ILdi { dst: 1, imm: 10 },
        Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 },
        Instruction::Halt,
    ];
    let no_strip = assemble(&program).unwrap();
    let stripped = assemble_stripped(program);

    // Code should be identical: no labels to strip
    assert_eq!(no_strip.code, stripped.code);
    assert_eq!(no_strip.labels, stripped.labels);
    assert!(stripped.debug_symbols.is_empty());
    // entry_point: no_strip has 0 (first non-label), stripped always 0
    assert_eq!(stripped.entry_point, 0);
    assert_eq!(no_strip.entry_point, 0);
}

// -- Edge case: all instructions are labels (empty code output) ---------------

#[test]
fn test_strip_all_labels_produces_empty_code() {
    let program = vec![
        Instruction::Label("a".to_string()),
        Instruction::Label("b".to_string()),
        Instruction::Label("c".to_string()),
    ];
    let result = assemble_stripped(program);
    assert!(result.code.is_empty());
    assert_eq!(result.entry_point, 0);
    assert_eq!(result.debug_symbols.len(), 3);
    // All labels should map to stripped addr 0 (pointing past end, but that
    // is valid -- same semantic as non-stripped where they point to the next
    // instruction which doesn't exist)
    assert_eq!(result.labels["a"], 0);
    assert_eq!(result.labels["b"], 0);
    assert_eq!(result.labels["c"], 0);

    // .cqb round-trip should also produce empty code
    let image = cqb_roundtrip(&result, false);
    assert!(image.code.is_empty());
}

// -- Edge case: labels at end of program --------------------------------------

#[test]
fn test_strip_labels_at_end() {
    // Labels at end point past the last instruction (common for "end:" labels)
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Halt,
        Instruction::Label("end".to_string()),
        Instruction::Label("done".to_string()),
    ];
    let result = assemble_stripped(program);
    assert_eq!(result.code.len(), 2);
    // "end" at idx 2, labels_before=0, stripped=2
    // "done" at idx 3, labels_before=1, stripped=2
    assert_eq!(result.labels["end"], 2);
    assert_eq!(result.labels["done"], 2);
}

// -- assemble_source_with_options end-to-end ----------------------------------

#[test]
fn test_assemble_source_with_options_strip() {
    let source = "\
LABEL: start
ILDI R0, 42
JMP start
HALT
";
    let opts = AssemblyOptions { strip_labels: true };
    let result = assemble_source_with_options(source, &opts).unwrap();
    // 4 source instructions, 1 label => 3 code words
    assert_eq!(result.code.len(), 3);
    assert_eq!(result.entry_point, 0);
    assert_eq!(result.labels["start"], 0);
    // Verify the JMP target decodes correctly
    let decoded = decode_ok(result.code[1]);
    assert_eq!(decoded, Instruction::Jmp { target: "@0".to_string() });
}

#[test]
fn test_assemble_source_with_options_no_strip() {
    let source = "\
LABEL: start
ILDI R0, 42
HALT
";
    let opts = AssemblyOptions { strip_labels: false };
    let result = assemble_source_with_options(source, &opts).unwrap();
    // Should be identical to assemble_source
    let baseline = assemble_source(source).unwrap();
    assert_eq!(result.code, baseline.code);
    assert_eq!(result.labels, baseline.labels);
    assert_eq!(result.entry_point, baseline.entry_point);
}

// -- Backward compatibility: wrapper functions unchanged behavior -------------

#[test]
fn test_backward_compat_assemble_wrapper() {
    // assemble() must behave identically to assemble_with_options(default)
    let program = vec![
        Instruction::Label("x".to_string()),
        Instruction::ILdi { dst: 0, imm: 7 },
        Instruction::Jmp { target: "x".to_string() },
        Instruction::Halt,
    ];
    let r1 = assemble(&program).unwrap();
    let r2 = assemble_with_options(&program, &AssemblyOptions::default()).unwrap();
    assert_eq!(r1.code, r2.code);
    assert_eq!(r1.labels, r2.labels);
    assert_eq!(r1.debug_symbols, r2.debug_symbols);
    assert_eq!(r1.entry_point, r2.entry_point);
}

#[test]
fn test_backward_compat_assemble_source_wrapper() {
    let source = "LABEL: y\nILDI R0, 99\nHALT\n";
    let r1 = assemble_source(source).unwrap();
    let r2 = assemble_source_with_options(source, &AssemblyOptions::default()).unwrap();
    assert_eq!(r1.code, r2.code);
    assert_eq!(r1.labels, r2.labels);
    assert_eq!(r1.debug_symbols, r2.debug_symbols);
    assert_eq!(r1.entry_point, r2.entry_point);
}

// -- Complex multi-label program with forward + backward jumps ----------------

#[test]
fn test_strip_complex_program_all_branch_types() {
    // A program exercising JMP, CALL, JIF, JMPF with multiple labels
    // interleaved among code, including forward and backward references.
    //
    // Index  Instruction           labels_before  stripped_pos
    // 0      LABEL: start          0              -> stripped addr 0
    // 1      ILDI R0, 1            -              0
    // 2      JIF R0, skip          -              1  (target: "skip" = stripped 4)
    // 3      CALL sub              -              2  (target: "sub"  = stripped 5)
    // 4      LABEL: skip           1              -> stripped addr 3
    // 5      JMP start             -              3  (target: "start"= stripped 0)
    // 6      LABEL: end            2              -> stripped addr 4
    // 7      HALT                  -              4
    // 8      LABEL: sub            3              -> stripped addr 5
    // 9      ILDI R1, 99           -              5
    // 10     JMPF 0, end         -              6  (target: "end"  = stripped 4)
    // 11     RET                   -              7
    let program = vec![
        Instruction::Label("start".to_string()),
        Instruction::ILdi { dst: 0, imm: 1 },
        Instruction::Jif { pred: 0, target: "skip".to_string() },
        Instruction::Call { target: "sub".to_string() },
        Instruction::Label("skip".to_string()),
        Instruction::Jmp { target: "start".to_string() },
        Instruction::Label("end".to_string()),
        Instruction::Halt,
        Instruction::Label("sub".to_string()),
        Instruction::ILdi { dst: 1, imm: 99 },
        Instruction::JmpF { flag: 0, target: "end".to_string() },
        Instruction::Ret,
    ];

    let result = assemble_stripped(program);
    // 12 total - 4 labels = 8 code words
    assert_eq!(result.code.len(), 8);

    // Verify label addresses
    assert_eq!(result.labels["start"], 0);
    assert_eq!(result.labels["skip"], 3);
    assert_eq!(result.labels["end"], 4);
    assert_eq!(result.labels["sub"], 5);

    // Verify decoded targets
    // code[1] = JIF R0, skip -> target @3
    let jif = decode_ok(result.code[1]);
    assert_eq!(jif, Instruction::Jif { pred: 0, target: "@3".to_string() });

    // code[2] = CALL sub -> target @5
    let call = decode_ok(result.code[2]);
    assert_eq!(call, Instruction::Call { target: "@5".to_string() });

    // code[3] = JMP start -> target @0
    let jmp = decode_ok(result.code[3]);
    assert_eq!(jmp, Instruction::Jmp { target: "@0".to_string() });

    // code[6] = JMPF 0, end -> target @4
    let jmpf = decode_ok(result.code[6]);
    assert_eq!(
        jmpf,
        Instruction::JmpF { flag: 0, target: "@4".to_string() }
    );

    // .cqb round-trip should preserve everything
    let image = cqb_roundtrip(&result, true);
    assert_eq!(image.code, result.code);
    assert_eq!(image.entry_point, 0);
    let debug = image.debug_symbols.expect("debug symbols should be present");
    assert_eq!(debug.len(), 4);
}

// -- Stripped vs non-stripped: same non-label words encode identically ---------

#[test]
fn test_strip_non_branch_words_identical_to_no_strip() {
    // For instructions that do NOT reference labels, the encoded word should
    // be identical regardless of strip_labels setting.
    let program = vec![
        Instruction::Label("x".to_string()),
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::IAdd { dst: 1, lhs: 0, rhs: 0 },
        Instruction::Nop,
        Instruction::Halt,
    ];
    let no_strip = assemble(&program).unwrap();
    let stripped = assemble_stripped(program);

    // Non-label, non-branch words should be identical
    // no_strip: code[0]=LABEL, code[1]=ILDI, code[2]=IADD, code[3]=NOP, code[4]=HALT
    // stripped: code[0]=ILDI, code[1]=IADD, code[2]=NOP, code[3]=HALT
    assert_eq!(no_strip.code[1], stripped.code[0]); // ILDI
    assert_eq!(no_strip.code[2], stripped.code[1]); // IADD
    assert_eq!(no_strip.code[3], stripped.code[2]); // NOP
    assert_eq!(no_strip.code[4], stripped.code[3]); // HALT
}
