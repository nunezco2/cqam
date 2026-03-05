//! Assembler and disassembler for the CQAM binary (`.cqb`) format.
//!
//! `cqam-as` converts between three representations:
//!
//! 1. **Text source** (`.cqam`) — parsed by `cqam-core` into `Vec<Instruction>`.
//! 2. **Binary image** (`.cqb`) — 32-bit little-endian instruction words preceded
//!    by a 12-byte header and an optional debug section.
//! 3. **Disassembled text** — human-readable dump of instruction words.
//!
//! # Key types
//!
//! | Module | Key type / function | Purpose |
//! |--------|---------------------|---------|
//! | [`assembler`] | [`assemble_source`] | Parse + assemble in one step |
//! | [`assembler`] | [`assemble`] | Assemble a `Vec<Instruction>` |
//! | [`assembler`] | [`AssemblyResult`] | Encoded words + label map + debug |
//! | [`assembler`] | [`AssemblyOptions`] | Toggle label stripping |
//! | [`binary`] | [`write_cqb`] / [`read_cqb`] | Serialise / deserialise `.cqb` |
//! | [`binary`] | [`CqbImage`] | Loaded binary image |
//! | [`disassembler`] | [`disassemble`] | Decode `Vec<u32>` to text |
//!
//! # Usage
//!
//! ```
//! use cqam_as::{assemble_source, write_cqb};
//!
//! let result = assemble_source("ILDI R0, 42\nHALT\n").unwrap();
//! assert_eq!(result.code.len(), 2);
//!
//! let mut buf: Vec<u8> = Vec::new();
//! write_cqb(&mut buf, &result, false).unwrap();
//! assert!(buf.starts_with(b"CQAM"));
//! ```

pub mod assembler;
pub mod binary;
pub mod disassembler;

// Re-export primary types and functions for convenience.
pub use assembler::{
    assemble, assemble_source,
    assemble_with_options, assemble_source_with_options,
    AssemblyOptions, AssemblyResult,
};
pub use binary::{read_cqb, read_cqb_file, write_cqb, write_cqb_file, CqbImage};
pub use disassembler::{disassemble, disassemble_one};
