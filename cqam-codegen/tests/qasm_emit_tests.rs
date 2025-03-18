use cqam_codegen::qasm::OpenQASMEmitter;
use cqam_codegen::emitter::QASMEmitter;
use cqam_core::instruction::Instruction;

#[test]
fn test_basic_qasm_emit() {
    let program = vec![
        Instruction::QPrep { dst: "0".into(), dist_src: "".into() },
        Instruction::QKernel { dst: "1".into(), src: "0".into(), kernel: "entangle".into(), ctx: None },
        Instruction::QMeas { dst: "0".into(), src: "0".into() },
    ];
    let emitter = OpenQASMEmitter;
    let output = emitter.emit_program(&program);
    println!("{}", output);
    assert!(output.contains("OPENQASM 3.0"));
    assert!(output.contains("reset q[0];"));
    assert!(output.contains("cx q[0], q[1];"));
    assert!(output.contains("c[0] = measure q[0];"));
}
