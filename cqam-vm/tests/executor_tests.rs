// cqam-vm/tests/parse_instruction_tests.rs

use cqam_vm::executor::parse_instruction;
use cqam_core::instruction::Instruction;

#[test]
fn test_parse_instruction_variants() {
    let cases = vec![
        (
            "CL:LOAD R1, 42",
            Instruction::ClLoad {
                dst: "R1".into(),
                src: "42".into(),
            },
        ),
        (
            "CL:ADD R3, R1, R2",
            Instruction::ClAdd {
                dst: "R3".into(),
                lhs: "R1".into(),
                rhs: "R2".into(),
            },
        ),
        (
            "CL:SUB R4, R3, R1",
            Instruction::ClSub {
                dst: "R4".into(),
                lhs: "R3".into(),
                rhs: "R1".into(),
            },
        ),
        (
            "CL:STORE addrX, R4",
            Instruction::ClStore {
                addr: "addrX".into(),
                src: "R4".into(),
            },
        ),
        (
            "CL:JMP done_label",
            Instruction::ClJump {
                label: "done_label".into(),
            },
        ),
        (
            "CL:IF flag, then_label",
            Instruction::ClIf {
                pred: "flag".into(),
                label: "then_label".into(),
            },
        ),
        (
            "LABEL: mylabel",
            Instruction::Label("mylabel".into()),
        ),
    ];

    for (input, expected) in cases {
        let parsed = parse_instruction(input);
        assert_eq!(
            parsed, expected,
            "Failed to parse instruction correctly for input: {}",
            input
        );
    }
}
