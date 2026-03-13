//! Re-export `C64` from `cqam-core` for backward compatibility.
//!
//! The canonical definition lives in `cqam_core::complex::C64`.
//! This module re-exports it so existing `cqam_sim::complex::C64` imports
//! continue to work without changes.

pub use cqam_core::complex::C64;
