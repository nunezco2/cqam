# cqam2qasm — CQAM to OpenQASM 3.0 CLI Tool

A command-line tool for converting CQAM (Classical-Quantum Abstract Machine) assembly files into OpenQASM 3.0 format.

## Usage

```bash
cargo run --bin cqam2qasm -- <input.cqam> <output.qasm>
```

- `input.cqam`: Path to the CQAM assembly source file.
- `output.qasm`: Destination file to write OpenQASM code.

### Example
```bash
cargo run --bin cqam2qasm -- examples/sample.cqam out.qasm
```

### Emit to stdout
```bash
cargo run --bin cqam2qasm -- examples/sample.cqam --emit
```

### Pipe from stdin
```bash
echo "CL:ADD x, a, b" | cargo run --bin cqam2qasm -- - --emit
```

## Directory Structure
```
cqam2qasm/
├── src/
│   └── main.rs
├── tests/
│   └── cli_test.rs
├── examples/
│   └── sample.cqam
```

## Features
- Classical and hybrid instruction translation
- Graceful fallbacks for unknown or incomplete input
- Auto-generates QASM headers and structure

## Dependencies
- `cqam-core`: Instruction types and parser
- `cqam-codegen`: QASM formatting utilities

## Testing
```bash
cargo test --package cqam2qasm
```

---
Built as part of the CQAM compiler stack.
