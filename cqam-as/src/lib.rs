// cqam-as/src/lib.rs
//
// Phase 5: Assembler and disassembler for the CQAM binary format.
//
// Public API re-exports for library consumers (cqam-run, tests, etc.).

pub mod assembler;
pub mod binary;
pub mod disassembler;

// Re-export primary types and functions for convenience.
pub use assembler::{assemble, assemble_source, AssemblyResult};
pub use binary::{read_cqb, read_cqb_file, write_cqb, write_cqb_file, CqbImage};
pub use disassembler::{disassemble, disassemble_one};
