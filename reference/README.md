# CQAM Reference Documentation

This directory contains the reference documentation for the Classical-Quantum
Abstract Machine (CQAM).

## Contents

- [ISA Reference Card](./isa.md) — Every instruction mnemonic, operand
  signature, operation description, and encoding format. Includes tables of all
  named constants (distribution IDs, kernel IDs, PSW flag IDs, trap IDs,
  reduction function IDs, observation mode IDs, file selector IDs) and the
  binary `.cqb` file format.

- [Machine Specification](./spec.md) — Formal machine model: register files,
  memory banks, program status word, interrupt model, quantum simulation model
  (density matrices, kernels, fidelity metrics), hybrid execution model, and
  formal operational semantics.

- [Binary Opcode Reference](./opcodes.md) — 32-bit instruction word encoding
  formats and the complete opcode table with hex assignments for all 70+
  instructions.

- [QASM Generation Semantics](./qasm.md) — How the `cqam-codegen` crate
  translates CQAM instructions into OpenQASM 3.0 output, including the
  scan-declare-emit pipeline, register declaration mapping, kernel template
  expansion, and gate stub generation.

- [Instruction Examples](./examples.md) — Syntax reference and usage examples
  for every CQAM instruction type, including observation modes, masked gate
  operations, and amplitude encoding.

---

For build instructions and a project overview, see the top-level
[README.md](../README.md). API documentation is generated from inline Rust doc
comments via `cargo doc --workspace --no-deps`.
