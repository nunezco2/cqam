// cqam-core/tests/instruction_tests.rs

use cqam_core::instruction::Instruction;

#[test]
fn test_instruction_enum_basic() {
    let instr = Instruction::ClLoad {
        dst: "R1".to_string(),
        src: "42".to_string(),
    };
    assert_eq!(
        format!("{:?}", instr),
        "ClLoad { dst: \"R1\", src: \"42\" }"
    );
}
