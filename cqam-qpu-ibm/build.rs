fn main() {
    let qiskit_c_dir = std::env::var("QISKIT_C_DIR")
        .unwrap_or_else(|_| "/tmp/qiskit/dist/c".to_string());

    let lib_dir = format!("{}/lib", qiskit_c_dir);

    println!("cargo:rustc-link-search=native={}", lib_dir);
    println!("cargo:rustc-link-lib=dylib=qiskit");
    println!("cargo:rerun-if-env-changed=QISKIT_C_DIR");
}
