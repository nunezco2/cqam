use cqam_core::instruction::Instruction;
use cqam_codegen::qasm::{QasmFormat, emit_qasm_program};

#[test]
fn test_qasm_format_classical_ops() {
    let load = Instruction::ClLoad { dst: "x".into(), src: "5".into() };
    let add = Instruction::ClAdd { dst: "z".into(), lhs: "x".into(), rhs: "y".into() };
    let sub = Instruction::ClSub { dst: "r".into(), lhs: "a".into(), rhs: "b".into() };

    assert_eq!(load.to_qasm(true), Some("// CL:LOAD x, 5\n    let x = 5;".to_string()));
    assert_eq!(add.to_qasm(true), Some("// CL:ADD z, x, y\n    let z = x + y;".to_string()));
    assert_eq!(sub.to_qasm(true), Some("// CL:SUB r, a, b\n    let r = a - b;".to_string()));
}

#[test]
fn test_qasm_format_control_flow() {
    let label = Instruction::Label("LOOP".into());
    let jmp = Instruction::ClJump { label: "LOOP".into() };
    let cond = Instruction::ClIf { pred: "flag".into(), label: "THEN".into() };

    assert_eq!(label.to_qasm(true), Some("// LABEL: LOOP".to_string()));
    assert_eq!(jmp.to_qasm(true), Some("// CL:JMP LOOP".to_string()));
    assert_eq!(cond.to_qasm(true), Some("// CL:IF flag, THEN\n    if (flag) { goto THEN; }".to_string()));
}

#[test]
fn test_qasm_format_hybrid() {
    let fork = Instruction::HybFork;
    let merge = Instruction::HybMerge;
    let cond_exec = Instruction::HybCondExec { flag: "hf".into(), then_label: "LBL".into() };
    let reduce = Instruction::HybReduce { src: "in".into(), dst: "out".into(), function: "round".into() };

    assert_eq!(fork.to_qasm(true), Some("// HYB: fork".to_string()));
    assert_eq!(merge.to_qasm(true), Some("// HYB: merge".to_string()));
    assert_eq!(cond_exec.to_qasm(true), Some("// HYB:COND_EXEC hf, LBL\n    if (hf) { goto LBL; }".to_string()));
    assert_eq!(reduce.to_qasm(true), Some("// HYB:REDUCE in, out, round\n    let out = round(in);".to_string()));
}

#[test]
fn test_qasm_format_quantum_variants() {
    let qprep = Instruction::QPrep { dst: "q1".into(), dist_src: "distA".into() };
    let qmeas = Instruction::QMeas { dst: "m1".into(), src: "q1".into() };
    let qobserve = Instruction::QObserve { dst: "obs1".into(), src: "q2".into() };

    let qkernel_basic = Instruction::QKernel {
        dst: "q3".into(),
        src: "q2".into(),
        kernel: "modexp".into(),
        ctx: None
    };

    let qkernel_ctx = Instruction::QKernel {
        dst: "q4".into(),
        src: "q2".into(),
        kernel: "modexp".into(),
        ctx: Some("qctx".into())
    };

    assert_eq!(qprep.to_qasm(true), Some("// QPREP: q1 from distA".to_string()));
    assert_eq!(qmeas.to_qasm(true), Some("// QMEAS m1, q1\n    m1 = measure q1[0];".to_string()));
    assert_eq!(qobserve.to_qasm(true), Some("// QOBSERVE obs1, q2".to_string()));
    assert_eq!(qkernel_basic.to_qasm(true), Some("// QKERNEL: q3 = modexp(q2)\n    q3 = modexp(q2);".to_string()));
    assert_eq!(qkernel_ctx.to_qasm(true), Some("// QKERNEL: q4 = modexp(q2) in context qctx\n    q4 = modexp(q2);".to_string()));
}

#[test]
fn test_emit_qasm_program() {
    let program = vec![
        Instruction::ClLoad { dst: "x".into(), src: "5".into() },
        Instruction::ClAdd { dst: "z".into(), lhs: "x".into(), rhs: "y".into() },
        Instruction::QPrep { dst: "q1".into(), dist_src: "distA".into() },
        Instruction::QMeas { dst: "m1".into(), src: "q1".into() },
    ];

    let output = emit_qasm_program(&program, true);
    assert!(output.contains("OPENQASM 3.0;"));
    assert!(output.contains("// CL:LOAD x, 5"));
    assert!(output.contains("let x = 5;"));
    assert!(output.contains("// CL:ADD z, x, y"));
    assert!(output.contains("let z = x + y;"));
    assert!(output.contains("// QPREP: q1 from distA"));
    assert!(output.contains("// QMEAS m1, q1"));
    assert!(output.contains("m1 = measure q1[0];"));
    assert!(output.contains("// === BEGIN CQAM GENERATED QASM ==="));
    assert!(output.contains("// === END CQAM GENERATED QASM ==="));
}

#[test]
fn test_emit_qasm_program_basic() {
    let program = vec![
        Instruction::Label("START".into()),
        Instruction::ClLoad { dst: "R1".into(), src: "5".into() },
        Instruction::ClAdd { dst: "R2".into(), lhs: "R1".into(), rhs: "10".into() },
        Instruction::ClStore { addr: "result".into(), src: "R2".into() },
        Instruction::Halt,
    ];

    let qasm_output = emit_qasm_program(&program, true);

    assert!(qasm_output.contains("// LABEL: START"));
    assert!(qasm_output.contains("OPENQASM 3.0;"));
    assert!(qasm_output.contains("// CL:LOAD R1, 5"));
    assert!(qasm_output.contains("let R1 = 5;"));
    assert!(qasm_output.contains("let R2 = R1 + 10;"));
    assert!(qasm_output.contains("result = R2;") || qasm_output.contains("// CL:STORE result, R2"));
    assert!(qasm_output.contains("// HALT"));
}

#[test]
fn test_qasm_kernel_function_emission() {
    let program = vec![
        Instruction::QKernel {
            dst: "qA".into(),
            src: "qX".into(),
            kernel: "apply_modexp".into(),
            ctx: None,
        },
        Instruction::QKernel {
            dst: "qB".into(),
            src: "qY".into(),
            kernel: "apply_modexp".into(),
            ctx: None,
        },
        Instruction::QKernel {
            dst: "qC".into(),
            src: "qZ".into(),
            kernel: "other_kernel".into(),
            ctx: None,
        },
    ];

    let qasm = emit_qasm_program(&program, true);

    print!("{}", qasm);

    // Ensure each kernel function appears only once
    let modexp_def_count = qasm.matches("def apply_modexp(qbit x)").count();
    let other_def_count = qasm.matches("def other_kernel(qbit x)").count();

    assert_eq!(modexp_def_count, 1);
    assert_eq!(other_def_count, 1);
    assert!(qasm.contains("apply_modexp(qX)") || qasm.contains("= apply_modexp(qX);"));
    assert!(qasm.contains("apply_modexp(qY)") || qasm.contains("= apply_modexp(qY);"));
    assert!(qasm.contains("other_kernel(qZ)") || qasm.contains("= other_kernel(qZ);"));
}

#[test]
fn test_qasm_format_hyb_cond_exec_expansion() {
    let instr = Instruction::HybCondExec {
        flag: "qf".into(),
        then_label: "THEN".into(),
    };

    let expected_qasm = "// HYB:COND_EXEC qf, THEN\n    if (qf) { goto THEN; }".to_string();
    assert_eq!(instr.to_qasm(true), Some(expected_qasm));
}
