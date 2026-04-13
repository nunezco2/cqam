//! Build script for `cqam-qpu-ibm`.
//!
//! Resolves the location of the Qiskit C library (`libqiskit`) using the
//! following precedence:
//!
//!   1. `QISKIT_C_DIR` environment variable (explicit override).
//!   2. The system default `/opt/qiskit/dist/c` when present.
//!   3. A cargo-managed clone+build into `OUT_DIR/qiskit-src/dist/c`.
//!
//! The cargo-managed path clones the upstream Qiskit repository (pinned via
//! `QISKIT_GIT_REV`, default `main`) and invokes `make c`. It requires `git`,
//! `make`, and a working Python 3 toolchain on `PATH`. Set `CQAM_NO_QISKIT_BUILD=1`
//! to disable the automatic build and fail fast instead.

use std::path::PathBuf;
use std::process::Command;

const DEFAULT_SYSTEM_PREFIX: &str = "/opt/qiskit/dist/c";
const QISKIT_GIT_URL: &str = "https://github.com/Qiskit/qiskit.git";
const DEFAULT_GIT_REV: &str = "main";

fn main() {
    println!("cargo:rerun-if-env-changed=QISKIT_C_DIR");
    println!("cargo:rerun-if-env-changed=QISKIT_GIT_REV");
    println!("cargo:rerun-if-env-changed=CQAM_NO_QISKIT_BUILD");

    let qiskit_c_dir = resolve_qiskit_c_dir();
    let lib_dir = qiskit_c_dir.join("lib");

    if !lib_dir.exists() {
        panic!(
            "Resolved QISKIT_C_DIR={} but {} does not exist",
            qiskit_c_dir.display(),
            lib_dir.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=qiskit");
    println!("cargo:qiskit_c_dir={}", qiskit_c_dir.display());
}

fn resolve_qiskit_c_dir() -> PathBuf {
    if let Ok(explicit) = std::env::var("QISKIT_C_DIR") {
        let p = PathBuf::from(&explicit);
        if !p.join("lib").exists() {
            panic!(
                "QISKIT_C_DIR={} is set but {}/lib does not exist",
                explicit, explicit
            );
        }
        return p;
    }

    let system = PathBuf::from(DEFAULT_SYSTEM_PREFIX);
    if system.join("lib").exists() {
        return system;
    }

    if std::env::var("CQAM_NO_QISKIT_BUILD").is_ok() {
        panic!(
            "libqiskit not found at {} and CQAM_NO_QISKIT_BUILD is set. \
             Install libqiskit to {} or unset CQAM_NO_QISKIT_BUILD to \
             let cargo build it.",
            DEFAULT_SYSTEM_PREFIX, DEFAULT_SYSTEM_PREFIX
        );
    }

    build_vendored_qiskit()
}

fn build_vendored_qiskit() -> PathBuf {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let src_dir = out_dir.join("qiskit-src");
    let dist_c = src_dir.join("dist").join("c");
    let rev = std::env::var("QISKIT_GIT_REV").unwrap_or_else(|_| DEFAULT_GIT_REV.to_string());

    if dist_c.join("lib").exists() {
        return dist_c;
    }

    if !src_dir.join(".git").exists() {
        eprintln!(
            "cqam-qpu-ibm: cloning {} (rev {}) into {}",
            QISKIT_GIT_URL,
            rev,
            src_dir.display()
        );
        run(
            Command::new("git")
                .arg("clone")
                .arg("--depth=1")
                .arg("--branch")
                .arg(&rev)
                .arg(QISKIT_GIT_URL)
                .arg(&src_dir),
            "git clone qiskit",
        );
    }

    eprintln!(
        "cqam-qpu-ibm: building libqiskit via `make c` in {}",
        src_dir.display()
    );
    run(
        Command::new("make").arg("c").current_dir(&src_dir),
        "make c (qiskit)",
    );

    if !dist_c.join("lib").exists() {
        panic!(
            "`make c` completed but {}/lib is missing",
            dist_c.display()
        );
    }
    dist_c
}

fn run(cmd: &mut Command, label: &str) {
    let status = cmd.status().unwrap_or_else(|e| {
        panic!(
            "failed to spawn `{}` for {}: {}. Ensure the required toolchain \
             (git, make, python3) is installed.",
            format_cmd(cmd),
            label,
            e
        )
    });
    if !status.success() {
        panic!("{} failed: {} (exit {:?})", label, format_cmd(cmd), status.code());
    }
}

fn format_cmd(cmd: &Command) -> String {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    format!("{} {}", program, args.join(" "))
}
