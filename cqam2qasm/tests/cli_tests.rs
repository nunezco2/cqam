//! CLI integration tests for `cqam2qasm`: verifies end-to-end QASM output.

use std::process::Command;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_cqam2qasm_cli_output() {
    let test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let input_path = test_dir.join("sample.cqam");
    let output_path = test_dir.join("output.qasm");

    // Ensure input exists
    assert!(input_path.exists(), "sample.cqam not found at {:?}", input_path);

    // Run CLI
    let status = Command::new("cargo")
        .args([
            "run", "-p", "cqam2qasm", "--",
            input_path.to_str().unwrap(),
            "--out", output_path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to execute cqam2qasm CLI");

    assert!(status.success(), "CLI execution failed");

    let output = fs::read_to_string(&output_path).expect("Failed to read output.qasm");
    assert!(output.contains("OPENQASM 3.0;"));
    // Body lines use bare assignments without type prefixes; check for bare assignment
    assert!(
        output.contains("R0 = 5;") || output.contains("reset q0;"),
        "Output missing expected content"
    );
}
