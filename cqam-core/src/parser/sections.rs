//! Section and pragma parsers for `.data`, `.shared`, `.private`, and `#!` directives.
//!
//! Also contains data directive helpers (`.ascii`, `.i64`, `.f64`, `.c64`) and
//! label-substitution functions (`substitute_data_refs`, `substitute_shared_refs`).

use super::types::{DataSection, SharedSection, PrivateSection, ProgramMetadata};
use super::helpers::strip_comments;
use crate::error::CqamError;

/// Parse `.data` section lines into a [`DataSection`].
///
/// Supported directives:
///   - `label:` — names the next allocation (must precede a data directive)
///   - `.ascii "string"` — stores one ASCII byte per cell, NUL-terminated
///   - `.asciiz "string"` — alias for `.ascii`
///   - `.i64 val1, val2, ...` — stores literal i64 values
///   - `.f64 val1, val2, ...` — stores f64 values bit-cast to i64
///   - `.c64 z1, z2, ...` — stores complex values (aJb format), 2 cells each;
///     a trailing comma continues on the next line
pub(crate) fn parse_data_section(lines: &[(usize, &str)]) -> Result<DataSection, CqamError> {
    let mut ds = DataSection::default();
    let mut pending_label: Option<(String, usize)> = None; // (name, line_num)

    let mut i = 0;
    while i < lines.len() {
        let (line_num, raw_line) = lines[i];
        i += 1;
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        // Label definition: `name:`
        if !line.starts_with('.') && line.ends_with(':') {
            let name = line[..line.len() - 1].trim();
            if name.is_empty() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: ".data label requires a name".to_string(),
                });
            }
            if ds.labels.contains_key(name) {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("duplicate .data label '{}'", name),
                });
            }
            pending_label = Some((name.to_string(), line_num));
            continue;
        }

        // Data directives
        let base_addr = ds.cells.len() as u16;
        let cells_before = ds.cells.len();
        let mut logical_count: Option<u16> = None;

        if let Some(rest) = line.strip_prefix(".org") {
            // .org N — advance allocation pointer to address N
            let rest = rest.trim();
            let addr: usize = rest.parse().map_err(|_| CqamError::ParseError {
                line: line_num,
                message: format!(".org: invalid address '{}'", rest),
            })?;
            if addr < ds.cells.len() {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!(".org {} is below current position {}", addr, ds.cells.len()),
                });
            }
            ds.cells.resize(addr, 0);
            continue;
        } else if let Some(rest) = line.strip_prefix(".ascii") {
            parse_ascii_directive(rest.trim_start_matches('z'), line_num, &mut ds.cells)?;
        } else if let Some(rest) = line.strip_prefix(".i64") {
            parse_i64_directive(rest, line_num, &mut ds.cells)?;
        } else if let Some(rest) = line.strip_prefix(".f64") {
            parse_f64_directive(rest, line_num, &mut ds.cells)?;
        } else if let Some(rest) = line.strip_prefix(".c64") {
            // .c64 supports continuation: if a line ends with ',', the next
            // non-empty, non-comment line is a continuation of the same directive.
            let mut combined = rest.to_string();
            while combined.trim_end().ends_with(',') && i < lines.len() {
                let (_, next_raw) = lines[i];
                let next = strip_comments(next_raw).trim();
                if next.is_empty() {
                    i += 1;
                    continue;
                }
                // Stop if next line is a label or another directive
                if next.starts_with('.') || next.ends_with(':') {
                    break;
                }
                i += 1;
                combined.push_str(", ");
                combined.push_str(next);
            }
            let n = parse_c64_directive(&combined, line_num, &mut ds.cells)?;
            logical_count = Some(n as u16);
        } else if let Some(rest) = line.strip_prefix(".qstate") {
            // .qstate re_alpha, im_alpha, re_beta, im_beta
            // Produces exactly 4 CMEM cells. Validates normalization at assembly time.
            let tokens: Vec<&str> = rest.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if tokens.len() != 4 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!(".qstate requires exactly 4 values (re_a, im_a, re_b, im_b), got {}", tokens.len()),
                });
            }
            let vals: Vec<f64> = tokens.iter().enumerate().map(|(j, t)| {
                t.parse::<f64>().map_err(|_| CqamError::ParseError {
                    line: line_num,
                    message: format!(".qstate: invalid float '{}' at position {}", t, j),
                })
            }).collect::<Result<Vec<_>, _>>()?;
            let (re_a, im_a, re_b, im_b) = (vals[0], vals[1], vals[2], vals[3]);
            let norm_sq = re_a * re_a + im_a * im_a + re_b * re_b + im_b * im_b;
            if (norm_sq - 1.0).abs() > 1e-10 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!(
                        ".qstate: amplitudes not normalized (|alpha|^2 + |beta|^2 = {:.12}, expected 1.0)",
                        norm_sq
                    ),
                });
            }
            ds.cells.push(re_a.to_bits() as i64);
            ds.cells.push(im_a.to_bits() as i64);
            ds.cells.push(re_b.to_bits() as i64);
            ds.cells.push(im_b.to_bits() as i64);
        } else {
            return Err(CqamError::ParseError {
                line: line_num,
                message: format!("unknown .data directive: {}", line),
            });
        }

        let count = logical_count.unwrap_or((ds.cells.len() - cells_before) as u16);

        // Register the pending label at this base address
        if let Some((name, _)) = pending_label.take() {
            ds.labels.insert(name, (base_addr, count));
        }
    }

    if let Some((name, ln)) = pending_label {
        return Err(CqamError::ParseError {
            line: ln,
            message: format!(".data label '{}' has no data directive", name),
        });
    }

    Ok(ds)
}

/// Parse `.ascii "..."` — one ASCII byte per cell, auto NUL-terminated.
fn parse_ascii_directive(
    rest: &str,
    line_num: usize,
    cells: &mut Vec<i64>,
) -> Result<(), CqamError> {
    let rest = rest.trim();
    if !rest.starts_with('"') || !rest.ends_with('"') || rest.len() < 2 {
        return Err(CqamError::ParseError {
            line: line_num,
            message: ".ascii requires a quoted string".to_string(),
        });
    }
    let inner = &rest[1..rest.len() - 1];
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => cells.push(10),   // newline
                Some('t') => cells.push(9),    // tab
                Some('\\') => cells.push(92),  // backslash
                Some('"') => cells.push(34),   // quote
                Some('0') => cells.push(0),    // NUL
                Some(c) => {
                    return Err(CqamError::ParseError {
                        line: line_num,
                        message: format!("unknown escape '\\{}'", c),
                    });
                }
                None => {
                    return Err(CqamError::ParseError {
                        line: line_num,
                        message: "trailing backslash in string".to_string(),
                    });
                }
            }
        } else {
            cells.push(ch as i64);
        }
    }
    // NUL terminator (so PRINT_STR can also detect end by zero)
    cells.push(0);
    Ok(())
}

/// Parse `.i64 val1, val2, ...`
fn parse_i64_directive(
    rest: &str,
    line_num: usize,
    cells: &mut Vec<i64>,
) -> Result<(), CqamError> {
    for tok in rest.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let val: i64 = tok.parse().map_err(|_| CqamError::ParseError {
            line: line_num,
            message: format!(".i64: invalid integer '{}'", tok),
        })?;
        cells.push(val);
    }
    Ok(())
}

/// Parse `.f64 val1, val2, ...` — stores f64 bit-patterns as i64.
fn parse_f64_directive(
    rest: &str,
    line_num: usize,
    cells: &mut Vec<i64>,
) -> Result<(), CqamError> {
    for tok in rest.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let val: f64 = tok.parse().map_err(|_| CqamError::ParseError {
            line: line_num,
            message: format!(".f64: invalid float '{}'", tok),
        })?;
        cells.push(val.to_bits() as i64);
    }
    Ok(())
}

/// Parse a single complex literal in `aJb` format into (re, im).
fn parse_complex_literal(tok: &str, line_num: usize) -> Result<(f64, f64), CqamError> {
    let pos = tok.find(['j', 'J']).ok_or_else(|| CqamError::ParseError {
        line: line_num,
        message: format!(".c64: missing 'J' separator in complex literal '{}'", tok),
    })?;
    let re_str = &tok[..pos];
    let im_str = &tok[pos + 1..];
    if re_str.is_empty() {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!(".c64: missing real part in complex literal '{}'", tok),
        });
    }
    if im_str.is_empty() {
        return Err(CqamError::ParseError {
            line: line_num,
            message: format!(".c64: missing imaginary part in complex literal '{}'", tok),
        });
    }
    let re: f64 = re_str.parse().map_err(|_| CqamError::ParseError {
        line: line_num,
        message: format!(".c64: invalid real part '{}' in '{}'", re_str, tok),
    })?;
    let im: f64 = im_str.parse().map_err(|_| CqamError::ParseError {
        line: line_num,
        message: format!(".c64: invalid imaginary part '{}' in '{}'", im_str, tok),
    })?;
    Ok((re, im))
}

/// Parse `.c64 z1, z2, ...` — stores complex values (aJb format) as pairs of
/// f64 bit-patterns. Returns the number of complex values parsed (NOT the
/// number of cells, which is 2x that).
fn parse_c64_directive(
    rest: &str,
    line_num: usize,
    cells: &mut Vec<i64>,
) -> Result<usize, CqamError> {
    let mut value_count = 0;
    for tok in rest.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let (re, im) = parse_complex_literal(tok, line_num)?;
        cells.push(re.to_bits() as i64);
        cells.push(im.to_bits() as i64);
        value_count += 1;
    }
    Ok(value_count)
}

/// Replace `@label` with the CMEM base address and `@label.len` with the
/// cell count for all data labels found in the line.
pub(crate) fn substitute_data_refs(line: &str, ds: &DataSection) -> String {
    if ds.labels.is_empty() || !line.contains('@') {
        return line.to_string();
    }

    let mut result = line.to_string();

    // Process longest labels first to avoid partial substitution
    let mut labels: Vec<_> = ds.labels.iter().collect();
    labels.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (name, (base, len)) in &labels {
        // @label.len must be replaced before @label to avoid partial match
        let len_token = format!("@{}.len", name);
        result = result.replace(&len_token, &len.to_string());
        let addr_token = format!("@{}", name);
        result = result.replace(&addr_token, &base.to_string());
    }

    result
}

/// Replace `@label` with the CMEM base address and `@label.len` with the
/// cell count for all shared labels found in the line.
pub(crate) fn substitute_shared_refs(line: &str, ss: &SharedSection) -> String {
    if ss.labels.is_empty() || !line.contains('@') {
        return line.to_string();
    }

    let mut result = line.to_string();

    // Process longest labels first to avoid partial substitution
    let mut labels: Vec<_> = ss.labels.iter().collect();
    labels.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (name, (base, len)) in &labels {
        // @label.len must be replaced before @label to avoid partial match
        let len_token = format!("@{}.len", name);
        result = result.replace(&len_token, &len.to_string());
        let addr_token = format!("@{}", name);
        result = result.replace(&addr_token, &base.to_string());
    }

    result
}

/// Parse `.shared` section lines into a [`SharedSection`].
///
/// Reuses the same directive syntax as `.data` (labels, `.org`, `.i64`, etc.).
pub(crate) fn parse_shared_section(lines: &[(usize, &str)]) -> Result<SharedSection, CqamError> {
    if lines.is_empty() {
        return Ok(SharedSection::default());
    }
    let ds = parse_data_section(lines)?;
    Ok(SharedSection {
        base: 0,
        cells: ds.cells,
        labels: ds.labels,
    })
}

/// Parse `.private` section lines into a [`PrivateSection`].
///
/// Supported directives:
///   - `.size N` — per-thread private memory size in cells
pub(crate) fn parse_private_section(lines: &[(usize, &str)]) -> Result<PrivateSection, CqamError> {
    let mut ps = PrivateSection::default();

    for &(line_num, raw_line) in lines {
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix(".size") {
            let rest = rest.trim();
            let n: u16 = rest.parse().map_err(|_| CqamError::ParseError {
                line: line_num,
                message: format!(".size: invalid value '{}'", rest),
            })?;
            ps.size = n;
        } else {
            return Err(CqamError::ParseError {
                line: line_num,
                message: format!("unknown .private directive: {}", line),
            });
        }
    }

    Ok(ps)
}

/// Parse a single pragma line (without the `#!` prefix).
///
/// Returns Ok(()) and updates metadata, or Err on malformed pragma.
pub(crate) fn parse_pragma(
    line: &str,
    line_num: usize,
    metadata: &mut ProgramMetadata,
) -> Result<(), CqamError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        // Empty pragma -- ignore
        return Ok(());
    }

    match tokens[0] {
        "qubits" => {
            if tokens.len() < 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "#! qubits requires a number".to_string(),
                });
            }
            let n: u8 = tokens[1].parse().map_err(|_| CqamError::ParseError {
                line: line_num,
                message: format!("#! qubits value must be a number, got '{}'", tokens[1]),
            })?;
            if n == 0 || n > 16 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("#! qubits must be 1..16, got {}", n),
                });
            }
            metadata.qubits = Some(n);
        }
        "threads" => {
            if tokens.len() < 2 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: "#! threads requires a number".to_string(),
                });
            }
            let n: u16 = tokens[1].parse().map_err(|_| CqamError::ParseError {
                line: line_num,
                message: format!("#! threads value must be a number, got '{}'", tokens[1]),
            })?;
            if n == 0 || n > 256 {
                return Err(CqamError::ParseError {
                    line: line_num,
                    message: format!("#! threads must be 1..256, got {}", n),
                });
            }
            metadata.threads = Some(n);
        }
        _ => {
            // Unknown pragma -- ignore for forward compatibility
        }
    }
    Ok(())
}
