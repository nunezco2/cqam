//! Histogram formatting for ECALL PRINT_HIST.

use cqam_core::register::HybridValue;
use cqam_core::shot::ShotHistogram;

/// Format an H register value as a human-readable histogram.
///
/// # Arguments
/// * `h_index` - H register index (0-7)
/// * `value` - The HybridValue to format
/// * `mode` - Visualization mode (0=table, 1=bar, 2=sorted-by-state, 3=top-k)
/// * `top_k` - Number of top outcomes to show (mode 3 only)
/// * `num_qubits` - Qubit count for binary state labels
pub fn format_histogram(
    h_index: u8,
    value: &HybridValue,
    mode: u32,
    top_k: u32,
    num_qubits: u8,
) -> String {
    match value {
        HybridValue::Empty => format!("H{}: (empty)", h_index),
        HybridValue::Int(k) => {
            let width = num_qubits as usize;
            format!(
                "H{}: |{:0width$b}> (single sample)",
                h_index, *k as u32,
                width = width,
            )
        }
        HybridValue::Float(f) => format!("H{}: {:.6}", h_index, f),
        HybridValue::Complex(re, im) => {
            if *im >= 0.0 {
                format!("H{}: ({:.6} + {:.6}i)", h_index, re, im)
            } else {
                format!("H{}: ({:.6} - {:.6}i)", h_index, re, im.abs())
            }
        }
        HybridValue::Dist(entries) => format_dist(h_index, entries, mode, top_k, num_qubits),
        HybridValue::Hist(hist) => format_hist(h_index, hist, mode, top_k, num_qubits),
    }
}

fn format_dist(
    h_index: u8,
    entries: &[(u32, f64)],
    mode: u32,
    top_k: u32,
    num_qubits: u8,
) -> String {
    let width = num_qubits as usize;
    let mut lines = Vec::new();

    // Sort entries
    let mut sorted: Vec<(u32, f64)> = entries.to_vec();
    match mode {
        2 => sorted.sort_by_key(|(state, _)| *state),
        _ => sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)),
    }

    let total_outcomes = sorted.len();

    // Header
    lines.push(format!("H{} (exact, {} outcomes):", h_index, total_outcomes));

    match mode {
        0 | 2 => {
            for &(state, prob) in &sorted {
                lines.push(format!(
                    "  |{:0width$b}> : {:.6}",
                    state, prob,
                    width = width,
                ));
            }
        }
        1 => {
            let max_prob = sorted.iter().map(|(_, p)| *p).fold(0.0_f64, f64::max);
            let bar_max: usize = 50;
            for &(state, prob) in &sorted {
                let bar_len = if max_prob > 0.0 {
                    ((prob / max_prob) * bar_max as f64).round() as usize
                } else {
                    0
                };
                let bar: String = "\u{2588}".repeat(bar_len);
                let padding = " ".repeat(bar_max.saturating_sub(bar_len));
                lines.push(format!(
                    "  |{:0width$b}> {}{} {:.2}%",
                    state, bar, padding,
                    prob * 100.0,
                    width = width,
                ));
            }
        }
        3 => {
            let k = (top_k as usize).min(sorted.len());
            lines[0] = format!("H{} (exact, top {} of {}):", h_index, k, total_outcomes);
            for &(state, prob) in sorted.iter().take(k) {
                lines.push(format!(
                    "  |{:0width$b}> : {:.6}",
                    state, prob,
                    width = width,
                ));
            }
            if k < total_outcomes {
                let rest_prob: f64 = sorted.iter().skip(k).map(|(_, p)| p).sum();
                lines.push(format!(
                    "  ... {} more outcomes ({:.2}% total)",
                    total_outcomes - k,
                    rest_prob * 100.0,
                ));
            }
        }
        _ => {
            for &(state, prob) in &sorted {
                lines.push(format!(
                    "  |{:0width$b}> : {:.6}",
                    state, prob,
                    width = width,
                ));
            }
        }
    }

    lines.join("\n")
}

fn format_hist(
    h_index: u8,
    hist: &ShotHistogram,
    mode: u32,
    top_k: u32,
    num_qubits: u8,
) -> String {
    let width = num_qubits as usize;
    let total = hist.total_shots;
    let mut lines = Vec::new();

    // Sort entries
    let mut sorted: Vec<(u32, u32)> = hist.counts.iter().map(|(&s, &c)| (s, c)).collect();
    match mode {
        2 => sorted.sort_by_key(|(state, _)| *state),
        _ => sorted.sort_by(|a, b| b.1.cmp(&a.1)),
    }

    let total_outcomes = sorted.len();

    // Header
    lines.push(format!(
        "H{} ({} shots, {} outcomes):",
        h_index, total, total_outcomes,
    ));

    match mode {
        0 | 2 => {
            for &(state, count) in &sorted {
                let pct = 100.0 * count as f64 / total as f64;
                lines.push(format!(
                    "  |{:0width$b}> : {:6} ({:6.2}%)",
                    state, count, pct,
                    width = width,
                ));
            }
        }
        1 => {
            let max_count = sorted.iter().map(|(_, c)| *c).max().unwrap_or(1);
            let bar_max: usize = 50;
            for &(state, count) in &sorted {
                let bar_len = if max_count > 0 {
                    ((count as f64 / max_count as f64) * bar_max as f64).round() as usize
                } else {
                    0
                };
                let bar: String = "\u{2588}".repeat(bar_len);
                let padding = " ".repeat(bar_max.saturating_sub(bar_len));
                let pct = 100.0 * count as f64 / total as f64;
                lines.push(format!(
                    "  |{:0width$b}> {}{} {:6.2}%",
                    state, bar, padding, pct,
                    width = width,
                ));
            }
        }
        3 => {
            let k = (top_k as usize).min(sorted.len());
            lines[0] = format!(
                "H{} ({} shots, top {} of {}):",
                h_index, total, k, total_outcomes,
            );
            for &(state, count) in sorted.iter().take(k) {
                let pct = 100.0 * count as f64 / total as f64;
                lines.push(format!(
                    "  |{:0width$b}> : {:6} ({:6.2}%)",
                    state, count, pct,
                    width = width,
                ));
            }
            if k < total_outcomes {
                let rest_count: u32 = sorted.iter().skip(k).map(|(_, c)| c).sum();
                let rest_pct = 100.0 * rest_count as f64 / total as f64;
                lines.push(format!(
                    "  ... {} more outcomes ({:.2}% total)",
                    total_outcomes - k, rest_pct,
                ));
            }
        }
        _ => {
            for &(state, count) in &sorted {
                let pct = 100.0 * count as f64 / total as f64;
                lines.push(format!(
                    "  |{:0width$b}> : {:6} ({:6.2}%)",
                    state, count, pct,
                    width = width,
                ));
            }
        }
    }

    lines.join("\n")
}
