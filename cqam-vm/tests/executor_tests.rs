// cqam-vm/tests/executor_tests.rs
//
// Phase 4: Test the executor with Result-based error handling.

use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;

// ===========================================================================
// Integer arithmetic
// ===========================================================================

#[test]
fn test_iadd_and_isub() {
    let program = vec![];
    let mut ctx = ExecutionContext::new(program);

    execute_instruction(&mut ctx, &Instruction::ILdi { dst: 0, imm: 2 }).unwrap();
    execute_instruction(&mut ctx, &Instruction::ILdi { dst: 1, imm: 3 }).unwrap();

    execute_instruction(&mut ctx, &Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 5);

    execute_instruction(&mut ctx, &Instruction::ISub { dst: 3, lhs: 1, rhs: 0 }).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 1);
}

#[test]
fn test_imul() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 6).unwrap();
    ctx.iregs.set(1, 7).unwrap();
    execute_instruction(&mut ctx, &Instruction::IMul { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 42);
}

#[test]
fn test_idiv_and_imod() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 17).unwrap();
    ctx.iregs.set(1, 5).unwrap();

    execute_instruction(&mut ctx, &Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 3);

    execute_instruction(&mut ctx, &Instruction::IMod { dst: 3, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 2);
}

#[test]
fn test_idiv_by_zero_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 42).unwrap();
    ctx.iregs.set(1, 0).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Division by zero"));
}

#[test]
fn test_imod_by_zero_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 42).unwrap();
    ctx.iregs.set(1, 0).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::IMod { dst: 2, lhs: 0, rhs: 1 });
    assert!(result.is_err());
}

// ===========================================================================
// Integer bitwise
// ===========================================================================

#[test]
fn test_iand_ior_ixor() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 0b1100).unwrap();
    ctx.iregs.set(1, 0b1010).unwrap();

    execute_instruction(&mut ctx, &Instruction::IAnd { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 0b1000);

    execute_instruction(&mut ctx, &Instruction::IOr { dst: 3, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 0b1110);

    execute_instruction(&mut ctx, &Instruction::IXor { dst: 4, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(4).unwrap(), 0b0110);
}

#[test]
fn test_inot() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 0).unwrap();
    execute_instruction(&mut ctx, &Instruction::INot { dst: 1, src: 0 }).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), -1);
}

#[test]
fn test_ishl_ishr() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 1).unwrap();

    execute_instruction(&mut ctx, &Instruction::IShl { dst: 1, src: 0, amt: 3 }).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), 8);

    execute_instruction(&mut ctx, &Instruction::IShr { dst: 2, src: 1, amt: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 4);
}

// ===========================================================================
// Integer memory
// ===========================================================================

#[test]
fn test_ildi() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_instruction(&mut ctx, &Instruction::ILdi { dst: 0, imm: 42 }).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 42);
}

#[test]
fn test_ildi_negative() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_instruction(&mut ctx, &Instruction::ILdi { dst: 0, imm: -32768 }).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), -32768);
}

#[test]
fn test_ildm_and_istr() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 99).unwrap();
    execute_instruction(&mut ctx, &Instruction::IStr { src: 0, addr: 500 }).unwrap();
    execute_instruction(&mut ctx, &Instruction::ILdm { dst: 1, addr: 500 }).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), 99);
}

// ===========================================================================
// Integer comparison
// ===========================================================================

#[test]
fn test_ieq() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 5).unwrap();
    ctx.iregs.set(1, 5).unwrap();
    ctx.iregs.set(2, 3).unwrap();

    execute_instruction(&mut ctx, &Instruction::IEq { dst: 3, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 1);

    execute_instruction(&mut ctx, &Instruction::IEq { dst: 3, lhs: 0, rhs: 2 }).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 0);
}

#[test]
fn test_ilt_igt() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 3).unwrap();
    ctx.iregs.set(1, 5).unwrap();

    execute_instruction(&mut ctx, &Instruction::ILt { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 1);

    execute_instruction(&mut ctx, &Instruction::IGt { dst: 3, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 0);
}

// ===========================================================================
// Float arithmetic
// ===========================================================================

#[test]
fn test_fadd_fsub() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(0, 1.5).unwrap();
    ctx.fregs.set(1, 2.5).unwrap();

    execute_instruction(&mut ctx, &Instruction::FAdd { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert!((ctx.fregs.get(2).unwrap() - 4.0).abs() < 1e-10);

    execute_instruction(&mut ctx, &Instruction::FSub { dst: 3, lhs: 1, rhs: 0 }).unwrap();
    assert!((ctx.fregs.get(3).unwrap() - 1.0).abs() < 1e-10);
}

#[test]
fn test_fmul_fdiv() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(0, 3.0).unwrap();
    ctx.fregs.set(1, 4.0).unwrap();

    execute_instruction(&mut ctx, &Instruction::FMul { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert!((ctx.fregs.get(2).unwrap() - 12.0).abs() < 1e-10);

    execute_instruction(&mut ctx, &Instruction::FDiv { dst: 3, lhs: 1, rhs: 0 }).unwrap();
    assert!((ctx.fregs.get(3).unwrap() - (4.0 / 3.0)).abs() < 1e-10);
}

#[test]
fn test_fdiv_by_zero_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(0, 1.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::FDiv { dst: 2, lhs: 0, rhs: 1 });
    assert!(result.is_err());
}

// ===========================================================================
// Complex arithmetic
// ===========================================================================

#[test]
fn test_zadd_zsub() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.zregs.set(1, (3.0, 4.0)).unwrap();

    execute_instruction(&mut ctx, &Instruction::ZAdd { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert_eq!(ctx.zregs.get(2).unwrap(), (4.0, 6.0));

    execute_instruction(&mut ctx, &Instruction::ZSub { dst: 3, lhs: 1, rhs: 0 }).unwrap();
    assert_eq!(ctx.zregs.get(3).unwrap(), (2.0, 2.0));
}

#[test]
fn test_zmul() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.zregs.set(1, (3.0, 4.0)).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZMul { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    let (re, im) = ctx.zregs.get(2).unwrap();
    assert!((re - (-5.0)).abs() < 1e-10);
    assert!((im - 10.0).abs() < 1e-10);
}

#[test]
fn test_zdiv_by_zero_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::ZDiv { dst: 2, lhs: 0, rhs: 1 });
    assert!(result.is_err());
}

// ===========================================================================
// Type conversion
// ===========================================================================

#[test]
fn test_cvtif() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 42).unwrap();
    execute_instruction(&mut ctx, &Instruction::CvtIF { dst_f: 0, src_i: 0 }).unwrap();
    assert!((ctx.fregs.get(0).unwrap() - 42.0).abs() < 1e-10);
}

#[test]
fn test_cvtfi() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(0, 3.7).unwrap();
    execute_instruction(&mut ctx, &Instruction::CvtFI { dst_i: 0, src_f: 0 }).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 3);
}

#[test]
fn test_cvtfz() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.fregs.set(0, 5.0).unwrap();
    execute_instruction(&mut ctx, &Instruction::CvtFZ { dst_z: 0, src_f: 0 }).unwrap();
    assert_eq!(ctx.zregs.get(0).unwrap(), (5.0, 0.0));
}

#[test]
fn test_cvtzf() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.zregs.set(0, (3.125, 2.625)).unwrap();
    execute_instruction(&mut ctx, &Instruction::CvtZF { dst_f: 0, src_z: 0 }).unwrap();
    assert!((ctx.fregs.get(0).unwrap() - 3.125).abs() < 1e-10);
}

// ===========================================================================
// Control flow
// ===========================================================================

#[test]
fn test_jmp() {
    let program = vec![
        Instruction::Jmp { target: "END".into() },
        Instruction::ILdi { dst: 0, imm: 999 },
        Instruction::Label("END".into()),
        Instruction::ILdi { dst: 1, imm: 42 },
    ];
    let mut ctx = ExecutionContext::new(program.clone());

    execute_instruction(&mut ctx, &program[0]).unwrap();
    assert_eq!(ctx.pc, 2);
}

#[test]
fn test_jif_taken() {
    let program = vec![
        Instruction::Label("TARGET".into()),
        Instruction::Halt,
    ];
    let mut ctx = ExecutionContext::new(program);
    ctx.iregs.set(0, 1).unwrap();
    ctx.pc = 0;

    execute_instruction(&mut ctx, &Instruction::Jif { pred: 0, target: "TARGET".into() }).unwrap();
    assert_eq!(ctx.pc, 0);
}

#[test]
fn test_jif_not_taken() {
    let program = vec![
        Instruction::Label("TARGET".into()),
        Instruction::Halt,
    ];
    let mut ctx = ExecutionContext::new(program);
    ctx.iregs.set(0, 0).unwrap();
    ctx.pc = 0;

    execute_instruction(&mut ctx, &Instruction::Jif { pred: 0, target: "TARGET".into() }).unwrap();
    assert_eq!(ctx.pc, 1);
}

#[test]
fn test_call_and_ret() {
    let program = vec![
        Instruction::Call { target: "FUNC".into() },
        Instruction::Halt,
        Instruction::Label("FUNC".into()),
        Instruction::ILdi { dst: 0, imm: 42 },
        Instruction::Ret,
    ];
    let mut ctx = ExecutionContext::new(program.clone());

    execute_instruction(&mut ctx, &program[0]).unwrap();
    assert_eq!(ctx.pc, 2);
    assert_eq!(ctx.call_stack.len(), 1);

    ctx.advance_pc();
    execute_instruction(&mut ctx, &program[3]).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 42);

    execute_instruction(&mut ctx, &Instruction::Ret).unwrap();
    assert_eq!(ctx.pc, 1);
    assert_eq!(ctx.call_stack.len(), 0);
}

#[test]
fn test_halt_sets_trap() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_instruction(&mut ctx, &Instruction::Halt).unwrap();
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_ret_empty_stack_halts() {
    let mut ctx = ExecutionContext::new(vec![]);
    execute_instruction(&mut ctx, &Instruction::Ret).unwrap();
    assert!(ctx.psw.trap_halt);
}

// ===========================================================================
// PSW updates
// ===========================================================================

#[test]
fn test_arithmetic_sets_zero_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 5).unwrap();
    ctx.iregs.set(1, 5).unwrap();
    execute_instruction(&mut ctx, &Instruction::ISub { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert!(ctx.psw.zf);
}

#[test]
fn test_arithmetic_sets_negative_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 3).unwrap();
    ctx.iregs.set(1, 5).unwrap();
    execute_instruction(&mut ctx, &Instruction::ISub { dst: 2, lhs: 0, rhs: 1 }).unwrap();
    assert!(ctx.psw.nf);
}

// ===========================================================================
// run_program integration test
// ===========================================================================

#[test]
fn test_run_program_simple() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 10 },
        Instruction::ILdi { dst: 1, imm: 20 },
        Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 },
        Instruction::Halt,
    ];
    let mut ctx = ExecutionContext::new(program);
    cqam_vm::executor::run_program(&mut ctx).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 30);
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_run_program_with_loop() {
    let program = vec![
        Instruction::ILdi { dst: 0, imm: 0 },
        Instruction::ILdi { dst: 1, imm: 3 },
        Instruction::ILdi { dst: 2, imm: 1 },
        Instruction::Label("LOOP".into()),
        Instruction::IEq { dst: 3, lhs: 0, rhs: 1 },
        Instruction::Jif { pred: 3, target: "END".into() },
        Instruction::IAdd { dst: 0, lhs: 0, rhs: 2 },
        Instruction::Jmp { target: "LOOP".into() },
        Instruction::Label("END".into()),
        Instruction::Halt,
    ];
    let mut ctx = ExecutionContext::new(program);
    cqam_vm::executor::run_program(&mut ctx).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 3);
    assert!(ctx.psw.trap_halt);
}

// ===========================================================================
// Shift overflow tests (Fix 2.4)
// ===========================================================================

#[test]
fn test_ishl_amt_64_does_not_panic() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, 1).unwrap();
    // amt=64 is clamped to 63; 1 << 63 = i64::MIN
    execute_instruction(&mut ctx, &Instruction::IShl { dst: 1, src: 0, amt: 64 }).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), i64::MIN);
}

#[test]
fn test_ishr_amt_64_does_not_panic() {
    let mut ctx = ExecutionContext::new(vec![]);
    ctx.iregs.set(0, i64::MIN).unwrap();
    // amt=64 is clamped to 63; i64::MIN >> 63 = -1 (arithmetic shift)
    execute_instruction(&mut ctx, &Instruction::IShr { dst: 1, src: 0, amt: 64 }).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), -1);
}
