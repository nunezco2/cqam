//! Tests for `execute_instruction` and `run_program` across all instruction groups.

use cqam_core::instruction::Instruction;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;
use cqam_vm::fork::ForkManager;

// ===========================================================================
// Integer arithmetic
// ===========================================================================

#[test]
fn test_iadd_and_isub() {
    let program = vec![];
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();

    execute_instruction(&mut ctx, &Instruction::ILdi { dst: 0, imm: 2 }, &mut fm).unwrap();
    execute_instruction(&mut ctx, &Instruction::ILdi { dst: 1, imm: 3 }, &mut fm).unwrap();

    execute_instruction(&mut ctx, &Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 5);

    execute_instruction(&mut ctx, &Instruction::ISub { dst: 3, lhs: 1, rhs: 0 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 1);
}

#[test]
fn test_imul() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 6).unwrap();
    ctx.iregs.set(1, 7).unwrap();
    execute_instruction(&mut ctx, &Instruction::IMul { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 42);
}

#[test]
fn test_idiv_and_imod() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 17).unwrap();
    ctx.iregs.set(1, 5).unwrap();

    execute_instruction(&mut ctx, &Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 3);

    execute_instruction(&mut ctx, &Instruction::IMod { dst: 3, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 2);
}

#[test]
fn test_idiv_by_zero_sets_trap_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 42).unwrap();
    ctx.iregs.set(1, 0).unwrap();
    execute_instruction(&mut ctx, &Instruction::IDiv { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert!(ctx.psw.trap_arith);
    assert_eq!(ctx.iregs.get(2).unwrap(), 0); // safe default
}

#[test]
fn test_imod_by_zero_sets_trap_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 42).unwrap();
    ctx.iregs.set(1, 0).unwrap();
    execute_instruction(&mut ctx, &Instruction::IMod { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert!(ctx.psw.trap_arith);
    assert_eq!(ctx.iregs.get(2).unwrap(), 0);
}

// ===========================================================================
// Integer bitwise
// ===========================================================================

#[test]
fn test_iand_ior_ixor() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 0b1100).unwrap();
    ctx.iregs.set(1, 0b1010).unwrap();

    execute_instruction(&mut ctx, &Instruction::IAnd { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 0b1000);

    execute_instruction(&mut ctx, &Instruction::IOr { dst: 3, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 0b1110);

    execute_instruction(&mut ctx, &Instruction::IXor { dst: 4, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(4).unwrap(), 0b0110);
}

#[test]
fn test_inot() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 0).unwrap();
    execute_instruction(&mut ctx, &Instruction::INot { dst: 1, src: 0 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), -1);
}

#[test]
fn test_ishl_ishr() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 1).unwrap();

    execute_instruction(&mut ctx, &Instruction::IShl { dst: 1, src: 0, amt: 3 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), 8);

    execute_instruction(&mut ctx, &Instruction::IShr { dst: 2, src: 1, amt: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 4);
}

// ===========================================================================
// Integer memory
// ===========================================================================

#[test]
fn test_ildi() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    execute_instruction(&mut ctx, &Instruction::ILdi { dst: 0, imm: 42 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 42);
}

#[test]
fn test_ildi_negative() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    execute_instruction(&mut ctx, &Instruction::ILdi { dst: 0, imm: -32768 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), -32768);
}

#[test]
fn test_ildm_and_istr() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 99).unwrap();
    execute_instruction(&mut ctx, &Instruction::IStr { src: 0, addr: 500 }, &mut fm).unwrap();
    execute_instruction(&mut ctx, &Instruction::ILdm { dst: 1, addr: 500 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), 99);
}

// ===========================================================================
// Integer comparison
// ===========================================================================

#[test]
fn test_ieq() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 5).unwrap();
    ctx.iregs.set(1, 5).unwrap();
    ctx.iregs.set(2, 3).unwrap();

    execute_instruction(&mut ctx, &Instruction::IEq { dst: 3, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 1);

    execute_instruction(&mut ctx, &Instruction::IEq { dst: 3, lhs: 0, rhs: 2 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 0);
}

#[test]
fn test_ilt_igt() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 3).unwrap();
    ctx.iregs.set(1, 5).unwrap();

    execute_instruction(&mut ctx, &Instruction::ILt { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 1);

    execute_instruction(&mut ctx, &Instruction::IGt { dst: 3, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(3).unwrap(), 0);
}

// ===========================================================================
// Float arithmetic
// ===========================================================================

#[test]
fn test_fadd_fsub() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 1.5).unwrap();
    ctx.fregs.set(1, 2.5).unwrap();

    execute_instruction(&mut ctx, &Instruction::FAdd { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(2).unwrap() - 4.0).abs() < 1e-10);

    execute_instruction(&mut ctx, &Instruction::FSub { dst: 3, lhs: 1, rhs: 0 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(3).unwrap() - 1.0).abs() < 1e-10);
}

#[test]
fn test_fmul_fdiv() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 3.0).unwrap();
    ctx.fregs.set(1, 4.0).unwrap();

    execute_instruction(&mut ctx, &Instruction::FMul { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(2).unwrap() - 12.0).abs() < 1e-10);

    execute_instruction(&mut ctx, &Instruction::FDiv { dst: 3, lhs: 1, rhs: 0 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(3).unwrap() - (4.0 / 3.0)).abs() < 1e-10);
}

#[test]
fn test_fdiv_by_zero_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 1.0).unwrap();
    ctx.fregs.set(1, 0.0).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::FDiv { dst: 2, lhs: 0, rhs: 1 }, &mut fm);
    assert!(result.is_err());
}

// ===========================================================================
// Complex arithmetic
// ===========================================================================

#[test]
fn test_zadd_zsub() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.zregs.set(1, (3.0, 4.0)).unwrap();

    execute_instruction(&mut ctx, &Instruction::ZAdd { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.zregs.get(2).unwrap(), (4.0, 6.0));

    execute_instruction(&mut ctx, &Instruction::ZSub { dst: 3, lhs: 1, rhs: 0 }, &mut fm).unwrap();
    assert_eq!(ctx.zregs.get(3).unwrap(), (2.0, 2.0));
}

#[test]
fn test_zmul() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.zregs.set(1, (3.0, 4.0)).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZMul { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    let (re, im) = ctx.zregs.get(2).unwrap();
    assert!((re - (-5.0)).abs() < 1e-10);
    assert!((im - 10.0).abs() < 1e-10);
}

#[test]
fn test_zdiv_by_zero_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.zregs.set(1, (0.0, 0.0)).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::ZDiv { dst: 2, lhs: 0, rhs: 1 }, &mut fm);
    assert!(result.is_err());
}

// ===========================================================================
// Type conversion
// ===========================================================================

#[test]
fn test_cvtif() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 42).unwrap();
    execute_instruction(&mut ctx, &Instruction::CvtIF { dst_f: 0, src_i: 0 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(0).unwrap() - 42.0).abs() < 1e-10);
}

#[test]
fn test_cvtfi() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 3.7).unwrap();
    execute_instruction(&mut ctx, &Instruction::CvtFI { dst_i: 0, src_f: 0 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 3);
}

#[test]
fn test_cvtfz() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 5.0).unwrap();
    execute_instruction(&mut ctx, &Instruction::CvtFZ { dst_z: 0, src_f: 0 }, &mut fm).unwrap();
    assert_eq!(ctx.zregs.get(0).unwrap(), (5.0, 0.0));
}

#[test]
fn test_cvtzf() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (3.125, 2.625)).unwrap();
    execute_instruction(&mut ctx, &Instruction::CvtZF { dst_f: 0, src_z: 0 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(0).unwrap() - 3.125).abs() < 1e-10);
}

// ===========================================================================
// Configuration query
// ===========================================================================

#[test]
fn test_iqcfg_loads_qubit_count() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.config.default_qubits = 4;
    execute_instruction(&mut ctx, &Instruction::IQCfg { dst: 0 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 4);
    assert!(!ctx.psw.trap_arith);
    assert!(!ctx.psw.zf);
    assert!(!ctx.psw.nf);
}

#[test]
fn test_iqcfg_traps_on_zero() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.config.default_qubits = 0;
    execute_instruction(&mut ctx, &Instruction::IQCfg { dst: 0 }, &mut fm).unwrap();
    assert!(ctx.psw.trap_arith);
    assert_eq!(ctx.iregs.get(0).unwrap(), 0);
}

#[test]
fn test_iqcfg_traps_on_exceeds_dm_max() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.config.default_qubits = 17;
    ctx.config.force_density_matrix = true;
    execute_instruction(&mut ctx, &Instruction::IQCfg { dst: 0 }, &mut fm).unwrap();
    assert!(ctx.psw.trap_arith);
    assert_eq!(ctx.iregs.get(0).unwrap(), 0);
}

#[test]
fn test_iqcfg_allows_sv_above_dm_max() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.config.default_qubits = 20;
    ctx.config.force_density_matrix = false;
    execute_instruction(&mut ctx, &Instruction::IQCfg { dst: 0 }, &mut fm).unwrap();
    assert!(!ctx.psw.trap_arith);
    assert_eq!(ctx.iregs.get(0).unwrap(), 20);
}

#[test]
fn test_iqcfg_traps_on_exceeds_sv_max() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.config.default_qubits = 25; // > MAX_SV_QUBITS (24)
    ctx.config.force_density_matrix = false;
    execute_instruction(&mut ctx, &Instruction::IQCfg { dst: 0 }, &mut fm).unwrap();
    assert!(ctx.psw.trap_arith);
    assert_eq!(ctx.iregs.get(0).unwrap(), 0);
}

#[test]
fn test_iqcfg_boundary_dm_max() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.config.default_qubits = 16; // exactly MAX_QUBITS
    ctx.config.force_density_matrix = true;
    execute_instruction(&mut ctx, &Instruction::IQCfg { dst: 5 }, &mut fm).unwrap();
    assert!(!ctx.psw.trap_arith);
    assert_eq!(ctx.iregs.get(5).unwrap(), 16);
}

#[test]
fn test_iqcfg_boundary_sv_max() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.config.default_qubits = 24; // exactly MAX_SV_QUBITS
    ctx.config.force_density_matrix = false;
    execute_instruction(&mut ctx, &Instruction::IQCfg { dst: 3 }, &mut fm).unwrap();
    assert!(!ctx.psw.trap_arith);
    assert_eq!(ctx.iregs.get(3).unwrap(), 24);
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
    let mut fm = ForkManager::new();

    execute_instruction(&mut ctx, &program[0], &mut fm).unwrap();
    assert_eq!(ctx.pc, 2);
}

#[test]
fn test_jif_taken() {
    let program = vec![
        Instruction::Label("TARGET".into()),
        Instruction::Halt,
    ];
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 1).unwrap();
    ctx.pc = 0;

    execute_instruction(&mut ctx, &Instruction::Jif { pred: 0, target: "TARGET".into() }, &mut fm).unwrap();
    assert_eq!(ctx.pc, 0);
}

#[test]
fn test_jif_not_taken() {
    let program = vec![
        Instruction::Label("TARGET".into()),
        Instruction::Halt,
    ];
    let mut ctx = ExecutionContext::new(program);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 0).unwrap();
    ctx.pc = 0;

    execute_instruction(&mut ctx, &Instruction::Jif { pred: 0, target: "TARGET".into() }, &mut fm).unwrap();
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
    let mut fm = ForkManager::new();

    execute_instruction(&mut ctx, &program[0], &mut fm).unwrap();
    assert_eq!(ctx.pc, 2);
    assert_eq!(ctx.call_stack.len(), 1);

    ctx.advance_pc();
    execute_instruction(&mut ctx, &program[3], &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 42);

    execute_instruction(&mut ctx, &Instruction::Ret, &mut fm).unwrap();
    assert_eq!(ctx.pc, 1);
    assert_eq!(ctx.call_stack.len(), 0);
}

#[test]
fn test_halt_sets_trap() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    execute_instruction(&mut ctx, &Instruction::Halt, &mut fm).unwrap();
    assert!(ctx.psw.trap_halt);
}

#[test]
fn test_ret_empty_stack_halts() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    execute_instruction(&mut ctx, &Instruction::Ret, &mut fm).unwrap();
    assert!(ctx.psw.trap_halt);
}

// ===========================================================================
// PSW updates
// ===========================================================================

#[test]
fn test_arithmetic_sets_zero_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 5).unwrap();
    ctx.iregs.set(1, 5).unwrap();
    execute_instruction(&mut ctx, &Instruction::ISub { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert!(ctx.psw.zf);
}

#[test]
fn test_arithmetic_sets_negative_flag() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 3).unwrap();
    ctx.iregs.set(1, 5).unwrap();
    execute_instruction(&mut ctx, &Instruction::ISub { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
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
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();
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
    let mut fm = ForkManager::new();
    cqam_vm::executor::run_program(&mut ctx, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 3);
    assert!(ctx.psw.trap_halt);
}

// ===========================================================================
// Shift overflow tests (Fix 2.4)
// ===========================================================================

#[test]
fn test_ishl_amt_64_does_not_panic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 1).unwrap();
    // amt=64 is clamped to 63; 1 << 63 = i64::MIN
    execute_instruction(&mut ctx, &Instruction::IShl { dst: 1, src: 0, amt: 64 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), i64::MIN);
}

// --- Register-indirect memory ------------------------------------------------

#[test]
fn test_ildx_basic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.cmem.store(100, 42);
    ctx.iregs.set(1, 100).unwrap();
    execute_instruction(&mut ctx, &Instruction::ILdx { dst: 0, addr_reg: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 42);
}

#[test]
fn test_istrx_basic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 42).unwrap();
    ctx.iregs.set(1, 200).unwrap();
    execute_instruction(&mut ctx, &Instruction::IStrx { src: 0, addr_reg: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.cmem.load(200), 42);
}

#[test]
fn test_fldx_basic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    let val: f64 = 3.15;
    ctx.cmem.store(50, val.to_bits() as i64);
    ctx.iregs.set(1, 50).unwrap();
    execute_instruction(&mut ctx, &Instruction::FLdx { dst: 0, addr_reg: 1 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(0).unwrap() - 3.15).abs() < 1e-10);
}

#[test]
fn test_fstrx_basic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 3.15).unwrap();
    ctx.iregs.set(1, 60).unwrap();
    execute_instruction(&mut ctx, &Instruction::FStrx { src: 0, addr_reg: 1 }, &mut fm).unwrap();
    let stored = f64::from_bits(ctx.cmem.load(60) as u64);
    assert!((stored - 3.15).abs() < 1e-10);
}

#[test]
fn test_zldx_basic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    let re: f64 = 1.5;
    let im: f64 = 2.5;
    ctx.cmem.store(80, re.to_bits() as i64);
    ctx.cmem.store(81, im.to_bits() as i64);
    ctx.iregs.set(1, 80).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZLdx { dst: 0, addr_reg: 1 }, &mut fm).unwrap();
    let (got_re, got_im) = ctx.zregs.get(0).unwrap();
    assert!((got_re - 1.5).abs() < 1e-10);
    assert!((got_im - 2.5).abs() < 1e-10);
}

#[test]
fn test_zstrx_basic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.iregs.set(1, 90).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZStrx { src: 0, addr_reg: 1 }, &mut fm).unwrap();
    let re = f64::from_bits(ctx.cmem.load(90) as u64);
    let im = f64::from_bits(ctx.cmem.load(91) as u64);
    assert!((re - 1.0).abs() < 1e-10);
    assert!((im - 2.0).abs() < 1e-10);
}

#[test]
fn test_ildx_istrx_roundtrip() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 9999).unwrap();
    ctx.iregs.set(1, 300).unwrap();
    execute_instruction(&mut ctx, &Instruction::IStrx { src: 0, addr_reg: 1 }, &mut fm).unwrap();
    execute_instruction(&mut ctx, &Instruction::ILdx { dst: 2, addr_reg: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), 9999);
}

#[test]
fn test_fldx_fstrx_roundtrip() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, std::f64::consts::E).unwrap();
    ctx.iregs.set(1, 400).unwrap();
    execute_instruction(&mut ctx, &Instruction::FStrx { src: 0, addr_reg: 1 }, &mut fm).unwrap();
    execute_instruction(&mut ctx, &Instruction::FLdx { dst: 2, addr_reg: 1 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(2).unwrap() - std::f64::consts::E).abs() < 1e-15);
}

#[test]
fn test_zldx_zstrx_roundtrip() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (-3.7, 4.2)).unwrap();
    ctx.iregs.set(1, 500).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZStrx { src: 0, addr_reg: 1 }, &mut fm).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZLdx { dst: 2, addr_reg: 1 }, &mut fm).unwrap();
    let (re, im) = ctx.zregs.get(2).unwrap();
    assert!((re - (-3.7)).abs() < 1e-15);
    assert!((im - 4.2).abs() < 1e-15);
}

// -- Error cases: negative address --

#[test]
fn test_ildx_negative_address_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(1, -1).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::ILdx { dst: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Address out of range"), "Error message: {}", msg);
    assert!(msg.contains("ILDX"), "Error message should mention ILDX: {}", msg);
}

#[test]
fn test_istrx_negative_address_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 42).unwrap();
    ctx.iregs.set(1, -5).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::IStrx { src: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
}

#[test]
fn test_fldx_negative_address_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(1, -100).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::FLdx { dst: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
}

#[test]
fn test_fstrx_negative_address_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 1.0).unwrap();
    ctx.iregs.set(1, -1).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::FStrx { src: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
}

#[test]
fn test_zldx_negative_address_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(1, -1).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::ZLdx { dst: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
}

#[test]
fn test_zstrx_negative_address_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.iregs.set(1, -1).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::ZStrx { src: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
}

// -- Error cases: address too large --

#[test]
fn test_ildx_address_too_large_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(1, 70000).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::ILdx { dst: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("Address out of range"), "Error message: {}", msg);
}

#[test]
fn test_istrx_address_too_large_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 42).unwrap();
    ctx.iregs.set(1, 0x10000).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::IStrx { src: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
}

#[test]
fn test_fstrx_address_too_large_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 1.0).unwrap();
    ctx.iregs.set(1, 70000).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::FStrx { src: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
}

// -- Error cases: ZLDX/ZSTRX boundary (needs two cells, max_addr = 0xFFFE) --

#[test]
fn test_zldx_address_65535_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(1, 65535).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::ZLdx { dst: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("ZLDX"), "Error message should mention ZLDX: {}", msg);
}

#[test]
fn test_zstrx_address_65535_returns_error() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (1.0, 2.0)).unwrap();
    ctx.iregs.set(1, 65535).unwrap();
    let result = execute_instruction(&mut ctx, &Instruction::ZStrx { src: 0, addr_reg: 1 }, &mut fm);
    assert!(result.is_err());
}

// -- Boundary cases: max valid addresses --

#[test]
fn test_ildx_address_65535_succeeds() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.cmem.store(65535, 77);
    ctx.iregs.set(1, 65535).unwrap();
    execute_instruction(&mut ctx, &Instruction::ILdx { dst: 0, addr_reg: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 77);
}

#[test]
fn test_istrx_address_65535_succeeds() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 88).unwrap();
    ctx.iregs.set(1, 65535).unwrap();
    execute_instruction(&mut ctx, &Instruction::IStrx { src: 0, addr_reg: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.cmem.load(65535), 88);
}

#[test]
fn test_fldx_address_65535_succeeds() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    let val: f64 = 2.719;
    ctx.cmem.store(65535, val.to_bits() as i64);
    ctx.iregs.set(1, 65535).unwrap();
    execute_instruction(&mut ctx, &Instruction::FLdx { dst: 0, addr_reg: 1 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(0).unwrap() - 2.719).abs() < 1e-10);
}

#[test]
fn test_zldx_address_65534_succeeds() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    let re: f64 = 5.0;
    let im: f64 = 6.0;
    ctx.cmem.store(65534, re.to_bits() as i64);
    ctx.cmem.store(65535, im.to_bits() as i64);
    ctx.iregs.set(1, 65534).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZLdx { dst: 0, addr_reg: 1 }, &mut fm).unwrap();
    let (got_re, got_im) = ctx.zregs.get(0).unwrap();
    assert!((got_re - 5.0).abs() < 1e-10);
    assert!((got_im - 6.0).abs() < 1e-10);
}

#[test]
fn test_zstrx_address_65534_succeeds() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.zregs.set(0, (7.0, 8.0)).unwrap();
    ctx.iregs.set(1, 65534).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZStrx { src: 0, addr_reg: 1 }, &mut fm).unwrap();
    let re = f64::from_bits(ctx.cmem.load(65534) as u64);
    let im = f64::from_bits(ctx.cmem.load(65535) as u64);
    assert!((re - 7.0).abs() < 1e-10);
    assert!((im - 8.0).abs() < 1e-10);
}

// -- Boundary: address zero --

#[test]
fn test_ildx_address_zero_succeeds() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.cmem.store(0, 123);
    ctx.iregs.set(1, 0).unwrap();
    execute_instruction(&mut ctx, &Instruction::ILdx { dst: 0, addr_reg: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(0).unwrap(), 123);
}

#[test]
fn test_zldx_address_zero_succeeds() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.cmem.store(0, 1.0_f64.to_bits() as i64);
    ctx.cmem.store(1, 2.0_f64.to_bits() as i64);
    ctx.iregs.set(1, 0).unwrap();
    execute_instruction(&mut ctx, &Instruction::ZLdx { dst: 0, addr_reg: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.zregs.get(0).unwrap(), (1.0, 2.0));
}

// ===========================================================================
// Shift overflow tests (Fix 2.4)
// ===========================================================================

#[test]
fn test_ishr_amt_64_does_not_panic() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, i64::MIN).unwrap();
    // amt=64 is clamped to 63; i64::MIN >> 63 = -1 (arithmetic shift)
    execute_instruction(&mut ctx, &Instruction::IShr { dst: 1, src: 0, amt: 64 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), -1);
}

// --- Shift boundary and overflow ---------------------------------------------

#[test]
fn test_ishl_amt_zero() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 0xFF).unwrap();
    execute_instruction(&mut ctx, &Instruction::IShl { dst: 1, src: 0, amt: 0 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), 0xFF);
}

#[test]
fn test_ishr_amt_zero() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 0xFF).unwrap();
    execute_instruction(&mut ctx, &Instruction::IShr { dst: 1, src: 0, amt: 0 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), 0xFF);
}

#[test]
fn test_ishl_amt_63() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, 1).unwrap();
    execute_instruction(&mut ctx, &Instruction::IShl { dst: 1, src: 0, amt: 63 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), i64::MIN);
}

#[test]
fn test_ishr_amt_63() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, i64::MIN).unwrap();
    execute_instruction(&mut ctx, &Instruction::IShr { dst: 1, src: 0, amt: 63 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(1).unwrap(), -1);
}

#[test]
fn test_iadd_wrapping_overflow() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, i64::MAX).unwrap();
    ctx.iregs.set(1, 1).unwrap();
    execute_instruction(&mut ctx, &Instruction::IAdd { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), i64::MIN, "i64::MAX + 1 should wrap to i64::MIN");
}

#[test]
fn test_imul_wrapping_overflow() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, i64::MAX).unwrap();
    ctx.iregs.set(1, 2).unwrap();
    execute_instruction(&mut ctx, &Instruction::IMul { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), i64::MAX.wrapping_mul(2));
}

// ===========================================================================
// Transcendental float math (FSIN, FCOS, FATAN2, FSQRT)
// ===========================================================================

#[test]
fn test_fsin() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    let pi_half = std::f64::consts::FRAC_PI_2;
    ctx.fregs.set(0, pi_half).unwrap();
    execute_instruction(&mut ctx, &Instruction::FSin { dst: 1, src: 0 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(1).unwrap() - 1.0).abs() < 1e-10, "sin(pi/2) = 1");
}

#[test]
fn test_fcos() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 0.0).unwrap();
    execute_instruction(&mut ctx, &Instruction::FCos { dst: 1, src: 0 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(1).unwrap() - 1.0).abs() < 1e-10, "cos(0) = 1");
}

#[test]
fn test_fatan2() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 1.0).unwrap();
    ctx.fregs.set(1, 1.0).unwrap();
    execute_instruction(&mut ctx, &Instruction::FAtan2 { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    let expected = std::f64::consts::FRAC_PI_4;
    assert!((ctx.fregs.get(2).unwrap() - expected).abs() < 1e-10, "atan2(1,1) = pi/4");
}

#[test]
fn test_fsqrt() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, 9.0).unwrap();
    execute_instruction(&mut ctx, &Instruction::FSqrt { dst: 1, src: 0 }, &mut fm).unwrap();
    assert!((ctx.fregs.get(1).unwrap() - 3.0).abs() < 1e-10, "sqrt(9) = 3");
}

#[test]
fn test_fsqrt_negative_sets_trap() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.fregs.set(0, -1.0).unwrap();
    execute_instruction(&mut ctx, &Instruction::FSqrt { dst: 1, src: 0 }, &mut fm).unwrap();
    assert!(ctx.fregs.get(1).unwrap().is_nan(), "sqrt(-1) should be NaN");
    assert!(ctx.psw.trap_arith, "sqrt of negative should set trap_arith");
}

#[test]
fn test_isub_wrapping_underflow() {
    let mut ctx = ExecutionContext::new(vec![]);
    let mut fm = ForkManager::new();
    ctx.iregs.set(0, i64::MIN).unwrap();
    ctx.iregs.set(1, 1).unwrap();
    execute_instruction(&mut ctx, &Instruction::ISub { dst: 2, lhs: 0, rhs: 1 }, &mut fm).unwrap();
    assert_eq!(ctx.iregs.get(2).unwrap(), i64::MAX, "i64::MIN - 1 should wrap to i64::MAX");
}
