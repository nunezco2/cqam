//! Tests for conditional jump extensions: ICMP, ICMPI, JMPFN, JGT, JLE,
//! and all assembler aliases (JEQ/JNE/JLT/JGE/JZ/JNZ/JOV/JNO + quantum aliases).

use std::collections::HashMap;

use cqam_core::instruction::{Instruction, FlagId};
use cqam_core::opcode::{decode, encode};
use cqam_core::parser::parse_instruction;
use cqam_vm::context::ExecutionContext;
use cqam_vm::executor::execute_instruction;
use cqam_vm::fork::ForkManager;
use cqam_sim::backend::SimulationBackend;

// ===========================================================================
// Helpers
// ===========================================================================

fn make_ctx() -> (ExecutionContext, ForkManager, SimulationBackend) {
    let ctx = ExecutionContext::new(vec![]);
    let fm = ForkManager::new();
    let be = SimulationBackend::new();
    (ctx, fm, be)
}

fn exec(ctx: &mut ExecutionContext, instr: Instruction, fm: &mut ForkManager, be: &mut SimulationBackend) {
    execute_instruction(ctx, &instr, fm, be).unwrap();
}

fn roundtrip(instr: &Instruction, labels: &HashMap<String, u32>) -> Instruction {
    let word = encode(instr, labels).expect("encode failed");
    decode(word).expect("decode failed")
}

fn labels_map() -> HashMap<String, u32> {
    let mut m = HashMap::new();
    m.insert("target".to_string(), 42);
    m
}

// ===========================================================================
// A. ICMP -- compare equal, less-than, greater-than
// ===========================================================================

#[test]
fn icmp_equal_sets_zf() {
    let (mut ctx, mut fm, mut be) = make_ctx();
    ctx.iregs.set(0, 7).unwrap();
    ctx.iregs.set(1, 7).unwrap();
    exec(&mut ctx, Instruction::ICmp { lhs: 0, rhs: 1 }, &mut fm, &mut be);
    assert!(ctx.psw.zf, "ZF should be set when Ra == Rb");
    assert!(!ctx.psw.nf, "NF should be clear when Ra == Rb");
    assert!(!ctx.psw.of, "OF should be clear for equal values");
}

#[test]
fn icmp_less_than_sets_nf() {
    let (mut ctx, mut fm, mut be) = make_ctx();
    ctx.iregs.set(0, 3).unwrap();
    ctx.iregs.set(1, 9).unwrap();
    exec(&mut ctx, Instruction::ICmp { lhs: 0, rhs: 1 }, &mut fm, &mut be);
    assert!(!ctx.psw.zf, "ZF should be clear when Ra < Rb");
    assert!(ctx.psw.nf, "NF should be set when Ra < Rb");
}

#[test]
fn icmp_greater_than_clears_nf_zf() {
    let (mut ctx, mut fm, mut be) = make_ctx();
    ctx.iregs.set(0, 10).unwrap();
    ctx.iregs.set(1, 3).unwrap();
    exec(&mut ctx, Instruction::ICmp { lhs: 0, rhs: 1 }, &mut fm, &mut be);
    assert!(!ctx.psw.zf, "ZF should be clear when Ra > Rb");
    assert!(!ctx.psw.nf, "NF should be clear when Ra > Rb");
}

#[test]
fn icmp_does_not_write_registers() {
    // The result register is not written: R2 stays zero.
    let (mut ctx, mut fm, mut be) = make_ctx();
    ctx.iregs.set(0, 5).unwrap();
    ctx.iregs.set(1, 3).unwrap();
    exec(&mut ctx, Instruction::ICmp { lhs: 0, rhs: 1 }, &mut fm, &mut be);
    assert_eq!(ctx.iregs.get(2).unwrap(), 0);
}

// ===========================================================================
// B. ICMPI -- compare register against immediate
// ===========================================================================

#[test]
fn icmpi_equal_sets_zf() {
    let (mut ctx, mut fm, mut be) = make_ctx();
    ctx.iregs.set(0, 42).unwrap();
    exec(&mut ctx, Instruction::ICmpI { src: 0, imm: 42 }, &mut fm, &mut be);
    assert!(ctx.psw.zf);
    assert!(!ctx.psw.nf);
}

#[test]
fn icmpi_less_than_sets_nf() {
    let (mut ctx, mut fm, mut be) = make_ctx();
    ctx.iregs.set(0, 1).unwrap();
    exec(&mut ctx, Instruction::ICmpI { src: 0, imm: 100 }, &mut fm, &mut be);
    assert!(!ctx.psw.zf);
    assert!(ctx.psw.nf);
}

#[test]
fn icmpi_greater_than_clears_flags() {
    let (mut ctx, mut fm, mut be) = make_ctx();
    ctx.iregs.set(0, 200).unwrap();
    exec(&mut ctx, Instruction::ICmpI { src: 0, imm: 50 }, &mut fm, &mut be);
    assert!(!ctx.psw.zf);
    assert!(!ctx.psw.nf);
}

// ===========================================================================
// C. JMPFN -- jump when flag is NOT set
// ===========================================================================

#[test]
fn jmpfn_jumps_when_flag_clear() {
    // ZF is clear → JmpFN ZF should jump.
    let (mut ctx, mut fm, mut be) = make_ctx();
    ctx.psw.zf = false;
    // We need the label in the label table for jump_to_label to work.
    // ExecutionContext requires the program to have the label.
    // We'll use a program with a label at position 5.
    let mut ctx = ExecutionContext::new(vec![
        Instruction::Nop, Instruction::Nop, Instruction::Nop,
        Instruction::Nop, Instruction::Nop,
        Instruction::Label("target".to_string()),
        Instruction::Nop,
    ]);
    ctx.labels.insert("target".to_string(), 5);
    ctx.psw.zf = false;
    let old_pc = ctx.pc;
    execute_instruction(&mut ctx, &Instruction::JmpFN { flag: FlagId::Zf, target: "target".to_string() }, &mut fm, &mut be).unwrap();
    assert_eq!(ctx.pc, 5, "JmpFN should jump when flag is clear");
    let _ = old_pc; // suppress warning
}

#[test]
fn jmpfn_does_not_jump_when_flag_set() {
    let mut fm = ForkManager::new();
    let mut be = SimulationBackend::new();
    let mut ctx = ExecutionContext::new(vec![
        Instruction::Nop, Instruction::Label("target".to_string()),
    ]);
    ctx.labels.insert("target".to_string(), 1);
    ctx.psw.zf = true; // flag IS set → should NOT jump
    let start_pc = ctx.pc;
    execute_instruction(&mut ctx, &Instruction::JmpFN { flag: FlagId::Zf, target: "target".to_string() }, &mut fm, &mut be).unwrap();
    // PC advances by 1 (no jump taken)
    assert_eq!(ctx.pc, start_pc + 1, "JmpFN should NOT jump when flag is set");
}

// ===========================================================================
// D. JGT / JLE -- compound flag conditions
// ===========================================================================

#[test]
fn jgt_jumps_when_greater_than() {
    let mut fm = ForkManager::new();
    let mut be = SimulationBackend::new();
    let mut ctx = ExecutionContext::new(vec![
        Instruction::Nop, Instruction::Nop, Instruction::Nop,
        Instruction::Label("tgt".to_string()),
    ]);
    ctx.labels.insert("tgt".to_string(), 3);
    // ZF=0, NF=false, OF=false → NF==OF → JGT should jump
    ctx.psw.zf = false;
    ctx.psw.nf = false;
    ctx.psw.of = false;
    execute_instruction(&mut ctx, &Instruction::Jgt { target: "tgt".to_string() }, &mut fm, &mut be).unwrap();
    assert_eq!(ctx.pc, 3);
}

#[test]
fn jgt_does_not_jump_when_equal() {
    let mut fm = ForkManager::new();
    let mut be = SimulationBackend::new();
    let mut ctx = ExecutionContext::new(vec![
        Instruction::Nop, Instruction::Label("tgt".to_string()),
    ]);
    ctx.labels.insert("tgt".to_string(), 1);
    // ZF=1 → equal, NOT greater than
    ctx.psw.zf = true;
    ctx.psw.nf = false;
    ctx.psw.of = false;
    let pc_before = ctx.pc;
    execute_instruction(&mut ctx, &Instruction::Jgt { target: "tgt".to_string() }, &mut fm, &mut be).unwrap();
    assert_eq!(ctx.pc, pc_before + 1, "JGT should not jump when ZF=1");
}

#[test]
fn jle_jumps_when_equal() {
    let mut fm = ForkManager::new();
    let mut be = SimulationBackend::new();
    let mut ctx = ExecutionContext::new(vec![
        Instruction::Nop, Instruction::Nop,
        Instruction::Label("tgt".to_string()),
    ]);
    ctx.labels.insert("tgt".to_string(), 2);
    // ZF=1 → ZF=1 OR NF!=OF → should jump
    ctx.psw.zf = true;
    ctx.psw.nf = false;
    ctx.psw.of = false;
    execute_instruction(&mut ctx, &Instruction::Jle { target: "tgt".to_string() }, &mut fm, &mut be).unwrap();
    assert_eq!(ctx.pc, 2);
}

#[test]
fn jle_jumps_when_less_than() {
    let mut fm = ForkManager::new();
    let mut be = SimulationBackend::new();
    let mut ctx = ExecutionContext::new(vec![
        Instruction::Nop, Instruction::Nop,
        Instruction::Label("tgt".to_string()),
    ]);
    ctx.labels.insert("tgt".to_string(), 2);
    // NF=true, OF=false → NF!=OF → should jump (less-than case)
    ctx.psw.zf = false;
    ctx.psw.nf = true;
    ctx.psw.of = false;
    execute_instruction(&mut ctx, &Instruction::Jle { target: "tgt".to_string() }, &mut fm, &mut be).unwrap();
    assert_eq!(ctx.pc, 2);
}

#[test]
fn jle_does_not_jump_when_greater_than() {
    let mut fm = ForkManager::new();
    let mut be = SimulationBackend::new();
    let mut ctx = ExecutionContext::new(vec![
        Instruction::Nop, Instruction::Label("tgt".to_string()),
    ]);
    ctx.labels.insert("tgt".to_string(), 1);
    // ZF=0, NF=false, OF=false → NOT (ZF=1 OR NF!=OF) → should NOT jump
    ctx.psw.zf = false;
    ctx.psw.nf = false;
    ctx.psw.of = false;
    let pc_before = ctx.pc;
    execute_instruction(&mut ctx, &Instruction::Jle { target: "tgt".to_string() }, &mut fm, &mut be).unwrap();
    assert_eq!(ctx.pc, pc_before + 1, "JLE should not jump when ZF=0 and NF==OF");
}

// ===========================================================================
// E. Alias parsing (JEQ, JNE, JLT, JGE, JZ, JNZ, JOV, JNO)
// ===========================================================================

#[test]
fn alias_jeq_parses_to_jmpf_zf() {
    let instr = parse_instruction("JEQ my_label").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Zf, target: "my_label".to_string() });
}

#[test]
fn alias_jz_parses_to_jmpf_zf() {
    let instr = parse_instruction("JZ my_label").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Zf, target: "my_label".to_string() });
}

#[test]
fn alias_jne_parses_to_jmpfn_zf() {
    let instr = parse_instruction("JNE my_label").unwrap();
    assert_eq!(instr, Instruction::JmpFN { flag: FlagId::Zf, target: "my_label".to_string() });
}

#[test]
fn alias_jnz_parses_to_jmpfn_zf() {
    let instr = parse_instruction("JNZ my_label").unwrap();
    assert_eq!(instr, Instruction::JmpFN { flag: FlagId::Zf, target: "my_label".to_string() });
}

#[test]
fn alias_jlt_parses_to_jmpf_nf() {
    let instr = parse_instruction("JLT my_label").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Nf, target: "my_label".to_string() });
}

#[test]
fn alias_jge_parses_to_jmpfn_nf() {
    let instr = parse_instruction("JGE my_label").unwrap();
    assert_eq!(instr, Instruction::JmpFN { flag: FlagId::Nf, target: "my_label".to_string() });
}

#[test]
fn alias_jov_parses_to_jmpf_of() {
    let instr = parse_instruction("JOV my_label").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Of, target: "my_label".to_string() });
}

#[test]
fn alias_jno_parses_to_jmpfn_of() {
    let instr = parse_instruction("JNO my_label").unwrap();
    assert_eq!(instr, Instruction::JmpFN { flag: FlagId::Of, target: "my_label".to_string() });
}

// ===========================================================================
// F. Quantum alias parsing
// ===========================================================================

#[test]
fn alias_jqact_parses_to_jmpf_qf() {
    let instr = parse_instruction("JQACT lbl").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Qf, target: "lbl".to_string() });
}

#[test]
fn alias_jsup_parses_to_jmpf_sf() {
    let instr = parse_instruction("JSUP lbl").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Sf, target: "lbl".to_string() });
}

#[test]
fn alias_jent_parses_to_jmpf_ef() {
    let instr = parse_instruction("JENT lbl").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Ef, target: "lbl".to_string() });
}

#[test]
fn alias_jinf_parses_to_jmpf_if() {
    let instr = parse_instruction("JINF lbl").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::If, target: "lbl".to_string() });
}

#[test]
fn alias_jcol_parses_to_jmpf_cf() {
    let instr = parse_instruction("JCOL lbl").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Cf, target: "lbl".to_string() });
}

#[test]
fn alias_jdec_parses_to_jmpf_df() {
    let instr = parse_instruction("JDEC lbl").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Df, target: "lbl".to_string() });
}

#[test]
fn alias_jnrm_parses_to_jmpf_nw() {
    let instr = parse_instruction("JNRM lbl").unwrap();
    assert_eq!(instr, Instruction::JmpF { flag: FlagId::Nw, target: "lbl".to_string() });
}

// ===========================================================================
// G. Encode/decode round-trips for all 5 real opcodes
// ===========================================================================

#[test]
fn roundtrip_icmp() {
    let instr = Instruction::ICmp { lhs: 3, rhs: 7 };
    let word = encode(&instr, &HashMap::new()).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_icmpi() {
    let instr = Instruction::ICmpI { src: 2, imm: -100 };
    let word = encode(&instr, &HashMap::new()).unwrap();
    let decoded = decode(word).unwrap();
    assert_eq!(decoded, instr);
}

#[test]
fn roundtrip_jmpfn() {
    let lm = labels_map();
    let instr = Instruction::JmpFN { flag: FlagId::Nf, target: "target".to_string() };
    let decoded = roundtrip(&instr, &lm);
    // After round-trip the target becomes "@42"
    assert_eq!(decoded, Instruction::JmpFN { flag: FlagId::Nf, target: "@42".to_string() });
}

#[test]
fn roundtrip_jgt() {
    let lm = labels_map();
    let instr = Instruction::Jgt { target: "target".to_string() };
    let decoded = roundtrip(&instr, &lm);
    assert_eq!(decoded, Instruction::Jgt { target: "@42".to_string() });
}

#[test]
fn roundtrip_jle() {
    let lm = labels_map();
    let instr = Instruction::Jle { target: "target".to_string() };
    let decoded = roundtrip(&instr, &lm);
    assert_eq!(decoded, Instruction::Jle { target: "@42".to_string() });
}

// ===========================================================================
// H. FlagId::Nw gets_flag support
// ===========================================================================

#[test]
fn flagid_nw_get_flag() {
    use cqam_vm::psw::ProgramStateWord;
    let mut psw = ProgramStateWord::new();
    assert!(!psw.get_flag(14), "NW should be clear initially");
    psw.norm_warn = true;
    assert!(psw.get_flag(14), "NW should be set after norm_warn=true");
}

#[test]
fn flagid_nw_mnemonic() {
    assert_eq!(FlagId::Nw.mnemonic(), "NW");
    assert_eq!(FlagId::from_mnemonic("NW"), Some(FlagId::Nw));
}
