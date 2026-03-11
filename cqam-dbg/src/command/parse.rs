//! Tokenizer and grammar for the debugger command language.
//!
//! Supports case-insensitive commands with unambiguous prefix matching.
//! See `design/cqam-dbg-architecture.md` Section 3 for the full command spec.

use super::{
    Command, DeleteTarget, FocusTarget, InfoSubcommand, PrintTarget, RunTarget, UnwatchTarget,
};

/// All known top-level command names for prefix matching.
const COMMAND_NAMES: &[(&str, &[&str])] = &[
    ("step", &["s"]),
    ("next", &["n"]),
    ("continue", &["c"]),
    ("run", &[]),
    ("finish", &["fin"]),
    ("break", &["b"]),
    ("delete", &["del"]),
    ("enable", &[]),
    ("disable", &[]),
    ("watch", &[]),
    ("unwatch", &[]),
    ("print", &["p"]),
    ("info", &[]),
    ("set", &[]),
    ("focus", &[]),
    ("load", &[]),
    ("restart", &[]),
    ("quit", &["q", "exit"]),
    ("help", &["h"]),
];

/// Parse a command string into a `Command`.
///
/// Supports case-insensitive commands and unambiguous prefix matching.
pub fn parse_command(input: &str) -> Result<Command, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Command::Empty);
    }

    let tokens: Vec<&str> = input.split_whitespace().collect();
    let cmd_token = tokens[0].to_lowercase();
    let rest = &tokens[1..];

    // First try exact alias match.
    if let Some(resolved) = resolve_exact_alias(&cmd_token) {
        return dispatch_command(resolved, rest, input);
    }

    // Then try unambiguous prefix matching.
    let resolved = resolve_prefix(&cmd_token)?;
    dispatch_command(resolved, rest, input)
}

/// Check for exact alias matches (s, n, c, fin, b, del, p, q, h, exit).
fn resolve_exact_alias(token: &str) -> Option<&'static str> {
    for &(name, aliases) in COMMAND_NAMES {
        if token == name {
            return Some(name);
        }
        for &alias in aliases {
            if token == alias {
                return Some(name);
            }
        }
    }
    None
}

/// Resolve a prefix to a unique command name.
fn resolve_prefix(prefix: &str) -> Result<&'static str, String> {
    let mut matches: Vec<&str> = Vec::new();

    for &(name, _) in COMMAND_NAMES {
        if name.starts_with(prefix) {
            matches.push(name);
        }
    }

    match matches.len() {
        0 => Err(format!("Unknown command: '{}'", prefix)),
        1 => Ok(matches[0]),
        _ => {
            // Check if it's an exact match for one of them.
            for &m in &matches {
                if m == prefix {
                    return Ok(m);
                }
            }
            Err(format!(
                "Ambiguous command '{}': could be {}",
                prefix,
                matches.join(", ")
            ))
        }
    }
}

/// Dispatch to the appropriate command parser based on the resolved command name.
fn dispatch_command(name: &str, args: &[&str], _full_input: &str) -> Result<Command, String> {
    match name {
        "step" => parse_step(args),
        "next" => Ok(Command::Next),
        "continue" => Ok(Command::Continue),
        "run" => parse_run(args),
        "finish" => Ok(Command::Finish),
        "break" => parse_break(args),
        "delete" => parse_delete(args),
        "enable" => parse_enable(args),
        "disable" => parse_disable(args),
        "watch" => parse_watch(args),
        "unwatch" => parse_unwatch(args),
        "print" => parse_print(args),
        "info" => parse_info(args),
        "set" => parse_set(args),
        "focus" => parse_focus(args),
        "load" => parse_load(args),
        "restart" => Ok(Command::Restart),
        "quit" => Ok(Command::Quit),
        "help" => Ok(Command::Help(args.first().map(|s| s.to_string()))),
        _ => Err(format!("Unknown command: '{}'", name)),
    }
}

// ---------------------------------------------------------------------------
// Individual command parsers
// ---------------------------------------------------------------------------

fn parse_step(args: &[&str]) -> Result<Command, String> {
    let count = if args.is_empty() {
        1
    } else {
        args[0]
            .parse::<usize>()
            .map_err(|_| format!("Invalid step count: '{}'", args[0]))?
    };
    if count == 0 {
        return Err("Step count must be at least 1".to_string());
    }
    Ok(Command::Step(count))
}

fn parse_run(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Run);
    }
    if args.len() >= 2 && args[0].to_lowercase() == "to" {
        let target = parse_run_target(args[1])?;
        return Ok(Command::RunTo(target));
    }
    Err("Usage: run [to ADDR|LABEL]".to_string())
}

fn parse_run_target(s: &str) -> Result<RunTarget, String> {
    if let Some(addr) = parse_address(s) {
        Ok(RunTarget::Addr(addr))
    } else {
        // Treat as label name.
        Ok(RunTarget::Label(s.to_string()))
    }
}

fn parse_break(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: break ADDR|LABEL|class CLASSNAME [if COND]".to_string());
    }

    // break class CLASSNAME
    if args[0].to_lowercase() == "class" {
        if args.len() < 2 {
            return Err(
                "Usage: break class CLASSNAME (quantum|hybrid|branch|memory|ecall|float|complex)"
                    .to_string(),
            );
        }
        return Ok(Command::BreakClass(args[1].to_lowercase()));
    }

    // Find "if" keyword to split target from condition.
    let if_pos = args
        .iter()
        .position(|t| t.to_lowercase() == "if");
    let (target_args, cond_str) = if let Some(pos) = if_pos {
        if pos == 0 {
            return Err("Missing breakpoint target before 'if'".to_string());
        }
        let cond_tokens = &args[pos + 1..];
        if cond_tokens.is_empty() {
            return Err("Missing condition after 'if'".to_string());
        }
        (&args[..pos], Some(cond_tokens.join(" ")))
    } else {
        (args, None)
    };

    if target_args.len() != 1 {
        return Err("Expected a single address or label for breakpoint target".to_string());
    }

    let target = target_args[0];

    if let Some(addr) = parse_address(target) {
        Ok(Command::BreakAddr(addr, cond_str))
    } else {
        Ok(Command::BreakLabel(target.to_string(), cond_str))
    }
}

fn parse_delete(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: delete N|all".to_string());
    }
    if args[0].to_lowercase() == "all" {
        return Ok(Command::Delete(DeleteTarget::All));
    }
    let id = args[0]
        .parse::<usize>()
        .map_err(|_| format!("Invalid breakpoint ID: '{}'", args[0]))?;
    Ok(Command::Delete(DeleteTarget::Id(id)))
}

fn parse_enable(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: enable N".to_string());
    }
    let id = args[0]
        .parse::<usize>()
        .map_err(|_| format!("Invalid breakpoint ID: '{}'", args[0]))?;
    Ok(Command::Enable(id))
}

fn parse_disable(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: disable N".to_string());
    }
    let id = args[0]
        .parse::<usize>()
        .map_err(|_| format!("Invalid breakpoint ID: '{}'", args[0]))?;
    Ok(Command::Disable(id))
}

fn parse_watch(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: watch REG (e.g., R3, F0, Z1)".to_string());
    }
    Ok(Command::Watch(args[0].to_uppercase()))
}

fn parse_unwatch(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: unwatch REG|all".to_string());
    }
    if args[0].to_lowercase() == "all" {
        return Ok(Command::Unwatch(UnwatchTarget::All));
    }
    Ok(Command::Unwatch(UnwatchTarget::Register(
        args[0].to_uppercase(),
    )))
}

fn parse_print(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: print REG|CMEM[ADDR]|CMEM[ADDR..ADDR]".to_string());
    }

    let joined = args.join("");
    let lower = joined.to_lowercase();

    // CMEM[ADDR..ADDR] range
    if lower.starts_with("cmem[") && lower.ends_with(']') {
        let inner = &joined[5..joined.len() - 1]; // between [ and ]
        if let Some(dot_pos) = inner.find("..") {
            let lo_str = &inner[..dot_pos];
            let hi_str = &inner[dot_pos + 2..];
            let lo = parse_address(lo_str)
                .ok_or_else(|| format!("Invalid CMEM start address: '{}'", lo_str))?
                as u16;
            let hi = parse_address(hi_str)
                .ok_or_else(|| format!("Invalid CMEM end address: '{}'", hi_str))?
                as u16;
            return Ok(Command::Print(PrintTarget::CmemRange(lo, hi)));
        } else {
            let addr = parse_address(inner)
                .ok_or_else(|| format!("Invalid CMEM address: '{}'", inner))?
                as u16;
            return Ok(Command::Print(PrintTarget::CmemAddr(addr)));
        }
    }

    // Register name
    Ok(Command::Print(PrintTarget::Register(
        args[0].to_uppercase(),
    )))
}

fn parse_info(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err(
            "Usage: info breakpoints|watchpoints|registers|quantum|psw|resources|stack|labels|program"
                .to_string(),
        );
    }

    let sub = args[0].to_lowercase();

    // Allow unambiguous prefix matching for info subcommands.
    let sub_names = [
        "breakpoints",
        "watchpoints",
        "registers",
        "quantum",
        "psw",
        "resources",
        "stack",
        "labels",
        "program",
    ];

    let resolved = resolve_info_sub(&sub, &sub_names)?;

    match resolved {
        "breakpoints" => Ok(Command::Info(InfoSubcommand::Breakpoints)),
        "watchpoints" => Ok(Command::Info(InfoSubcommand::Watchpoints)),
        "registers" => {
            let file = args.get(1).map(|s| s.to_uppercase());
            Ok(Command::Info(InfoSubcommand::Registers(file)))
        }
        "quantum" => {
            if let Some(qarg) = args.get(1) {
                let upper = qarg.to_uppercase();
                if let Some(rest) = upper.strip_prefix('Q') {
                    let idx = rest
                        .parse::<u8>()
                        .map_err(|_| format!("Invalid Q register: '{}'", qarg))?;
                    if idx > 7 {
                        return Err(format!("Q register index out of range: {}", idx));
                    }
                    Ok(Command::Info(InfoSubcommand::Quantum(Some(idx))))
                } else {
                    Err(format!(
                        "Expected Q register name (e.g., Q0), got '{}'",
                        qarg
                    ))
                }
            } else {
                Ok(Command::Info(InfoSubcommand::Quantum(None)))
            }
        }
        "psw" => Ok(Command::Info(InfoSubcommand::Psw)),
        "resources" => Ok(Command::Info(InfoSubcommand::Resources)),
        "stack" => Ok(Command::Info(InfoSubcommand::Stack)),
        "labels" => Ok(Command::Info(InfoSubcommand::Labels)),
        "program" => Ok(Command::Info(InfoSubcommand::Program)),
        _ => Err(format!("Unknown info subcommand: '{}'", sub)),
    }
}

fn resolve_info_sub<'a>(prefix: &str, names: &[&'a str]) -> Result<&'a str, String> {
    // Exact match first.
    for &name in names {
        if name == prefix {
            return Ok(name);
        }
    }
    let matches: Vec<&str> = names.iter().filter(|n| n.starts_with(prefix)).copied().collect();
    match matches.len() {
        0 => Err(format!(
            "Unknown info subcommand: '{}'. Available: {}",
            prefix,
            names.join(", ")
        )),
        1 => Ok(matches[0]),
        _ => Err(format!(
            "Ambiguous info subcommand '{}': could be {}",
            prefix,
            matches.join(", ")
        )),
    }
}

fn parse_set(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: set threshold FLOAT | set topk N | set qreg QN".to_string());
    }

    let sub = args[0].to_lowercase();
    match sub.as_str() {
        "threshold" | "thresh" => {
            if args.len() < 2 {
                return Err("Usage: set threshold FLOAT".to_string());
            }
            let val: f64 = args[1]
                .parse()
                .map_err(|_| format!("Invalid threshold value: '{}'", args[1]))?;
            if !(0.0..=1.0).contains(&val) {
                return Err("Threshold must be between 0.0 and 1.0".to_string());
            }
            Ok(Command::SetThreshold(val))
        }
        "topk" => {
            if args.len() < 2 {
                return Err("Usage: set topk N".to_string());
            }
            let val: usize = args[1]
                .parse()
                .map_err(|_| format!("Invalid topk value: '{}'", args[1]))?;
            if val == 0 {
                return Err("topk must be at least 1".to_string());
            }
            Ok(Command::SetTopK(val))
        }
        "qreg" => {
            if args.len() < 2 {
                return Err("Usage: set qreg QN (e.g., Q0)".to_string());
            }
            let upper = args[1].to_uppercase();
            if let Some(rest) = upper.strip_prefix('Q') {
                let idx: u8 = rest
                    .parse()
                    .map_err(|_| format!("Invalid Q register: '{}'", args[1]))?;
                if idx > 7 {
                    return Err(format!("Q register index out of range: {}", idx));
                }
                Ok(Command::SetQReg(idx))
            } else {
                Err(format!(
                    "Expected Q register name (e.g., Q0), got '{}'",
                    args[1]
                ))
            }
        }
        _ => Err(format!(
            "Unknown set option: '{}'. Available: threshold, topk, qreg",
            sub
        )),
    }
}

fn parse_focus(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: focus code|state|quantum|output".to_string());
    }
    let pane = args[0].to_lowercase();
    match pane.as_str() {
        "code" => Ok(Command::Focus(FocusTarget::Code)),
        "state" => Ok(Command::Focus(FocusTarget::State)),
        "quantum" => Ok(Command::Focus(FocusTarget::Quantum)),
        "output" => Ok(Command::Focus(FocusTarget::Output)),
        _ => Err(format!(
            "Unknown pane: '{}'. Available: code, state, quantum, output",
            pane
        )),
    }
}

fn parse_load(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: load FILE".to_string());
    }
    Ok(Command::Load(args.join(" ")))
}

// ---------------------------------------------------------------------------
// Address parsing helpers
// ---------------------------------------------------------------------------

/// Parse a numeric address (decimal or 0xHEX). Returns None if not a number.
fn parse_address(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        usize::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<usize>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert!(matches!(parse_command("").unwrap(), Command::Empty));
        assert!(matches!(parse_command("  ").unwrap(), Command::Empty));
    }

    #[test]
    fn test_step_default() {
        assert!(matches!(parse_command("step").unwrap(), Command::Step(1)));
        assert!(matches!(parse_command("s").unwrap(), Command::Step(1)));
    }

    #[test]
    fn test_step_n() {
        assert!(matches!(parse_command("step 5").unwrap(), Command::Step(5)));
        assert!(matches!(parse_command("s 10").unwrap(), Command::Step(10)));
    }

    #[test]
    fn test_step_invalid() {
        assert!(parse_command("step abc").is_err());
        assert!(parse_command("step 0").is_err());
    }

    #[test]
    fn test_next() {
        assert!(matches!(parse_command("next").unwrap(), Command::Next));
        assert!(matches!(parse_command("n").unwrap(), Command::Next));
    }

    #[test]
    fn test_continue() {
        assert!(matches!(parse_command("continue").unwrap(), Command::Continue));
        assert!(matches!(parse_command("c").unwrap(), Command::Continue));
    }

    #[test]
    fn test_run() {
        assert!(matches!(parse_command("run").unwrap(), Command::Run));
    }

    #[test]
    fn test_run_to_addr() {
        match parse_command("run to 0x0064").unwrap() {
            Command::RunTo(RunTarget::Addr(addr)) => assert_eq!(addr, 0x0064),
            other => panic!("Expected RunTo(Addr), got {:?}", other),
        }
    }

    #[test]
    fn test_run_to_label() {
        match parse_command("run to loop").unwrap() {
            Command::RunTo(RunTarget::Label(name)) => assert_eq!(name, "loop"),
            other => panic!("Expected RunTo(Label), got {:?}", other),
        }
    }

    #[test]
    fn test_finish() {
        assert!(matches!(parse_command("finish").unwrap(), Command::Finish));
        assert!(matches!(parse_command("fin").unwrap(), Command::Finish));
    }

    #[test]
    fn test_break_addr() {
        match parse_command("break 0x0010").unwrap() {
            Command::BreakAddr(addr, None) => assert_eq!(addr, 0x0010),
            other => panic!("Expected BreakAddr, got {:?}", other),
        }
    }

    #[test]
    fn test_break_label() {
        match parse_command("break main").unwrap() {
            Command::BreakLabel(name, None) => assert_eq!(name, "main"),
            other => panic!("Expected BreakLabel, got {:?}", other),
        }
    }

    #[test]
    fn test_break_class() {
        match parse_command("break class quantum").unwrap() {
            Command::BreakClass(class) => assert_eq!(class, "quantum"),
            other => panic!("Expected BreakClass, got {:?}", other),
        }
    }

    #[test]
    fn test_break_conditional() {
        match parse_command("break 0x0015 if R3 == 42").unwrap() {
            Command::BreakAddr(addr, Some(cond)) => {
                assert_eq!(addr, 0x0015);
                assert_eq!(cond, "R3 == 42");
            }
            other => panic!("Expected conditional BreakAddr, got {:?}", other),
        }
    }

    #[test]
    fn test_break_alias() {
        assert!(matches!(
            parse_command("b 0x10").unwrap(),
            Command::BreakAddr(0x10, None)
        ));
    }

    #[test]
    fn test_delete() {
        assert!(matches!(
            parse_command("delete 3").unwrap(),
            Command::Delete(DeleteTarget::Id(3))
        ));
        assert!(matches!(
            parse_command("delete all").unwrap(),
            Command::Delete(DeleteTarget::All)
        ));
        assert!(matches!(
            parse_command("del 1").unwrap(),
            Command::Delete(DeleteTarget::Id(1))
        ));
    }

    #[test]
    fn test_enable_disable() {
        assert!(matches!(parse_command("enable 2").unwrap(), Command::Enable(2)));
        assert!(matches!(parse_command("disable 3").unwrap(), Command::Disable(3)));
    }

    #[test]
    fn test_watch() {
        match parse_command("watch R3").unwrap() {
            Command::Watch(reg) => assert_eq!(reg, "R3"),
            other => panic!("Expected Watch, got {:?}", other),
        }
    }

    #[test]
    fn test_unwatch() {
        match parse_command("unwatch F0").unwrap() {
            Command::Unwatch(UnwatchTarget::Register(reg)) => assert_eq!(reg, "F0"),
            other => panic!("Expected Unwatch(Register), got {:?}", other),
        }
        assert!(matches!(
            parse_command("unwatch all").unwrap(),
            Command::Unwatch(UnwatchTarget::All)
        ));
    }

    #[test]
    fn test_print_register() {
        match parse_command("print R3").unwrap() {
            Command::Print(PrintTarget::Register(reg)) => assert_eq!(reg, "R3"),
            other => panic!("Expected Print(Register), got {:?}", other),
        }
        match parse_command("p F0").unwrap() {
            Command::Print(PrintTarget::Register(reg)) => assert_eq!(reg, "F0"),
            other => panic!("Expected Print(Register), got {:?}", other),
        }
    }

    #[test]
    fn test_print_cmem() {
        match parse_command("print CMEM[0x10]").unwrap() {
            Command::Print(PrintTarget::CmemAddr(addr)) => assert_eq!(addr, 0x10),
            other => panic!("Expected Print(CmemAddr), got {:?}", other),
        }
    }

    #[test]
    fn test_print_cmem_range() {
        match parse_command("print CMEM[0..10]").unwrap() {
            Command::Print(PrintTarget::CmemRange(lo, hi)) => {
                assert_eq!(lo, 0);
                assert_eq!(hi, 10);
            }
            other => panic!("Expected Print(CmemRange), got {:?}", other),
        }
    }

    #[test]
    fn test_info_subcommands() {
        assert!(matches!(
            parse_command("info breakpoints").unwrap(),
            Command::Info(InfoSubcommand::Breakpoints)
        ));
        assert!(matches!(
            parse_command("info watchpoints").unwrap(),
            Command::Info(InfoSubcommand::Watchpoints)
        ));
        assert!(matches!(
            parse_command("info psw").unwrap(),
            Command::Info(InfoSubcommand::Psw)
        ));
        assert!(matches!(
            parse_command("info resources").unwrap(),
            Command::Info(InfoSubcommand::Resources)
        ));
        assert!(matches!(
            parse_command("info stack").unwrap(),
            Command::Info(InfoSubcommand::Stack)
        ));
        assert!(matches!(
            parse_command("info labels").unwrap(),
            Command::Info(InfoSubcommand::Labels)
        ));
        assert!(matches!(
            parse_command("info program").unwrap(),
            Command::Info(InfoSubcommand::Program)
        ));
    }

    #[test]
    fn test_info_registers_with_file() {
        match parse_command("info registers R").unwrap() {
            Command::Info(InfoSubcommand::Registers(Some(file))) => assert_eq!(file, "R"),
            other => panic!("Expected Info(Registers(Some)), got {:?}", other),
        }
    }

    #[test]
    fn test_info_quantum_detail() {
        match parse_command("info quantum Q0").unwrap() {
            Command::Info(InfoSubcommand::Quantum(Some(0))) => {}
            other => panic!("Expected Info(Quantum(Some(0))), got {:?}", other),
        }
    }

    #[test]
    fn test_info_prefix_matching() {
        assert!(matches!(
            parse_command("info br").unwrap(),
            Command::Info(InfoSubcommand::Breakpoints)
        ));
        assert!(matches!(
            parse_command("info w").unwrap(),
            Command::Info(InfoSubcommand::Watchpoints)
        ));
    }

    #[test]
    fn test_set_threshold() {
        match parse_command("set threshold 0.05").unwrap() {
            Command::SetThreshold(v) => assert!((v - 0.05).abs() < 1e-10),
            other => panic!("Expected SetThreshold, got {:?}", other),
        }
    }

    #[test]
    fn test_set_topk() {
        assert!(matches!(
            parse_command("set topk 8").unwrap(),
            Command::SetTopK(8)
        ));
    }

    #[test]
    fn test_set_qreg() {
        assert!(matches!(
            parse_command("set qreg Q2").unwrap(),
            Command::SetQReg(2)
        ));
    }

    #[test]
    fn test_focus() {
        assert!(matches!(
            parse_command("focus code").unwrap(),
            Command::Focus(FocusTarget::Code)
        ));
        assert!(matches!(
            parse_command("focus output").unwrap(),
            Command::Focus(FocusTarget::Output)
        ));
    }

    #[test]
    fn test_load() {
        match parse_command("load /path/to/file.cqam").unwrap() {
            Command::Load(path) => assert_eq!(path, "/path/to/file.cqam"),
            other => panic!("Expected Load, got {:?}", other),
        }
    }

    #[test]
    fn test_restart() {
        assert!(matches!(parse_command("restart").unwrap(), Command::Restart));
    }

    #[test]
    fn test_quit() {
        assert!(matches!(parse_command("quit").unwrap(), Command::Quit));
        assert!(matches!(parse_command("q").unwrap(), Command::Quit));
        assert!(matches!(parse_command("exit").unwrap(), Command::Quit));
    }

    #[test]
    fn test_help() {
        assert!(matches!(
            parse_command("help").unwrap(),
            Command::Help(None)
        ));
        match parse_command("help step").unwrap() {
            Command::Help(Some(topic)) => assert_eq!(topic, "step"),
            other => panic!("Expected Help(Some), got {:?}", other),
        }
    }

    #[test]
    fn test_case_insensitive() {
        assert!(matches!(parse_command("STEP").unwrap(), Command::Step(1)));
        assert!(matches!(parse_command("Continue").unwrap(), Command::Continue));
        assert!(matches!(parse_command("QUIT").unwrap(), Command::Quit));
    }

    #[test]
    fn test_prefix_matching() {
        assert!(matches!(parse_command("ste").unwrap(), Command::Step(1)));
        assert!(matches!(parse_command("cont").unwrap(), Command::Continue));
        assert!(matches!(parse_command("fini").unwrap(), Command::Finish));
    }

    #[test]
    fn test_ambiguous_prefix() {
        // "dis" could be "disable" only (not ambiguous now since we removed duplicates)
        // But "en" matches "enable" uniquely
        assert!(parse_command("en 1").is_ok());
    }

    #[test]
    fn test_unknown_command() {
        assert!(parse_command("foobar").is_err());
    }

    #[test]
    fn test_missing_args() {
        assert!(parse_command("break").is_err());
        assert!(parse_command("delete").is_err());
        assert!(parse_command("enable").is_err());
        assert!(parse_command("disable").is_err());
        assert!(parse_command("watch").is_err());
        assert!(parse_command("unwatch").is_err());
        assert!(parse_command("print").is_err());
        assert!(parse_command("info").is_err());
        assert!(parse_command("set").is_err());
        assert!(parse_command("focus").is_err());
        assert!(parse_command("load").is_err());
    }
}
