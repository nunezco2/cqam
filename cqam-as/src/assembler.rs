// cqam-as/src/assembler.rs
//
// Phase 5/7: Two-pass assembler for the CQAM ISA.
//
// Pass 1: Scan for labels, assign each a word address and a numeric ID,
//         and build the symbol table.
// Pass 2: Encode each instruction into a u32 word using the symbol table
//         to resolve label references.
//
// Phase 7 adds configurable label stripping via AssemblyOptions.

use std::collections::HashMap;

use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;
use cqam_core::opcode;
use cqam_core::parser;

// =============================================================================
// Public types
// =============================================================================

/// The result of a successful assembly.
///
/// Contains the encoded binary, label metadata, and debug information
/// needed to write a `.cqb` file.
///
/// When `strip_labels` is enabled:
/// - `code` contains only non-label instruction words.
/// - `labels` maps label names to their *stripped* word addresses.
/// - `debug_symbols` is still populated for debug section output.
/// - `entry_point` is always 0.
#[derive(Debug, Clone)]
pub struct AssemblyResult {
    /// The encoded instruction words.
    ///
    /// When assembled without stripping, includes label pseudo-instruction
    /// words (one u32 per instruction, including labels).
    /// When assembled with stripping, contains only non-label words.
    pub code: Vec<u32>,

    /// Map from label name to word address (0-based).
    ///
    /// When assembled without stripping, addresses are original indices.
    /// When assembled with stripping, addresses are stripped (recomputed)
    /// indices that account for removed label words.
    pub labels: HashMap<String, u32>,

    /// Debug symbol table: (numeric_id, label_name) pairs.
    ///
    /// The numeric ID is assigned sequentially during pass 1 (0, 1, 2, ...).
    /// Written to the optional debug section of `.cqb` files.
    /// Populated regardless of strip_labels setting.
    pub debug_symbols: Vec<(u16, String)>,

    /// Word offset of the first executable instruction.
    ///
    /// When assembled without stripping: index of first non-label instruction.
    /// When assembled with stripping: always 0 (labels are removed, so the
    /// first word is always a non-label instruction).
    pub entry_point: u16,
}

/// Options controlling assembler behavior.
///
/// Use `Default::default()` for standard assembly (labels retained in the
/// code stream, identical to Phase 5 behavior).
#[derive(Debug, Clone, Default)]
pub struct AssemblyOptions {
    /// If true, label pseudo-instructions are removed from the output
    /// code stream. All jump/call targets are rewritten to stripped
    /// addresses (original index minus count of preceding labels).
    ///
    /// Debug symbols are preserved regardless of this setting.
    pub strip_labels: bool,
}

// =============================================================================
// Public API
// =============================================================================

/// Assemble a sequence of parsed instructions with configurable options.
///
/// When `options.strip_labels` is true:
/// - Label pseudo-instructions are removed from the output code stream.
/// - Jump/call targets are resolved to stripped addresses.
/// - The entry_point is always 0.
/// - Debug symbols are still populated for use with --debug.
///
/// When `options.strip_labels` is false:
/// - Behavior is identical to the original `assemble()`.
///
/// # Errors
///
/// - `CqamError::DuplicateLabel` if a label name is defined more than once.
/// - `CqamError::UnresolvedLabel` if a jump/call references an undefined label.
/// - `CqamError::AddressOverflow` if a conditional branch target exceeds 16 bits.
/// - `CqamError::OperandOverflow` for out-of-range register indices, etc.
pub fn assemble_with_options(
    instructions: &[Instruction],
    options: &AssemblyOptions,
) -> Result<AssemblyResult, CqamError> {
    // -- Pass 1: build symbol table ------------------------------------------
    let mut labels: HashMap<String, u32> = HashMap::new();
    let mut debug_symbols: Vec<(u16, String)> = Vec::new();
    let mut label_id_counter: u16 = 0;
    let mut entry_point: Option<u16> = None;
    let mut labels_before: u32 = 0;

    for (idx, instr) in instructions.iter().enumerate() {
        let word_addr = idx as u32;

        if let Instruction::Label(name) = instr {
            if labels.contains_key(name) {
                let existing = labels[name];
                return Err(CqamError::DuplicateLabel {
                    name: name.clone(),
                    first: existing,
                    second: if options.strip_labels {
                        word_addr - labels_before
                    } else {
                        word_addr
                    },
                });
            }

            let addr = if options.strip_labels {
                // stripped_label_addr = index - labels_before (count at indices < i)
                word_addr - labels_before
            } else {
                word_addr
            };

            labels.insert(name.clone(), addr);
            debug_symbols.push((label_id_counter, name.clone()));
            label_id_counter += 1;
            labels_before += 1;
        } else if !options.strip_labels && entry_point.is_none() {
            entry_point = Some(word_addr as u16);
        }
    }

    let entry_point = if options.strip_labels {
        0 // All labels removed, first word is always a non-label
    } else {
        entry_point.unwrap_or(0)
    };

    // -- Pass 2: encode instructions -----------------------------------------

    // We need label name->numeric ID mapping for L-format encoding.
    let label_name_to_id: HashMap<String, u16> = debug_symbols
        .iter()
        .map(|(id, name)| (name.clone(), *id))
        .collect();

    let capacity = if options.strip_labels {
        instructions.len() - labels_before as usize
    } else {
        instructions.len()
    };
    let mut code = Vec::with_capacity(capacity);

    for instr in instructions {
        if options.strip_labels {
            if matches!(instr, Instruction::Label(_)) {
                continue; // Skip label words in stripped mode
            }
            let word = opcode::encode(instr, &labels)?;
            code.push(word);
        } else {
            let word = match instr {
                Instruction::Label(name) => {
                    let lid = label_name_to_id[name];
                    opcode::encode_label(lid)
                }
                _ => opcode::encode(instr, &labels)?,
            };
            code.push(word);
        }
    }

    Ok(AssemblyResult {
        code,
        labels,
        debug_symbols,
        entry_point,
    })
}

/// Convenience: parse source text and assemble with options.
///
/// Equivalent to calling `parser::parse_program(source)` followed by
/// `assemble_with_options(&instructions, options)`.
pub fn assemble_source_with_options(
    source: &str,
    options: &AssemblyOptions,
) -> Result<AssemblyResult, CqamError> {
    let instructions = parser::parse_program(source)?;
    assemble_with_options(&instructions, options)
}

/// Assemble a sequence of parsed instructions into binary.
///
/// Wrapper around `assemble_with_options` with default options (labels
/// retained in the code stream).
///
/// Signature unchanged from Phase 5. All existing call sites continue
/// to work without modification.
pub fn assemble(instructions: &[Instruction]) -> Result<AssemblyResult, CqamError> {
    assemble_with_options(instructions, &AssemblyOptions::default())
}

/// Parse source text and assemble with default options.
///
/// Wrapper around `assemble_source_with_options` with default options.
///
/// Signature unchanged from Phase 5.
pub fn assemble_source(source: &str) -> Result<AssemblyResult, CqamError> {
    assemble_source_with_options(source, &AssemblyOptions::default())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assemble_empty_program() {
        let result = assemble(&[]).unwrap();
        assert!(result.code.is_empty());
        assert_eq!(result.entry_point, 0);
    }

    #[test]
    fn test_assemble_single_instruction() {
        let program = vec![Instruction::Halt];
        let result = assemble(&program).unwrap();
        assert_eq!(result.code.len(), 1);
        assert_eq!(result.entry_point, 0);
    }

    #[test]
    fn test_assemble_with_labels() {
        let program = vec![
            Instruction::Label("start".to_string()),
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::Jmp { target: "start".to_string() },
            Instruction::Halt,
        ];
        let result = assemble(&program).unwrap();
        assert_eq!(result.code.len(), 4);
        assert_eq!(result.labels["start"], 0);
        assert_eq!(result.entry_point, 1); // first non-label is at word 1
    }

    #[test]
    fn test_assemble_duplicate_label() {
        let program = vec![
            Instruction::Label("dup".to_string()),
            Instruction::Nop,
            Instruction::Label("dup".to_string()),
        ];
        let result = assemble(&program);
        assert!(result.is_err());
    }

    #[test]
    fn test_assemble_undefined_label() {
        let program = vec![
            Instruction::Jmp { target: "missing".to_string() },
        ];
        let result = assemble(&program);
        assert!(result.is_err());
    }

    #[test]
    fn test_assemble_source_roundtrip() {
        let source = "ILDI R0, 42\nHALT\n";
        let result = assemble_source(source).unwrap();
        assert_eq!(result.code.len(), 2);
    }

    // =========================================================================
    // Phase 7: AssemblyOptions and label stripping tests
    // =========================================================================

    #[test]
    fn test_no_strip_is_default() {
        let opts = AssemblyOptions::default();
        assert!(!opts.strip_labels);
    }

    #[test]
    fn test_assemble_equals_assemble_with_default_options() {
        let program = vec![
            Instruction::Label("start".to_string()),
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::Jmp { target: "start".to_string() },
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
    fn test_strip_removes_label_words() {
        let program = vec![
            Instruction::Label("start".to_string()),
            Instruction::ILdi { dst: 0, imm: 42 },
            Instruction::Halt,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        // 3 instructions, 1 label → 2 code words
        assert_eq!(result.code.len(), 2);
    }

    #[test]
    fn test_strip_preserves_non_label_count() {
        let program = vec![
            Instruction::Label("a".to_string()),
            Instruction::Label("b".to_string()),
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::Label("c".to_string()),
            Instruction::ILdi { dst: 1, imm: 2 },
            Instruction::Halt,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        // 6 total - 3 labels = 3 code words
        assert_eq!(result.code.len(), 3);
    }

    #[test]
    fn test_strip_jump_targets_resolved() {
        let program = vec![
            Instruction::Label("start".to_string()),
            Instruction::ILdi { dst: 0, imm: 42 },
            Instruction::Jmp { target: "start".to_string() },
            Instruction::Halt,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        // "start" should resolve to stripped addr 0
        assert_eq!(result.labels["start"], 0);
        // Decode JMP (second word in stripped output) - target should be @0
        let decoded = opcode::decode(result.code[1]).unwrap();
        assert_eq!(decoded, Instruction::Jmp { target: "@0".to_string() });
    }

    #[test]
    fn test_strip_conditional_branch_targets() {
        let program = vec![
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::Jif { pred: 0, target: "taken".to_string() },
            Instruction::Halt,
            Instruction::Label("taken".to_string()),
            Instruction::ILdi { dst: 1, imm: 42 },
            Instruction::Halt,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        // "taken" at original index 3, labels_before=0, stripped = 3-0 = 3
        // But wait: labels_before for label at index 3 is 0 (no labels before it)
        // stripped_addr = 3 - 0 = 3
        // After stripping: 5 non-label words at indices 0..4
        // "taken" should point to stripped addr 3
        assert_eq!(result.labels["taken"], 3);
        assert_eq!(result.code.len(), 5);
    }

    #[test]
    fn test_strip_consecutive_labels() {
        let program = vec![
            Instruction::Label("a".to_string()),
            Instruction::Label("b".to_string()),
            Instruction::Label("c".to_string()),
            Instruction::Nop,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        // All three labels should resolve to stripped addr 0
        assert_eq!(result.labels["a"], 0);
        assert_eq!(result.labels["b"], 0);
        assert_eq!(result.labels["c"], 0);
        // Only 1 code word (the NOP)
        assert_eq!(result.code.len(), 1);
    }

    #[test]
    fn test_strip_entry_point_is_zero() {
        let program = vec![
            Instruction::Label("start".to_string()),
            Instruction::Label("begin".to_string()),
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::Halt,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        assert_eq!(result.entry_point, 0);
    }

    #[test]
    fn test_strip_debug_symbols_preserved() {
        let program = vec![
            Instruction::Label("alpha".to_string()),
            Instruction::ILdi { dst: 0, imm: 1 },
            Instruction::Label("beta".to_string()),
            Instruction::Halt,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        assert_eq!(result.debug_symbols.len(), 2);
        assert_eq!(result.debug_symbols[0], (0, "alpha".to_string()));
        assert_eq!(result.debug_symbols[1], (1, "beta".to_string()));
    }

    #[test]
    fn test_strip_label_map_has_stripped_addrs() {
        // Index 0: LABEL: start   -> labels_before=0, stripped=0
        // Index 1: NOP
        // Index 2: LABEL: mid     -> labels_before=1, stripped=1
        // Index 3: NOP
        // Index 4: LABEL: end     -> labels_before=2, stripped=2
        // Index 5: HALT
        let program = vec![
            Instruction::Label("start".to_string()),
            Instruction::Nop,
            Instruction::Label("mid".to_string()),
            Instruction::Nop,
            Instruction::Label("end".to_string()),
            Instruction::Halt,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        assert_eq!(result.labels["start"], 0);
        assert_eq!(result.labels["mid"], 1);
        assert_eq!(result.labels["end"], 2);
        assert_eq!(result.code.len(), 3); // 3 non-label instructions
    }

    #[test]
    fn test_strip_empty_program() {
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&[], &opts).unwrap();
        assert!(result.code.is_empty());
        assert_eq!(result.entry_point, 0);
    }

    #[test]
    fn test_strip_only_labels() {
        let program = vec![
            Instruction::Label("a".to_string()),
            Instruction::Label("b".to_string()),
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        assert!(result.code.is_empty());
        assert_eq!(result.entry_point, 0);
        assert_eq!(result.debug_symbols.len(), 2);
    }

    #[test]
    fn test_strip_call_target_resolved() {
        let program = vec![
            Instruction::Call { target: "sub".to_string() },
            Instruction::Halt,
            Instruction::Label("sub".to_string()),
            Instruction::ILdi { dst: 0, imm: 99 },
            Instruction::Ret,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        // "sub" at index 2, labels_before=0, stripped=2
        assert_eq!(result.labels["sub"], 2);
        assert_eq!(result.code.len(), 4); // 5 - 1 label
    }

    #[test]
    fn test_strip_hcexec_target_resolved() {
        let program = vec![
            Instruction::HCExec { flag: 0, target: "branch".to_string() },
            Instruction::Halt,
            Instruction::Label("branch".to_string()),
            Instruction::HMerge,
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts).unwrap();
        // "branch" at index 2, labels_before=0, stripped=2
        assert_eq!(result.labels["branch"], 2);
        assert_eq!(result.code.len(), 3);
    }

    #[test]
    fn test_strip_duplicate_label_error() {
        let program = vec![
            Instruction::Label("dup".to_string()),
            Instruction::Nop,
            Instruction::Label("dup".to_string()),
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts);
        assert!(result.is_err());
    }

    #[test]
    fn test_strip_undefined_label_error() {
        let program = vec![
            Instruction::Jmp { target: "missing".to_string() },
        ];
        let opts = AssemblyOptions { strip_labels: true };
        let result = assemble_with_options(&program, &opts);
        assert!(result.is_err());
    }
}
