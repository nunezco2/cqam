// cqam-as/src/assembler.rs
//
// Phase 5: Two-pass assembler for the CQAM ISA.
//
// Pass 1: Scan for labels, assign each a word address and a numeric ID,
//         and build the symbol table.
// Pass 2: Encode each instruction into a u32 word using the symbol table
//         to resolve label references.

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
#[derive(Debug, Clone)]
pub struct AssemblyResult {
    /// The encoded instruction words (one u32 per instruction, including
    /// label pseudo-instructions).
    pub code: Vec<u32>,

    /// Map from label name to word address (0-based).
    ///
    /// Populated during pass 1. Used during pass 2 for encoding, and
    /// retained for debug output.
    pub labels: HashMap<String, u32>,

    /// Debug symbol table: (numeric_id, label_name) pairs.
    ///
    /// The numeric ID is assigned sequentially during pass 1 (0, 1, 2, ...).
    /// Written to the optional debug section of `.cqb` files.
    pub debug_symbols: Vec<(u16, String)>,

    /// Word offset of the first instruction that is not a Label.
    ///
    /// This becomes the `entry_point` field in the `.cqb` header.
    /// Programs that begin with labels will have `entry_point > 0`.
    pub entry_point: u16,
}

// =============================================================================
// Public API
// =============================================================================

/// Assemble a sequence of parsed instructions into binary.
///
/// This is the core assembly function. It performs two passes:
///
/// 1. **Pass 1 (scan):** Iterate over all instructions. Maintain a word
///    address counter starting at 0. For each `Label(name)`:
///    - If `name` is already in the symbol table, return
///      `CqamError::DuplicateLabel`.
///    - Otherwise, record `name -> current_address` and assign a sequential
///      numeric label ID.
///    - Increment the address counter for every instruction (labels occupy
///      a word slot).
///      Also track the first non-label instruction for `entry_point`.
///
/// 2. **Pass 2 (encode):** Iterate again. For each instruction, call
///    `opcode::encode(instr, &label_map)` to produce the u32 word.
///    Label instructions are encoded with their numeric ID in the L-format.
///
/// # Errors
///
/// - `CqamError::DuplicateLabel` if a label name is defined more than once.
/// - `CqamError::UnresolvedLabel` if a jump/call references an undefined label.
/// - `CqamError::AddressOverflow` if a conditional branch target exceeds 16 bits.
/// - `CqamError::OperandOverflow` for out-of-range register indices, etc.
///
/// # Examples
///
/// ```
/// use cqam_core::instruction::Instruction;
/// use cqam_as::assembler::assemble;
///
/// let program = vec![
///     Instruction::ILdi { dst: 0, imm: 42 },
///     Instruction::Halt,
/// ];
/// let result = assemble(&program).unwrap();
/// assert_eq!(result.code.len(), 2);
/// assert_eq!(result.entry_point, 0);
/// ```
pub fn assemble(instructions: &[Instruction]) -> Result<AssemblyResult, CqamError> {
    // -- Pass 1: build symbol table ------------------------------------------
    let mut labels: HashMap<String, u32> = HashMap::new();
    let mut debug_symbols: Vec<(u16, String)> = Vec::new();
    let mut label_id_counter: u16 = 0;
    let mut entry_point: Option<u16> = None;

    for (idx, instr) in instructions.iter().enumerate() {
        let word_addr = idx as u32;

        if let Instruction::Label(name) = instr {
            if labels.contains_key(name) {
                return Err(CqamError::DuplicateLabel {
                    name: name.clone(),
                    first: labels[name],
                    second: word_addr,
                });
            }
            labels.insert(name.clone(), word_addr);
            debug_symbols.push((label_id_counter, name.clone()));
            label_id_counter += 1;
        } else if entry_point.is_none() {
            entry_point = Some(word_addr as u16);
        }
    }

    let entry_point = entry_point.unwrap_or(0);

    // -- Pass 2: encode instructions -----------------------------------------
    let mut code = Vec::with_capacity(instructions.len());

    // We need to map label names to their numeric IDs for L-format encoding.
    let label_name_to_id: HashMap<String, u16> = debug_symbols
        .iter()
        .map(|(id, name)| (name.clone(), *id))
        .collect();

    for instr in instructions {
        let word = match instr {
            Instruction::Label(name) => {
                // Encode as L-format with the label's numeric ID.
                let lid = label_name_to_id[name];
                opcode::encode_label(lid)
            }
            _ => opcode::encode(instr, &labels)?,
        };
        code.push(word);
    }

    Ok(AssemblyResult {
        code,
        labels,
        debug_symbols,
        entry_point,
    })
}

/// Convenience function: parse source text and assemble in one step.
///
/// Equivalent to calling `parser::parse_program(source)` followed by
/// `assemble(&instructions)`.
///
/// # Errors
///
/// Returns parse errors from `parser::parse_program` or assembly errors
/// from `assemble`.
pub fn assemble_source(source: &str) -> Result<AssemblyResult, CqamError> {
    let instructions = parser::parse_program(source)?;
    assemble(&instructions)
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
}
