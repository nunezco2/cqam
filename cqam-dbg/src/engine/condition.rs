//! Conditional expression parsing and evaluation for conditional breakpoints.
//!
//! Supports expressions of the form: `REG OP LITERAL`
//!
//! Where:
//! - REG is R0--R15, F0--F15, Z0--Z15 with .re/.im suffix, or PSW.FLAG
//! - OP is ==, !=, <, >, <=, >=
//! - LITERAL is a numeric value (integer or float)

use cqam_vm::context::ExecutionContext;

/// Comparison operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl CompOp {
    /// Parse a comparison operator from a string token.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "==" => Some(Self::Eq),
            "!=" => Some(Self::Ne),
            "<" => Some(Self::Lt),
            ">" => Some(Self::Gt),
            "<=" => Some(Self::Le),
            ">=" => Some(Self::Ge),
            _ => None,
        }
    }

    /// Evaluate this operator on two f64 values.
    pub fn eval_f64(&self, lhs: f64, rhs: f64) -> bool {
        match self {
            Self::Eq => (lhs - rhs).abs() < 1e-12,
            Self::Ne => (lhs - rhs).abs() >= 1e-12,
            Self::Lt => lhs < rhs,
            Self::Gt => lhs > rhs,
            Self::Le => lhs <= rhs,
            Self::Ge => lhs >= rhs,
        }
    }

    /// Display string for this operator.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Gt => ">",
            Self::Le => "<=",
            Self::Ge => ">=",
        }
    }
}

/// The left-hand side of a condition: which register/flag to read.
#[derive(Debug, Clone, PartialEq)]
pub enum CondLhs {
    /// Integer register R0--R15.
    IReg(u8),
    /// Float register F0--F15.
    FReg(u8),
    /// Complex register real part: Z0--Z15 .re
    ZRegRe(u8),
    /// Complex register imaginary part: Z0--Z15 .im
    ZRegIm(u8),
    /// PSW flag by ID (0=ZF, 1=NF, ... 7=HF).
    PswFlag(u8),
}

impl CondLhs {
    /// Parse a left-hand side from a string like "R3", "F0", "Z1.re", "PSW.ZF".
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();

        // PSW flags: PSW.ZF, PSW.NF, etc.
        if let Some(flag_name) = s.strip_prefix("PSW.") {
            let flag_id = match flag_name.to_uppercase().as_str() {
                "ZF" => 0u8,
                "NF" => 1,
                "OF" => 2,
                "PF" => 3,
                "QF" => 4,
                "SF" => 5,
                "EF" => 6,
                "HF" => 7,
                "DF" => 8,
                "CF" => 9,
                "FK" => 10,
                "MG" => 11,
                "IF" => 12,
                "AF" => 13,
                _ => return None,
            };
            return Some(Self::PswFlag(flag_id));
        }

        // Complex with .re/.im suffix.
        if let Some(base) = s.strip_suffix(".re") {
            if let Some(idx) = parse_reg_index(base, "Z") {
                return Some(Self::ZRegRe(idx));
            }
        }
        if let Some(base) = s.strip_suffix(".im") {
            if let Some(idx) = parse_reg_index(base, "Z") {
                return Some(Self::ZRegIm(idx));
            }
        }

        // Integer registers: R0--R15.
        if let Some(idx) = parse_reg_index(s, "R") {
            if idx <= 15 {
                return Some(Self::IReg(idx));
            }
        }

        // Float registers: F0--F15.
        if let Some(idx) = parse_reg_index(s, "F") {
            if idx <= 15 {
                return Some(Self::FReg(idx));
            }
        }

        None
    }

    /// Read the current value from the execution context as f64.
    pub fn read(&self, ctx: &ExecutionContext) -> f64 {
        match self {
            Self::IReg(idx) => ctx.iregs.regs[*idx as usize] as f64,
            Self::FReg(idx) => ctx.fregs.regs[*idx as usize],
            Self::ZRegRe(idx) => ctx.zregs.regs[*idx as usize].0,
            Self::ZRegIm(idx) => ctx.zregs.regs[*idx as usize].1,
            Self::PswFlag(id) => {
                if ctx.psw.get_flag(*id) { 1.0 } else { 0.0 }
            }
        }
    }

    /// Return a display name like "R3", "F0", "Z1.re", "PSW.ZF".
    pub fn display_name(&self) -> String {
        match self {
            Self::IReg(idx) => format!("R{}", idx),
            Self::FReg(idx) => format!("F{}", idx),
            Self::ZRegRe(idx) => format!("Z{}.re", idx),
            Self::ZRegIm(idx) => format!("Z{}.im", idx),
            Self::PswFlag(id) => {
                let name = match id {
                    0 => "ZF",
                    1 => "NF",
                    2 => "OF",
                    3 => "PF",
                    4 => "QF",
                    5 => "SF",
                    6 => "EF",
                    7 => "HF",
                    8 => "DF",
                    9 => "CF",
                    10 => "FK",
                    11 => "MG",
                    12 => "IF",
                    13 => "AF",
                    _ => "??",
                };
                format!("PSW.{}", name)
            }
        }
    }
}

/// Parse a register index from a string like "R3" given prefix "R".
fn parse_reg_index(s: &str, prefix: &str) -> Option<u8> {
    let upper = s.to_uppercase();
    let upper_prefix = prefix.to_uppercase();
    if let Some(idx_str) = upper.strip_prefix(&upper_prefix) {
        idx_str.parse::<u8>().ok()
    } else {
        None
    }
}

/// A complete condition expression: LHS OP RHS_LITERAL.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub lhs: CondLhs,
    pub op: CompOp,
    pub rhs: f64,
}

impl Condition {
    /// Parse a condition from a string like "R3 == 42" or "F0 > 3.14".
    ///
    /// The input is expected to have whitespace between tokens:
    /// `<register> <op> <literal>`
    pub fn parse(input: &str) -> Result<Self, String> {
        let tokens: Vec<&str> = input.split_whitespace().collect();
        if tokens.len() != 3 {
            return Err(format!(
                "Expected 3 tokens (REG OP VALUE), got {}: '{}'",
                tokens.len(),
                input
            ));
        }

        let lhs = CondLhs::parse(tokens[0]).ok_or_else(|| {
            format!(
                "Unknown register '{}'. Expected R0-R15, F0-F15, Z0-Z15.re/.im, or PSW.FLAG",
                tokens[0]
            )
        })?;

        let op = CompOp::parse(tokens[1]).ok_or_else(|| {
            format!(
                "Unknown operator '{}'. Expected ==, !=, <, >, <=, >=",
                tokens[1]
            )
        })?;

        let rhs: f64 = tokens[2].parse().map_err(|_| {
            format!("Invalid numeric literal: '{}'", tokens[2])
        })?;

        Ok(Self { lhs, op, rhs })
    }

    /// Evaluate the condition against the current execution context.
    pub fn evaluate(&self, ctx: &ExecutionContext) -> bool {
        let lhs_val = self.lhs.read(ctx);
        self.op.eval_f64(lhs_val, self.rhs)
    }

    /// Return a human-readable description of this condition.
    pub fn describe(&self) -> String {
        format!("{} {} {}", self.lhs.display_name(), self.op.as_str(), self.rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::instruction::Instruction;

    fn make_ctx() -> ExecutionContext {
        ExecutionContext::new(vec![Instruction::Halt])
    }

    #[test]
    fn test_parse_ireg() {
        assert_eq!(CondLhs::parse("R3"), Some(CondLhs::IReg(3)));
        assert_eq!(CondLhs::parse("R15"), Some(CondLhs::IReg(15)));
        assert_eq!(CondLhs::parse("r0"), Some(CondLhs::IReg(0)));
    }

    #[test]
    fn test_parse_freg() {
        assert_eq!(CondLhs::parse("F0"), Some(CondLhs::FReg(0)));
        assert_eq!(CondLhs::parse("f15"), Some(CondLhs::FReg(15)));
    }

    #[test]
    fn test_parse_zreg() {
        assert_eq!(CondLhs::parse("Z1.re"), Some(CondLhs::ZRegRe(1)));
        assert_eq!(CondLhs::parse("Z3.im"), Some(CondLhs::ZRegIm(3)));
    }

    #[test]
    fn test_parse_psw() {
        assert_eq!(CondLhs::parse("PSW.ZF"), Some(CondLhs::PswFlag(0)));
        assert_eq!(CondLhs::parse("PSW.HF"), Some(CondLhs::PswFlag(7)));
    }

    #[test]
    fn test_parse_condition() {
        let cond = Condition::parse("R3 == 42").unwrap();
        assert_eq!(cond.lhs, CondLhs::IReg(3));
        assert_eq!(cond.op, CompOp::Eq);
        assert_eq!(cond.rhs, 42.0);
    }

    #[test]
    fn test_parse_float_condition() {
        let cond = Condition::parse("F0 > 3.14").unwrap();
        assert_eq!(cond.lhs, CondLhs::FReg(0));
        assert_eq!(cond.op, CompOp::Gt);
        assert!((cond.rhs - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_evaluate_condition_true() {
        let mut ctx = make_ctx();
        ctx.iregs.regs[3] = 42;
        let cond = Condition::parse("R3 == 42").unwrap();
        assert!(cond.evaluate(&ctx));
    }

    #[test]
    fn test_evaluate_condition_false() {
        let ctx = make_ctx();
        let cond = Condition::parse("R3 == 42").unwrap();
        assert!(!cond.evaluate(&ctx));
    }

    #[test]
    fn test_evaluate_psw_condition() {
        let mut ctx = make_ctx();
        ctx.psw.zf = true;
        let cond = Condition::parse("PSW.ZF == 1").unwrap();
        assert!(cond.evaluate(&ctx));
    }

    #[test]
    fn test_parse_bad_input() {
        assert!(Condition::parse("R3 == ").is_err());
        assert!(Condition::parse("R3").is_err());
        assert!(Condition::parse("X0 == 1").is_err());
        assert!(Condition::parse("R3 ?? 1").is_err());
    }

    #[test]
    fn test_describe() {
        let cond = Condition::parse("R3 == 42").unwrap();
        assert_eq!(cond.describe(), "R3 == 42");
    }

    #[test]
    fn test_comp_ops() {
        assert!(CompOp::Lt.eval_f64(1.0, 2.0));
        assert!(!CompOp::Lt.eval_f64(2.0, 1.0));
        assert!(CompOp::Ge.eval_f64(3.0, 3.0));
        assert!(CompOp::Ne.eval_f64(1.0, 2.0));
    }
}
