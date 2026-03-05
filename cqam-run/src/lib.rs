//! CQAM program runner: loading, execution, reporting, and simulation configuration.
//!
//! `cqam-run` exposes a library API used by the `cqam-run` binary and by
//! integration tests. It layers on top of `cqam-core` (parsing) and
//! `cqam-vm` (execution) and adds file I/O, configuration loading, and
//! human-readable report printing.
//!
//! # Key functions
//!
//! | Module | Function | Purpose |
//! |--------|----------|---------|
//! | [`loader`] | [`load_program`](loader::load_program) | Read a `.cqam` file from disk |
//! | [`runner`] | [`run_program`](runner::run_program) | Execute with default config |
//! | [`runner`] | [`run_program_with_config`](runner::run_program_with_config) | Execute with custom [`SimConfig`](simconfig::SimConfig) |
//! | [`report`] | [`print_report`](report::print_report) | Print final state, PSW, resources |
//! | [`simconfig`] | [`SimConfig`](simconfig::SimConfig) | TOML-based simulator config |
//!
//! # Typical workflow
//!
//! ```ignore
//! use cqam_run::loader::load_program;
//! use cqam_run::runner::run_program;
//! use cqam_run::report::print_report;
//!
//! let program = load_program("examples/arithmetic.cqam").unwrap();
//! let ctx = run_program(program).unwrap();
//! print_report(&ctx, true, false, false);
//! ```

pub mod loader;
pub mod runner;
pub mod report;
pub mod simconfig;
