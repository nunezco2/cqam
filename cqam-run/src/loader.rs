//! Program loader for the CQAM runner.
//!
//! Provides [`load_program`], which reads either a `.cqam` text-format source
//! file or a `.cqb` binary file from disk and returns a [`ParsedProgram`].

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use cqam_as::binary::read_cqb_file;
use cqam_core::error::CqamError;
use cqam_core::instruction::Instruction;
use cqam_core::opcode;
use cqam_core::parser::{ParsedProgram, ProgramMetadata};

/// Load a CQAM program from a text (`.cqam`) or binary (`.cqb`) file.
///
/// The format is determined by file extension:
/// - `.cqb` — decoded from binary using the opcode decoder
/// - anything else — parsed as text assembly
pub fn load_program(path: &str) -> Result<ParsedProgram, CqamError> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ext == "cqb" {
        load_binary(path)
    } else {
        load_text(path)
    }
}

/// Load a `.cqam` text source file.
fn load_text(path: &str) -> Result<ParsedProgram, CqamError> {
    let content = fs::read_to_string(path)?;
    cqam_core::parser::parse_program(&content)
}

/// Load a `.cqb` binary file.
///
/// Reads the binary image, decodes each instruction word, then resolves
/// `@N` address references in jump/call/setiv targets to the corresponding
/// label names discovered in the decoded instruction stream.
fn load_binary(path: &str) -> Result<ParsedProgram, CqamError> {
    let image = read_cqb_file(Path::new(path))?;

    let empty = HashMap::new();
    let debug_map = image.debug_symbols.as_ref().unwrap_or(&empty);

    let mut instructions = Vec::with_capacity(image.code.len());
    for &word in &image.code {
        let instr = opcode::decode_with_debug(word, debug_map)?;
        instructions.push(instr);
    }

    // Build address → label name map from decoded Label instructions.
    let addr_to_label: HashMap<String, String> = instructions
        .iter()
        .enumerate()
        .filter_map(|(idx, instr)| {
            if let Instruction::Label(name) = instr {
                Some((format!("@{}", idx), name.clone()))
            } else {
                None
            }
        })
        .collect();

    // Patch @N targets in jump/call/setiv instructions to use label names.
    for instr in &mut instructions {
        patch_target(instr, &addr_to_label);
    }

    Ok(ParsedProgram {
        instructions,
        metadata: ProgramMetadata::default(),
        data_section: Default::default(),
        shared_section: Default::default(),
        private_section: Default::default(),
    })
}

/// Replace `@N` address targets with the corresponding label name.
fn patch_target(instr: &mut Instruction, map: &HashMap<String, String>) {
    match instr {
        Instruction::Jmp { target }
        | Instruction::Call { target }
        | Instruction::SetIV { target, .. } => {
            if let Some(name) = map.get(target.as_str()) {
                *target = name.clone();
            }
        }
        Instruction::Jif { target, .. }
        | Instruction::JmpF { target, .. } => {
            if let Some(name) = map.get(target.as_str()) {
                *target = name.clone();
            }
        }
        _ => {}
    }
}
