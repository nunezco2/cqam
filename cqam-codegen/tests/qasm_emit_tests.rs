use cqam_core::instruction::Instruction;
use cqam_codegen::qasm::QasmFormat;
use cqam_codegen::qasm::emit_qasm_program;

#[test]
fn test_emit_qasm_format_for_cladd() {
    let instr = Instruction::ClAdd {
        dst: "R1".into(),
        lhs: "R2".into(),
        rhs: "R3".into(),
    };

    let qasm = instr.to_qasm(false).unwrap();
    assert!(qasm.contains("let R1 = R2 + R3"));
}

#[test]
fn test_emit_qasm_program_with_multiple_lines() {
    let program = vec![
        Instruction::Label("START".into()),
        Instruction::ClLoad { dst: "R1".into(), src: "42".into() },
        Instruction::ClAdd { dst: "R2".into(), lhs: "R1".into(), rhs: "5".into() },
        Instruction::ClStore { addr: "result".into(), src: "R2".into() },
        Instruction::Halt,
    ];

    let qasm_output = emit_qasm_program(&program, false);
    assert!(qasm_output.contains("OPENQASM 3.0;"));
    assert!(qasm_output.contains("let R1 = 42;"));
    assert!(qasm_output.contains("let R2 = R1 + 5;"));
    assert!(qasm_output.contains("result = R2;"));
    assert!(qasm_output.contains("// HALT"));
}

#[test]
fn test_emit_qasm_with_kernel_expansion() {
    let program = vec![Instruction::QKernel {
        dst: "qX".into(),
        src: "qA".into(),
        kernel: "QFourier".into(),
        ctx: None,
    }];

    let output = emit_qasm_program(&program, true);
    assert!(output.contains("QKERNEL: qX = QFourier(qA)"));
    assert!(output.contains("QFourier")); // Expect at least some line from template or placeholder
}
