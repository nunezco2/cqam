# cqam2qasm — CQAM to OpenQASM 3.0 Translator

A command-line tool for converting CQAM (Classical-Quantum Abstract Machine)
assembly files into OpenQASM 3.0 format. The translator is built on the
three-stage `cqam-codegen` pipeline: scan (identify quantum resources),
declare (emit register and gate declarations), and emit (translate each
instruction).

## Usage

```bash
cargo run --bin cqam2qasm -- <file.cqam> [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `-o <file>` | Write output to `<file>` (default: stdout) |
| `--fragment` | Emit the circuit body only — no QASM header, no register declarations, no gate stubs |
| `--expand` | Expand `QKERNEL` invocations to gate-level QASM templates |
| `--no-expand` | Emit opaque kernel stub calls instead of expanding templates (default) |
| `--doc` | Print the CQAM instruction reference and exit |

### Examples

Translate a program to a complete standalone QASM file with kernel expansion:

```bash
cargo run --bin cqam2qasm -- examples/basic/qrng.cqam -o qrng.qasm --expand
```

Emit only the circuit body for embedding in a larger QASM program:

```bash
cargo run --bin cqam2qasm -- examples/basic/swap_test.cqam --fragment
```

Print output to stdout:

```bash
cargo run --bin cqam2qasm -- examples/basic/ghz_verify.cqam --expand
```

## Emit Modes

**Full mode (default):** Emits a complete, valid OpenQASM 3.0 file including
the version header, `include "stdgates.inc"`, qubit and bit register
declarations derived from the CQAM quantum register usage, gate stub
definitions for each kernel invoked, and the translated circuit body.

**Fragment mode (`--fragment`):** Emits only the circuit body. Useful when
the CQAM snippet is to be embedded in a hand-written QASM context where
declarations already exist.

**Kernel expansion (`--expand`):** Rather than emitting a stub call such as
`fourier(q);`, the translator inlines the gate-level QASM template for each
kernel. Templates are available for all eleven CQAM kernels.

## Instruction Translation

CQAM instructions that have direct QASM equivalents — single-qubit gates
(QHADM, QFLIP, QPHASE, QROT), two-qubit gates (QCNOT, QCZ, QSWAP), and
measurement (QMEAS) — are translated literally. Classical instructions and
hybrid control-flow constructs (HFORK, HMERGE, HREDUCE) are emitted as
annotated comments; they have no QASM representation but are preserved in
the output for traceability.

## Dependencies

- `cqam-core` — instruction types and text parser
- `cqam-codegen` — three-stage QASM emission pipeline

## Testing

```bash
cargo test --package cqam2qasm
```

---

Built as part of the CQAM toolchain. For a project overview see the top-level
[README.md](../README.md).
