//! QASM output structural validation: pure-Rust checks that generated
//! OpenQASM 3.0 is well-formed (correct headers, declarations, and body).

use cqam_core::parser::parse_program;
use cqam_codegen::qasm::{EmitConfig, emit_qasm_program};

const ARITHMETIC_SOURCE: &str = "\
ILDI R0, 10
ILDI R1, 25
IADD R2, R0, R1
HALT
";

const QUANTUM_SOURCE: &str = "\
ILDI R0, 0
ILDI R1, 0
QPREP Q0, 0
QKERNEL ENTG, Q1, Q0, R0, R1
QOBSERVE H0, Q1
HREDUCE MODEV, H0, R2
HALT
";

fn emit_standalone(source: &str) -> String {
    let program = parse_program(source).expect("test source should parse").instructions;
    let config = EmitConfig::default();
    emit_qasm_program(&program, &config)
}

fn emit_fragment(source: &str) -> String {
    let program = parse_program(source).expect("test source should parse").instructions;
    let config = EmitConfig::fragment();
    emit_qasm_program(&program, &config)
}

#[test]
fn test_qasm_header_present() {
    let output = emit_standalone(ARITHMETIC_SOURCE);
    let first_line = output.lines()
        .find(|l| !l.trim().is_empty())
        .expect("Output should have non-empty lines");
    assert!(
        first_line.trim().starts_with("OPENQASM 3.0"),
        "First non-empty line should be OPENQASM header, got: {}", first_line
    );
}

#[test]
fn test_qasm_includes_stdgates() {
    let output = emit_standalone(ARITHMETIC_SOURCE);
    assert!(
        output.contains("stdgates.inc"),
        "Output should include stdgates.inc"
    );
}

#[test]
fn test_qasm_lines_properly_terminated() {
    let output = emit_standalone(QUANTUM_SOURCE);
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        let last = trimmed.chars().last().unwrap();
        assert!(
            last == ';' || last == '{' || last == '}',
            "Non-comment line should end with ';', '{{', or '}}', got: '{}'", trimmed
        );
    }
}

#[test]
fn test_qasm_balanced_braces() {
    let output = emit_standalone(QUANTUM_SOURCE);
    let opens = output.chars().filter(|&c| c == '{').count();
    let closes = output.chars().filter(|&c| c == '}').count();
    assert_eq!(opens, closes, "Braces should be balanced: {{ {} != }} {}", opens, closes);
}

#[test]
fn test_qasm_declarations_valid_types() {
    let output = emit_standalone(QUANTUM_SOURCE);
    let valid_types = ["int[64]", "float[64]", "qubit[16]", "bit[16]"];

    for line in output.lines() {
        let trimmed = line.trim();
        // Check lines that look like declarations (type keyword followed by identifier)
        for vt in &valid_types {
            if trimmed.starts_with(vt) {
                // This is a valid declaration line
                assert!(trimmed.ends_with(';'), "Declaration should end with ';': {}", trimmed);
            }
        }
    }
}

#[test]
fn test_qasm_no_raw_cqam_opcodes() {
    let output = emit_standalone(QUANTUM_SOURCE);
    let cqam_opcodes = [
        "ILDI", "IADD", "ISUB", "IMUL", "IDIV", "IMOD",
        "QPREP", "QKERNEL", "QOBSERVE", "HFORK", "HMERGE",
        "JMPF", "HREDUCE", "RETI", "SETIV",
    ];

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue; // CQAM opcodes may appear in comments
        }
        for opcode in &cqam_opcodes {
            // Check that the opcode doesn't appear as a standalone word
            for word in trimmed.split_whitespace() {
                let clean = word.trim_end_matches(|c: char| !c.is_alphanumeric());
                assert_ne!(
                    clean, *opcode,
                    "Raw CQAM opcode '{}' found on non-comment line: {}", opcode, trimmed
                );
            }
        }
    }
}

#[test]
fn test_qasm_fragment_mode_no_header() {
    let output = emit_fragment(ARITHMETIC_SOURCE);
    assert!(
        !output.contains("OPENQASM"),
        "Fragment mode should not contain OPENQASM header"
    );
    // Fragment should not have type declarations
    assert!(
        !output.contains("int[64]"),
        "Fragment mode should not contain register declarations"
    );
}

// --- Additional QASM validation tests ---

const HYBRID_SOURCE: &str = "\
ILDI R0, 42
HFORK
ILDI R1, 100
HMERGE
HALT
";

#[test]
fn test_qasm_hybrid_instructions_emitted_as_comments() {
    let output = emit_standalone(HYBRID_SOURCE);
    // HFORK and HMERGE have no QASM equivalent; they should be emitted as comments
    // and not appear as raw opcodes on non-comment lines.
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        for opcode in &["HFORK", "HMERGE"] {
            for word in trimmed.split_whitespace() {
                let clean = word.trim_end_matches(|c: char| !c.is_alphanumeric());
                assert_ne!(
                    clean, *opcode,
                    "Raw CQAM opcode '{}' found on non-comment line: {}", opcode, trimmed
                );
            }
        }
    }
}

#[test]
fn test_qasm_all_lines_are_valid_utf8_no_control_chars() {
    let output = emit_standalone(QUANTUM_SOURCE);
    for (i, line) in output.lines().enumerate() {
        for ch in line.chars() {
            assert!(
                !ch.is_control() || ch == '\t',
                "Line {} contains unexpected control character: {:?}", i + 1, ch
            );
        }
    }
}

#[test]
fn test_qasm_no_duplicate_declarations() {
    // Each register should be declared at most once.
    let output = emit_standalone(ARITHMETIC_SOURCE);
    let int_decls: Vec<&str> = output.lines()
        .map(|l| l.trim())
        .filter(|l| l.starts_with("int[64]"))
        .collect();
    // Check for duplicates: each declaration line should be unique.
    let unique_count = int_decls.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(
        int_decls.len(), unique_count,
        "Found duplicate int[64] declarations: {:?}", int_decls
    );
}
