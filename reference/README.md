# CQAM Reference Index

This directory contains the reference documentation for the Classical-Quantum
Abstract Machine.

## Contents

- [Machine Specification](./spec.md) -- Formal machine model, register files,
  memory banks, interrupt model, and execution semantics.
- [Binary Opcode Reference](./opcodes.md) -- 32-bit instruction word encoding
  formats and complete opcode table.
- [QASM Generation Semantics](./qasm.md) -- How CQAM instructions translate to
  OpenQASM 3.0 output, including template expansion.
- [Instruction Examples](./examples.md) -- Format and usage examples for all
  CQAM instruction types.

---
For additional resources, see the main project README or module-level docs in
`cqam-core` and `cqam-codegen` crates.
