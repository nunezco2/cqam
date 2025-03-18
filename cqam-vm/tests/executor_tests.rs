use cqam_core::instruction::Instruction;
use cqam_core::register::CValue;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;

#[test]
fn test_execute_add_and_sub() {
    let program = vec![
        Instruction::ClLoad { dst: "A".into(), src: "2".into() },
        Instruction::ClLoad { dst: "B".into(), src: "3".into() },
        Instruction::ClAdd { dst: "C".into(), lhs: "A".into(), rhs: "B".into() },
        Instruction::ClSub { dst: "D".into(), lhs: "B".into(), rhs: "A".into() },
    ];

    let mut ctx = ExecutionContext::new(program.clone());

    for instr in program {
        execute_instruction(&mut ctx, instr);
    }

    assert_eq!(ctx.registers.load_c("A"), Some(&CValue::Int(2)));
    assert_eq!(ctx.registers.load_c("B"), Some(&CValue::Int(3)));
    assert_eq!(ctx.registers.load_c("C"), Some(&CValue::Int(5)));
    assert_eq!(ctx.registers.load_c("D"), Some(&CValue::Int(1)));
}
