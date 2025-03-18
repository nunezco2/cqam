// cqam-vm/tests/cl_instruction_tests.rs

use cqam_core::instruction::Instruction;
use cqam_core::register::{CValue};
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;

#[test]
fn test_cl_load_and_add() {
    let program = vec![];
    let mut ctx = ExecutionContext::new(program);

    execute_instruction(&mut ctx, Instruction::ClLoad {
        dst: "R1".into(),
        src: "10".into(),
    });

    execute_instruction(&mut ctx, Instruction::ClLoad {
        dst: "R2".into(),
        src: "32".into(),
    });

    execute_instruction(&mut ctx, Instruction::ClAdd {
        dst: "R3".into(),
        lhs: "R1".into(),
        rhs: "R2".into(),
    });

    assert_eq!(ctx.registers.load_c("R3"), Some(&CValue::Int(42)));
}

#[test]
fn test_cl_sub_and_store() {
    let program = vec![];
    let mut ctx = ExecutionContext::new(program);

    execute_instruction(&mut ctx, Instruction::ClLoad {
        dst: "A".into(),
        src: "20".into(),
    });

    execute_instruction(&mut ctx, Instruction::ClLoad {
        dst: "B".into(),
        src: "5".into(),
    });

    execute_instruction(&mut ctx, Instruction::ClSub {
        dst: "C".into(),
        lhs: "A".into(),
        rhs: "B".into(),
    });

    execute_instruction(&mut ctx, Instruction::ClStore {
        addr: "mem1".into(),
        src: "C".into(),
    });

    assert_eq!(ctx.cmem.load("mem1"), Some(&15));
}
