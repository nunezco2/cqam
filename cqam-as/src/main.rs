// cqam-as/src/main.rs
//
// Phase 5/7: CLI entry point for the CQAM assembler/disassembler.
//
// Usage:
//   cqam-as --assemble input.cqam [-o output.cqb] [--debug] [--strip]
//   cqam-as --disassemble input.cqb [-o output.cqam]

use std::path::{Path, PathBuf};
use std::process;

use cqam_as::assembler::{self, AssemblyOptions};
use cqam_as::binary;
use cqam_as::disassembler;
use cqam_core::error::CqamError;

// =============================================================================
// CLI argument parsing (manual, no external dependency)
// =============================================================================

/// Parsed command-line arguments.
struct CliArgs {
    /// Operating mode: assemble or disassemble.
    mode: Mode,
    /// Input file path.
    input: PathBuf,
    /// Output file path (derived from input if not specified).
    output: PathBuf,
    /// Whether to include debug symbols in the .cqb output.
    include_debug: bool,
    /// Whether to strip label pseudo-instructions from the binary.
    /// Only meaningful in Assemble mode. Default: false.
    strip_labels: bool,
}

/// The two operating modes.
enum Mode {
    Assemble,
    Disassemble,
}

/// Parse CLI arguments from `std::env::args()`.
///
/// Returns `Err(String)` with a usage message on invalid arguments.
fn parse_args() -> Result<CliArgs, String> {
    let args: Vec<String> = std::env::args().collect();

    let mut mode: Option<Mode> = None;
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut include_debug = false;
    let mut strip_labels = false;

    let mut i = 1; // skip program name
    while i < args.len() {
        match args[i].as_str() {
            "--assemble" => {
                if mode.is_some() {
                    return Err("Cannot specify both --assemble and --disassemble".to_string());
                }
                mode = Some(Mode::Assemble);
                // Next arg should be the input file (if not another flag)
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    i += 1;
                    input = Some(PathBuf::from(&args[i]));
                }
            }
            "--disassemble" => {
                if mode.is_some() {
                    return Err("Cannot specify both --assemble and --disassemble".to_string());
                }
                mode = Some(Mode::Disassemble);
                // Next arg should be the input file (if not another flag)
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    i += 1;
                    input = Some(PathBuf::from(&args[i]));
                }
            }
            "-o" => {
                if i + 1 >= args.len() {
                    return Err("-o requires an output path argument".to_string());
                }
                i += 1;
                output = Some(PathBuf::from(&args[i]));
            }
            "--debug" => {
                include_debug = true;
            }
            "--strip" => {
                strip_labels = true;
            }
            other => {
                // If we haven't got an input file yet, treat this as the input
                if input.is_none() {
                    input = Some(PathBuf::from(other));
                } else {
                    return Err(format!("Unexpected argument: {}", other));
                }
            }
        }
        i += 1;
    }

    let mode = mode.ok_or("Must specify --assemble or --disassemble")?;
    let input = input.ok_or("Must specify an input file")?;
    let output = output.unwrap_or_else(|| default_output_path(&input, &mode));

    Ok(CliArgs {
        mode,
        input,
        output,
        include_debug,
        strip_labels,
    })
}

/// Derive a default output path from the input path and mode.
///
/// - Assemble: `input.cqam` -> `input.cqb`
/// - Disassemble: `input.cqb` -> `input.cqam`
fn default_output_path(input: &Path, mode: &Mode) -> PathBuf {
    let stem = input.file_stem().unwrap_or_default();
    let parent = input.parent().unwrap_or(Path::new("."));
    match mode {
        Mode::Assemble => parent.join(format!("{}.cqb", stem.to_string_lossy())),
        Mode::Disassemble => parent.join(format!("{}.cqam", stem.to_string_lossy())),
    }
}

// =============================================================================
// Main
// =============================================================================

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("Error: {}", msg);
            eprintln!();
            print_usage();
            process::exit(1);
        }
    };

    let result = match args.mode {
        Mode::Assemble => run_assemble(
            &args.input,
            &args.output,
            args.include_debug,
            args.strip_labels,
        ),
        Mode::Disassemble => run_disassemble(&args.input, &args.output),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

/// Run the assembler pipeline: read .cqam -> parse -> assemble -> write .cqb.
fn run_assemble(
    input: &Path,
    output: &Path,
    include_debug: bool,
    strip_labels: bool,
) -> Result<(), CqamError> {
    let source = std::fs::read_to_string(input)?;
    let options = AssemblyOptions { strip_labels };
    let result = assembler::assemble_source_with_options(&source, &options)?;

    if strip_labels {
        eprintln!(
            "Assembled {} instructions ({} labels stripped, entry at word {})",
            result.code.len(),
            result.debug_symbols.len(),
            result.entry_point,
        );
    } else {
        eprintln!(
            "Assembled {} instructions ({} labels, entry at word {})",
            result.code.len(),
            result.debug_symbols.len(),
            result.entry_point,
        );
    }

    binary::write_cqb_file(output, &result, include_debug)?;

    eprintln!(
        "Wrote {} bytes to {}",
        // Header (12) + code words (N*4) + optional debug
        12 + result.code.len() * 4,
        output.display(),
    );

    Ok(())
}

/// Run the disassembler pipeline: read .cqb -> decode -> write .cqam text.
fn run_disassemble(input: &Path, output: &Path) -> Result<(), CqamError> {
    let image = binary::read_cqb_file(input)?;

    eprintln!(
        "Loaded {} instruction words (entry at word {}, version {})",
        image.code.len(),
        image.entry_point,
        image.version,
    );

    let text = disassembler::disassemble(
        &image.code,
        image.debug_symbols.as_ref(),
    )?;

    std::fs::write(output, text.as_bytes())?;

    eprintln!("Wrote disassembly to {}", output.display());

    Ok(())
}

/// Print usage information to stderr.
fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  cqam-as --assemble <input.cqam> [-o <output.cqb>] [--debug] [--strip]");
    eprintln!("  cqam-as --disassemble <input.cqb> [-o <output.cqam>]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --assemble      Assemble .cqam text to .cqb binary");
    eprintln!("  --disassemble   Disassemble .cqb binary to .cqam text");
    eprintln!("  -o <path>       Output file path (default: derived from input)");
    eprintln!("  --debug         Include debug symbol table in .cqb output");
    eprintln!("  --strip         Remove label pseudo-instructions from binary output");
}
