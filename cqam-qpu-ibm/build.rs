//! Build script for `cqam-qpu-ibm`.
//!
//! Resolves the location of the Qiskit C library (`libqiskit`) and links
//! against it. Resolution order:
//!
//!   1. `QISKIT_C_DIR` — explicit override pointing at a prebuilt
//!      `dist/c` directory (must contain `lib/libqiskit.{dylib,so}` and
//!      `include/qiskit/*.h`).
//!   2. `$HOME/.local/qiskit/dist/c` — user-local canonical location.
//!      If absent, the build script clones upstream Qiskit into
//!      `$HOME/.local/qiskit` and runs `make c` there as part of the
//!      cargo build. Subsequent builds reuse the existing tree.
//!
//! No sudo is required: `~/.local` is user-owned on macOS and Linux.
//!
//! Requirements for the automatic clone+build path: `git`, `make`,
//! `python3`, and a working Rust toolchain on `PATH`.
//!
//! Knobs:
//!   - `QISKIT_GIT_REV`        — upstream revision (default `main`)
//!   - `CQAM_NO_QISKIT_BUILD`  — set to disable the auto-build and fail fast

use std::path::{Path, PathBuf};
use std::process::Command;

const QISKIT_SUBDIR: &str = ".local/qiskit";
const QISKIT_GIT_URL: &str = "https://github.com/Qiskit/qiskit.git";
const DEFAULT_GIT_REV: &str = "main";

fn main() {
    println!("cargo:rerun-if-env-changed=QISKIT_C_DIR");
    println!("cargo:rerun-if-env-changed=QISKIT_GIT_REV");
    println!("cargo:rerun-if-env-changed=CQAM_NO_QISKIT_BUILD");
    println!("cargo:rerun-if-env-changed=HOME");

    let qiskit_c_dir = resolve_or_build();
    let lib_dir = qiskit_c_dir.join("lib");

    if !lib_dir.exists() {
        panic!(
            "Resolved qiskit C dir {} but {} does not exist",
            qiskit_c_dir.display(),
            lib_dir.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=qiskit");
    println!("cargo:qiskit_c_dir={}", qiskit_c_dir.display());
}

fn resolve_or_build() -> PathBuf {
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

    let prefix = default_prefix();
    let dist_c = prefix.join("dist").join("c");
    if dist_c.join("lib").exists() {
        return dist_c;
    }

    if std::env::var("CQAM_NO_QISKIT_BUILD").is_ok() {
        panic!(
            "libqiskit not found at {} and CQAM_NO_QISKIT_BUILD is set. \
             Install libqiskit to {} or unset the flag.",
            dist_c.display(),
            dist_c.display()
        );
    }

    clone_and_build(&prefix);

    if !dist_c.join("lib").exists() {
        panic!(
            "`make c` completed but {} is missing",
            dist_c.join("lib").display()
        );
    }
    dist_c
}

fn default_prefix() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| {
        panic!(
            "$HOME is not set; cannot determine default qiskit prefix \
             (~/{}). Set QISKIT_C_DIR explicitly or export HOME.",
            QISKIT_SUBDIR
        )
    });
    PathBuf::from(home).join(QISKIT_SUBDIR)
}

fn clone_and_build(prefix: &Path) {
    ensure_writable(prefix);

    if !prefix.join(".git").exists() {
        let rev = std::env::var("QISKIT_GIT_REV").unwrap_or_else(|_| DEFAULT_GIT_REV.to_string());
        eprintln!(
            "cqam-qpu-ibm: cloning {} (rev {}) into {}",
            QISKIT_GIT_URL,
            rev,
            prefix.display()
        );

        let non_empty = std::fs::read_dir(prefix)
            .map(|mut it| it.next().is_some())
            .unwrap_or(false);
        if non_empty {
            panic!(
                "{} exists and is non-empty but has no .git directory. \
                 Remove its contents and rebuild, or point QISKIT_C_DIR at \
                 an existing prebuilt tree.",
                prefix.display()
            );
        }

        run(
            Command::new("git")
                .arg("clone")
                .arg("--depth=1")
                .arg("--branch")
                .arg(&rev)
                .arg(QISKIT_GIT_URL)
                .arg(prefix),
            "git clone qiskit",
        );
    }

    eprintln!(
        "cqam-qpu-ibm: building libqiskit via `make c` in {}",
        prefix.display()
    );
    run(
        Command::new("make").arg("c").current_dir(prefix),
        "make c (qiskit)",
    );
}

fn ensure_writable(prefix: &Path) {
    if prefix.exists() {
        let probe = prefix.join(".cqam-write-probe");
        match std::fs::write(&probe, b"") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
            }
            Err(e) => panic!(
                "{} exists but is not writable ({}). Check ownership/permissions.",
                prefix.display(),
                e
            ),
        }
    } else if let Err(e) = std::fs::create_dir_all(prefix) {
        panic!("failed to create {}: {}", prefix.display(), e);
    }
}

fn run(cmd: &mut Command, label: &str) {
    let status = cmd.status().unwrap_or_else(|e| {
        panic!(
            "failed to spawn `{}` for {}: {}. Ensure git, make, and python3 are on PATH.",
            format_cmd(cmd),
            label,
            e
        )
    });
    if !status.success() {
        panic!(
            "{} failed: {} (exit {:?})",
            label,
            format_cmd(cmd),
            status.code()
        );
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
