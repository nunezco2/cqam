# CQAM -- Classical-Quantum Abstract Machine

CQAM is a register-based virtual machine that combines classical integer,
floating-point, and complex arithmetic with an ensemble/probability quantum
model and a hybrid classical-quantum execution layer. All instructions encode
into 32-bit fixed-width words with an 8-bit opcode prefix.

## Architecture

```
  .cqam source          binary (.cqb)          OpenQASM 3.0
       |                     |                      |
       v                     v                      v
  +---------+          +---------+            +-----------+
  | parser  |--------->|  cqam-as |----------->| cqam2qasm |
  | (cqam-  |  IR      | (assemb- | .cqb      | (codegen)  |
  |  core)  |          |  ler)    |            +-----------+
  +---------+          +---------+
       |                     |
       v                     v
  +---------+          +---------+
  | cqam-vm |<---------|cqam-run |
  | (exec   |  load    | (CLI    |
  |  engine) |          |  runner)|
  +---------+          +---------+
       |
       v
  +---------+
  | cqam-sim|
  | (quantum|
  |  sim)   |
  +---------+
```

**Pipeline:** Text source is parsed into an IR (`Vec<Instruction>`) by
`cqam-core`. The assembler (`cqam-as`) encodes the IR into 32-bit binary.
The runner (`cqam-run`) loads source or binary and executes it on the VM.
The codegen tool (`cqam2qasm`) translates IR to OpenQASM 3.0.

## Workspace Crates

| Crate | Type | Description |
|-------|------|-------------|
| `cqam-core` | library | ISA definition, parser, opcode encoding/decoding, error types |
| `cqam-sim` | library | Quantum simulator: density matrices, kernels, QDist |
| `cqam-vm` | library | Execution engine, PSW, ISR table, resource tracker |
| `cqam-run` | binary | CLI runner: loads programs, runs the VM, prints reports |
| `cqam-as` | binary | Assembler and disassembler for the 32-bit binary format |
| `cqam-codegen` | library | QASM emission pipeline (scan, declare, emit) |
| `cqam2qasm` | binary | CLI tool to convert `.cqam` source to OpenQASM 3.0 |

## Building

Requires Rust 2024 edition (1.85+).

```bash
cargo build --workspace
```

Run the test suite:

```bash
cargo test --workspace
```

Run the linter:

```bash
cargo clippy --workspace --all-targets
```

## Usage

### Running a CQAM program

```bash
cargo run --bin cqam-run -- --input examples/arithmetic.cqam --print-final-state
```

Options:
- `--input <path>` -- path to a `.cqam` source file (required)
- `--print-final-state` -- dump register and memory state after execution
- `--psw-report` -- print PSW flag summary
- `--resource-usage` -- print cumulative resource tracker
- `--config <path>` -- path to TOML simulator configuration

To see log output from the VM (warnings, errors), set the `RUST_LOG`
environment variable:

```bash
RUST_LOG=info cargo run --bin cqam-run -- --input examples/grover.cqam --print-final-state
```

### Assembling to binary

```bash
cargo run --bin cqam-as -- --assemble --input examples/arithmetic.cqam --output out.cqb
```

Disassemble a binary:

```bash
cargo run --bin cqam-as -- --disassemble --input out.cqb
```

### Generating OpenQASM

```bash
cargo run --bin cqam2qasm -- examples/quantum_observe.cqam
```

## Example Programs

The `examples/` directory contains five sample programs:

| File | Description |
|------|-------------|
| `arithmetic.cqam` | Integer/float arithmetic, memory operations, type conversion |
| `quantum_observe.cqam` | QPREP, QKERNEL, QOBSERVE, HREDUCE pipeline |
| `hybrid_fork.cqam` | HFORK/HCEXEC/HMERGE conditional execution |
| `grover.cqam` | Full Grover search with iteration loop |
| `bell_state.cqam` | Bell state preparation and measurement |

## Documentation

Generate and open the full API reference (all crates):

```bash
cargo doc --workspace --no-deps --open
```

The HTML documentation is written to `target/doc/`. Key entry points:

- `cqam_core` -- ISA, parser, opcode encoding, error types
- `cqam_sim` -- density matrix quantum simulation, kernels, QDist
- `cqam_vm` -- execution engine, PSW, ISR, resource tracker
- `cqam_run` -- program runner and report printer
- `cqam_as` -- assembler, disassembler, binary I/O
- `cqam_codegen` -- OpenQASM 3.0 code generation

## Quantum Model

CQAM uses the **density matrix** formalism for all quantum register operations.
A quantum register `Q[k]` holds a 2^n x 2^n complex Hermitian matrix rho, where
n is the number of qubits. Key properties:

- Tr(rho) = 1 (normalised)
- rho is positive semi-definite (valid probability interpretation)
- Purity Tr(rho^2) is 1 for pure states and 1/dim for maximally mixed states

Quantum gates are applied as unitary conjugations: rho' = U rho U†. Measurement
extracts the diagonal probabilities p_k = Re(rho_kk) (the Born rule) and collapses
rho to the projector |outcome><outcome|. The measurement result is stored as a
`HybridValue::Dist` in the hybrid register file, where it can be reduced to a
classical value by `HREDUCE`.

## Reference Documentation

Detailed documentation is in the `reference/` directory:

- [ISA Reference Card](reference/isa.md) -- complete instruction set, encoding, named constants
- [Machine Specification](reference/spec.md) -- register files, memory, interrupts
- [Binary Opcode Reference](reference/opcodes.md) -- encoding formats and opcode table
- [QASM Generation Semantics](reference/qasm.md) -- codegen pipeline and templates
- [Instruction Examples](reference/examples.md) -- syntax and usage for all instructions

## QASM Kernel Templates

The `kernels/qasm_templates/` directory contains OpenQASM 3.0 gate body
templates for each implemented quantum kernel. These are expanded inline
by `cqam2qasm` when template expansion is enabled.

## License

See LICENSE file for details.
