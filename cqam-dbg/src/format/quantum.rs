//! Quantum state extraction, top-K filtering, bar chart rendering, and
//! coherence summary formatting for the QUANTUM pane.
#![allow(dead_code)]

use cqam_core::quantum_backend::{QRegHandle, QuantumBackend};

// =============================================================================
// Unicode block characters for bar chart rendering
// =============================================================================

/// Block characters from empty (0/8) to full (8/8).
const BLOCK_CHARS: [char; 9] = [
    ' ',        // 0/8
    '\u{258F}', // 1/8  LEFT 1/8 BLOCK
    '\u{258E}', // 2/8  LEFT 1/4 BLOCK
    '\u{258D}', // 3/8  LEFT 3/8 BLOCK
    '\u{258C}', // 4/8  LEFT 1/2 BLOCK
    '\u{258B}', // 5/8  LEFT 5/8 BLOCK
    '\u{258A}', // 6/8  LEFT 3/4 BLOCK
    '\u{2589}', // 7/8  LEFT 7/8 BLOCK
    '\u{2588}', // 8/8  FULL BLOCK
];

// =============================================================================
// Top-K entry
// =============================================================================

/// A single entry in the top-K probability display.
#[derive(Debug, Clone)]
pub struct TopKEntry {
    /// Basis state index (0..dim).
    pub basis_index: usize,
    /// Probability (diagonal element rho[k][k]).
    pub probability: f64,
    /// Complex amplitude. For Pure states, the actual amplitude from the
    /// statevector. For Mixed states, (sqrt(p), 0.0) as a placeholder.
    pub amplitude: (f64, f64),
    /// Phase in radians. For Mixed states, `None` (not meaningful).
    pub phase: Option<f64>,
}

/// Result of top-K extraction from a quantum register.
#[derive(Debug, Clone)]
pub struct TopKResult {
    /// Whether the register is a Pure state.
    pub is_pure: bool,
    /// Number of qubits in the register.
    pub num_qubits: u8,
    /// Hilbert space dimension (2^num_qubits).
    pub dimension: usize,
    /// Purity of the state (1.0 for pure, <1.0 for mixed).
    pub purity: f64,
    /// The filtered, sorted entries.
    pub entries: Vec<TopKEntry>,
    /// Number of basis states below the threshold (not shown).
    pub suppressed_count: usize,
}

// =============================================================================
// Top-K extraction
// =============================================================================

/// Extract the top-K basis states by probability from a quantum register handle.
///
/// # Arguments
///
/// * `backend` - The quantum backend to query state from.
/// * `handle` - The quantum register handle to inspect.
/// * `topk` - Maximum number of entries to return.
/// * `threshold` - Minimum probability to include (entries below are suppressed).
pub fn extract_top_k(backend: &dyn QuantumBackend, handle: QRegHandle, topk: usize, threshold: f64) -> TopKResult {
    let is_pure = backend.is_pure(handle).unwrap_or(false);
    let num_qubits = backend.num_qubits(handle).unwrap_or(0);
    let dimension = backend.dimension(handle).unwrap_or(0);
    let purity = backend.purity(handle).unwrap_or(0.0);

    let probs = backend.diagonal_probabilities(handle).unwrap_or_default();

    // Collect entries above threshold.
    let mut entries: Vec<(usize, f64)> = probs
        .iter()
        .enumerate()
        .filter(|entry| *entry.1 >= threshold)
        .map(|entry| (entry.0, *entry.1))
        .collect();

    // Sort descending by probability.
    entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let total_above = entries.len();
    entries.truncate(topk);

    let suppressed_count = dimension - total_above;

    let entries: Vec<TopKEntry> = entries
        .into_iter()
        .map(|(k, p)| {
            let (amplitude, phase) = if is_pure {
                // For Pure states, extract the actual amplitude from the backend.
                let amp = backend.amplitude(handle, k).unwrap_or((p.sqrt(), 0.0));
                let ph = amp.1.atan2(amp.0);
                (amp, Some(ph))
            } else {
                // For Mixed states, amplitude is not directly available.
                // Show sqrt(p) + 0i as a placeholder; phase is not meaningful.
                ((p.sqrt(), 0.0), None)
            };

            TopKEntry {
                basis_index: k,
                probability: p,
                amplitude,
                phase,
            }
        })
        .collect();

    TopKResult {
        is_pure,
        num_qubits,
        dimension,
        purity,
        entries,
        suppressed_count,
    }
}

// =============================================================================
// Bar chart rendering
// =============================================================================

/// Render a horizontal bar for a probability value using Unicode block characters.
///
/// The bar width scales to fill `max_width` terminal cells. A probability of 1.0
/// fills the entire width.
pub fn render_bar(probability: f64, max_width: usize) -> String {
    let full_eighths = (probability * max_width as f64 * 8.0).round() as usize;
    let full_cells = full_eighths / 8;
    let remainder = full_eighths % 8;

    let mut bar = String::with_capacity(full_cells + 1);
    for _ in 0..full_cells {
        bar.push(BLOCK_CHARS[8]); // full block
    }
    if remainder > 0 {
        bar.push(BLOCK_CHARS[remainder]);
    }
    bar
}

// =============================================================================
// Basis ket formatting
// =============================================================================

/// Format a basis state index as a ket label, e.g. "|011>".
pub fn format_basis_ket(index: usize, num_qubits: u8) -> String {
    let mut s = String::with_capacity(num_qubits as usize + 2);
    s.push('|');
    for bit in (0..num_qubits).rev() {
        if (index >> bit) & 1 == 1 {
            s.push('1');
        } else {
            s.push('0');
        }
    }
    s.push('>');
    s
}

// =============================================================================
// Coherence summary for mixed states
// =============================================================================

/// Coherence summary for mixed-state quantum registers.
#[derive(Debug, Clone)]
pub struct CoherenceSummary {
    /// Maximum off-diagonal magnitude: max |rho[i][j]| for i != j.
    pub max_off_diagonal: f64,
    /// Purity: Tr(rho^2).
    pub purity: f64,
}

/// Compute the coherence summary for a quantum register handle.
///
/// For pure states, returns `None` (coherence summary is only meaningful for
/// mixed states). For mixed states, computes the max off-diagonal magnitude
/// using exact computation for small matrices (<= 8 qubits) and sampling for
/// larger ones.
pub fn coherence_summary(backend: &dyn QuantumBackend, handle: QRegHandle) -> Option<CoherenceSummary> {
    let is_pure = backend.is_pure(handle).unwrap_or(true);
    if is_pure {
        return None;
    }
    let dim = backend.dimension(handle).unwrap_or(0);
    let max_off_diag = max_off_diagonal(backend, handle, dim);
    Some(CoherenceSummary {
        max_off_diagonal: max_off_diag,
        purity: backend.purity(handle).unwrap_or(0.0),
    })
}

/// Compute (or sample) the maximum off-diagonal magnitude.
///
/// For small matrices (dim^2 <= MAX_EXACT_ELEMENTS), computes exactly.
/// For larger matrices, uses random sampling.
fn max_off_diagonal(backend: &dyn QuantumBackend, handle: QRegHandle, dim: usize) -> f64 {
    const MAX_EXACT_ELEMENTS: usize = 65_536; // up to 8 qubits (256x256)

    if dim * dim <= MAX_EXACT_ELEMENTS {
        max_off_diagonal_exact(backend, handle, dim)
    } else {
        max_off_diagonal_sampled(backend, handle, dim, 100_000)
    }
}

/// Exact max off-diagonal computation for small matrices.
fn max_off_diagonal_exact(backend: &dyn QuantumBackend, handle: QRegHandle, dim: usize) -> f64 {
    let mut max_val: f64 = 0.0;
    for i in 0..dim {
        for j in 0..dim {
            if i != j {
                let (re, im) = backend.get_element(handle, i, j).unwrap_or((0.0, 0.0));
                let mag = (re * re + im * im).sqrt();
                if mag > max_val {
                    max_val = mag;
                }
            }
        }
    }
    max_val
}

/// Sampled max off-diagonal for large matrices.
///
/// Uses a simple deterministic sampling pattern rather than random sampling
/// to avoid adding a rand dependency. Samples stride-based off-diagonal pairs.
fn max_off_diagonal_sampled(backend: &dyn QuantumBackend, handle: QRegHandle, dim: usize, max_samples: usize) -> f64 {
    let mut max_val: f64 = 0.0;
    let mut count = 0usize;

    // Use a stride-based pattern to sample off-diagonal elements.
    // This covers a spread of the matrix without requiring randomness.
    let stride = ((dim * dim) as f64 / max_samples as f64).ceil() as usize;
    let stride = stride.max(1);

    'outer: for i in 0..dim {
        for j in (0..dim).step_by(stride) {
            if i == j {
                continue;
            }
            let (re, im) = backend.get_element(handle, i, j).unwrap_or((0.0, 0.0));
            let mag = (re * re + im * im).sqrt();
            if mag > max_val {
                max_val = mag;
            }
            count += 1;
            if count >= max_samples {
                break 'outer;
            }
        }
    }
    max_val
}

// =============================================================================
// Formatting helpers
// =============================================================================

/// Format an amplitude as a string: "re+imi" or "re-imi".
pub fn format_amplitude(amp: (f64, f64)) -> String {
    let (re, im) = amp;
    if im >= 0.0 {
        format!("{:.3}+{:.3}i", re, im)
    } else {
        format!("{:.3}{:.3}i", re, im)
    }
}

/// Format a phase value in radians.
pub fn format_phase(phase: Option<f64>) -> String {
    match phase {
        Some(p) => format!("{:.3} rad", p),
        None => "--".to_string(),
    }
}

/// Format the header line for the QUANTUM pane.
///
/// Example: "Q0: Pure, 3 qubits, dim=8, purity=1.000"
pub fn format_quantum_header(result: &TopKResult, qreg_index: u8) -> String {
    let type_label = if result.is_pure { "Pure" } else { "Mixed" };
    format!(
        "Q{}: {}, {} qubits, dim={}, purity={:.3}",
        qreg_index, type_label, result.num_qubits, result.dimension, result.purity
    )
}

/// Format the filter summary line.
///
/// Example: "Showing top 4 of 8 entries (threshold >= 0.01)"
pub fn format_filter_summary(result: &TopKResult, topk: usize, threshold: f64) -> String {
    format!(
        "Showing top {} of {} entries (threshold >= {:.2})",
        result.entries.len().min(topk),
        result.dimension,
        threshold
    )
}

/// Format the coherence summary line for mixed states.
///
/// Example: "Coherence: max|rho_ij|=0.354  purity=0.750"
pub fn format_coherence(summary: &CoherenceSummary) -> String {
    format!(
        "Coherence: max|rho_ij|={:.3}  purity={:.3}",
        summary.max_off_diagonal, summary.purity
    )
}

/// Format the suppressed count line.
///
/// Example: "(4 entries below threshold)"
pub fn format_suppressed(count: usize) -> String {
    if count == 0 {
        String::new()
    } else {
        format!("({} entries below threshold)", count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_bar_full() {
        let bar = render_bar(1.0, 10);
        // Should be 10 full blocks.
        assert_eq!(bar.chars().count(), 10);
        assert!(bar.chars().all(|c| c == '\u{2588}'));
    }

    #[test]
    fn test_render_bar_empty() {
        let bar = render_bar(0.0, 20);
        assert!(bar.is_empty());
    }

    #[test]
    fn test_render_bar_half() {
        let bar = render_bar(0.5, 10);
        // Should be approximately 5 full blocks.
        assert!(bar.chars().count() >= 4 && bar.chars().count() <= 6);
    }

    #[test]
    fn test_format_basis_ket() {
        assert_eq!(format_basis_ket(0, 3), "|000>");
        assert_eq!(format_basis_ket(7, 3), "|111>");
        assert_eq!(format_basis_ket(5, 3), "|101>");
        assert_eq!(format_basis_ket(0, 1), "|0>");
        assert_eq!(format_basis_ket(1, 1), "|1>");
    }

    #[test]
    fn test_format_amplitude_positive() {
        let s = format_amplitude((0.707, 0.707));
        assert!(s.contains("+"));
    }

    #[test]
    fn test_format_amplitude_negative() {
        let s = format_amplitude((0.707, -0.707));
        assert!(s.contains("-"));
    }

    #[test]
    fn test_format_phase_some() {
        let s = format_phase(Some(1.571));
        assert!(s.contains("rad"));
    }

    #[test]
    fn test_format_phase_none() {
        assert_eq!(format_phase(None), "--");
    }
}
