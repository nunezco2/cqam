# CQAM: A Classical-Quantum Abstract Machine

## Introduction

CQAM is a formal instruction set architecture and virtual machine designed to
model the integration of classical and quantum computing within a unified
execution environment. Unlike gate-level quantum assembly languages (e.g.,
OpenQASM, Quil), CQAM operates at the *systems* level: it defines a complete
machine model with classical register files, memory banks, a hardware call
stack, an interrupt controller, and a program status word alongside first-class
quantum registers and kernel-based quantum operations. The architecture is
motivated by the observation that practical quantum computation is inherently
hybrid — quantum processors do not operate in isolation but are orchestrated by
classical control logic that prepares inputs, dispatches quantum kernels,
interprets measurement results, and makes branching decisions conditioned on
those results.

CQAM provides a concrete abstraction for studying this classical-quantum
interface. Its instruction set captures the full lifecycle of a hybrid
computation:

1. **Classical setup.** Integer, floating-point, and complex arithmetic
   instructions prepare parameters, loop counters, and addresses using
   conventional register-to-register operations.

2. **Quantum state preparation.** The `QPREP` instruction initialises a
   quantum register to a named initial state (uniform superposition, zero
   state, Bell pair, GHZ state), producing a density matrix in the quantum
   register file.

3. **Quantum evolution.** The `QKERNEL` instruction applies a unitary
   transformation — selected by kernel ID and parameterised by classical
   register values — to a quantum register via the conjugation
   ρ′ = U ρ U†. Kernels (Fourier, Grover iteration, diffusion, entanglement)
   encapsulate multi-gate circuits as atomic ISA-level operations, reflecting
   the coprocessor model used by real quantum control systems.

4. **Measurement.** `QOBSERVE` performs a projective measurement under the
   Born rule, collapsing the density matrix to a basis state and storing the
   outcome distribution in the hybrid register file.

5. **Classical post-processing.** The `HREDUCE` instruction applies one of 14
   reduction functions (mean, mode, variance, magnitude, phase, etc.) to
   extract a classical scalar from the measurement distribution, depositing it
   in an integer or floating-point register for subsequent classical use.

6. **Hybrid control flow.** `HFORK` and `HMERGE` delimit parallel execution
   regions. `HCEXEC` provides conditional branching on PSW flags set by
   quantum operations, enabling measurement-dependent classical control flow.

7. **Interrupt-driven error handling.** A two-level interrupt model (NMI and
   maskable traps) supports handler registration via `SETIV` and return via
   `RETI`, allowing the classical control layer to respond to arithmetic
   faults, quantum fidelity violations, and synchronisation failures.

All instructions encode into a fixed-width 32-bit word with an 8-bit opcode
prefix. The ISA comprises over 60 instructions across six functional groups:
integer arithmetic and logic, floating-point arithmetic, complex arithmetic,
quantum operations, hybrid bridging operations, and control flow.

## Quantum Simulation Model

CQAM represents quantum state using the **density matrix** formalism. Each
quantum register Q[k] holds a 2ⁿ × 2ⁿ complex Hermitian matrix ρ satisfying:

- **Normalisation:** Tr(ρ) = 1
- **Positivity:** ρ is positive semi-definite
- **Purity:** Tr(ρ²) = 1 for pure states; Tr(ρ²) = 1/dim for maximally mixed states

This representation is strictly more general than the statevector formalism: it
correctly models mixed states, decoherence, and partial-trace operations needed
for entanglement quantification. Quantum gates are applied as unitary
conjugations ρ′ = U ρ U†, preserving all density matrix invariants.
Measurement extracts the diagonal probabilities pₖ = Re(ρₖₖ) and collapses ρ
to the projector |k⟩⟨k| corresponding to the sampled outcome.

The simulator provides five quantum kernels as ISA-level operations:

| Kernel | Description |
|--------|-------------|
| Init | Re-initialise to uniform superposition |
| Entangle | Apply CNOT-based entanglement circuit |
| Fourier | Quantum Fourier transform (QFT) |
| Diffuse | Grover diffusion (inversion about the mean) |
| GroverIter | Complete Grover iteration (oracle + diffusion) |

Fidelity metrics — von Neumann entropy, purity, entanglement entropy via
partial trace — are computed after each kernel application and exposed through
the PSW, enabling the classical control layer to monitor quantum state quality
and trigger interrupts when thresholds are violated.

## Machine Architecture

```
                      ┌─────────────────────────────────┐
                      │       Classical Subsystem        │
                      │                                  │
                      │  R0-R15  (16 × i64)   integers  │
                      │  F0-F15  (16 × f64)   floats    │
                      │  Z0-Z15  (16 × C64)   complex   │
                      │                                  │
                      │  CMEM    (64K × i64)   memory    │
                      │  PSW     condition/trap flags    │
                      │  CS      call stack              │
                      │  ISR     interrupt vector table  │
                      ├──────────────────────────────────┤
                      │     Classical-Quantum Bridge     │
                      │                                  │
                      │  H0-H7  (8 × HybridValue)       │
                      │    QOBSERVE ↓   ↑ HREDUCE       │
                      ├──────────────────────────────────┤
                      │       Quantum Subsystem          │
                      │                                  │
                      │  Q0-Q7  (8 × DensityMatrix)     │
                      │  QMEM   (256 × DensityMatrix)   │
                      │                                  │
                      │  Kernels: Init, Entangle,        │
                      │   Fourier, Diffuse, GroverIter   │
                      └─────────────────────────────────┘
```

**Data flow.** Classical registers hold parameters that configure quantum
operations (kernel selection, iteration targets, loop counters). Quantum
operations produce density matrices in the Q-register file. Measurement
(`QOBSERVE`) bridges the quantum-to-classical boundary by projecting a density
matrix into a probability distribution stored in the hybrid register file.
Reduction (`HREDUCE`) completes the bridge by extracting a classical scalar
from the distribution. This layered architecture enforces the no-cloning
constraint: quantum state cannot be copied into the classical subsystem without
measurement.

## Toolchain

The CQAM toolchain is implemented as a Rust workspace comprising seven crates:

| Crate | Role | Description |
|-------|------|-------------|
| `cqam-core` | ISA definition | Instruction enum, text parser, 32-bit opcode encoding/decoding, error types, register and memory abstractions |
| `cqam-sim` | Quantum backend | Density matrix representation, complex arithmetic, the `Kernel` trait, five concrete kernel implementations, probability distributions |
| `cqam-vm` | Execution engine | Instruction dispatch, program status word, fork/merge parallelism, ISR table, resource accounting |
| `cqam-run` | CLI runner | Program loader, execution driver, state and resource reporting |
| `cqam-as` | Assembler | Two-pass assembler (label resolution + encoding), binary `.cqb` format reader/writer, disassembler |
| `cqam-codegen` | Code generation | Three-stage OpenQASM 3.0 emission pipeline (scan, declare, emit) with kernel template expansion |
| `cqam2qasm` | CLI translator | Command-line interface for CQAM-to-OpenQASM translation |

### Build and test

Requires Rust 2024 edition (rustc 1.85+).

```bash
cargo build --workspace            # compile all crates
cargo test --workspace             # run the full test suite (~840 tests)
cargo clippy --workspace           # static analysis
cargo doc --workspace --no-deps --open   # generate and view API documentation
```

### Running a program

```bash
cargo run --bin cqam-run -- --input examples/grover.cqam --print-final-state
```

| Flag | Effect |
|------|--------|
| `--input <path>` | Path to a `.cqam` source file (required) |
| `--print-final-state` | Dump all register files and non-zero memory after execution |
| `--psw-report` | Print the final program status word |
| `--resource-usage` | Print cumulative resource accounting (time, space, superposition, entanglement, interference) |
| `--config <path>` | Load a TOML simulator configuration (fidelity thresholds, qubit count, cycle limit) |

Set `RUST_LOG=info` for VM-level diagnostics.

### Assembling and disassembling

```bash
cargo run --bin cqam-as -- --assemble --input examples/arithmetic.cqam --output out.cqb
cargo run --bin cqam-as -- --disassemble --input out.cqb
```

The `.cqb` binary format uses a fixed header (magic number, version, entry
point, code length) followed by 32-bit instruction words and an optional debug
symbol section for label restoration during disassembly.

### Generating OpenQASM 3.0

```bash
cargo run --bin cqam2qasm -- examples/quantum_observe.cqam
```

The codegen pipeline translates CQAM quantum operations into valid OpenQASM 3.0
source. Classical-only instructions are emitted as structured comments.
Quantum kernel bodies are expanded from gate-level templates when template
expansion is enabled.

## Example Programs

| File | Description |
|------|-------------|
| `arithmetic.cqam` | Classical integer and floating-point arithmetic, memory load/store, type conversion |
| `quantum_observe.cqam` | Full quantum pipeline: state preparation, kernel application, measurement, hybrid reduction |
| `hybrid_fork.cqam` | Parallel execution with `HFORK`/`HMERGE` and conditional branching via `HCEXEC` |
| `grover.cqam` | Multi-iteration Grover search with classical loop control and measurement extraction |
| `bell_state.cqam` | Bell state (|Φ⁺⟩) preparation, measurement, and mode/mean extraction |

## Reference Documentation

| Document | Contents |
|----------|----------|
| [ISA Reference Card](reference/isa.md) | Complete instruction table, encoding formats, named constants |
| [Machine Specification](reference/spec.md) | Register files, memory banks, PSW, interrupt model, formal operational semantics |
| [Binary Encoding Reference](reference/opcodes.md) | 32-bit word layout, opcode table, bit-field assignments for all 15 encoding formats |
| [QASM Generation Semantics](reference/qasm.md) | Codegen pipeline, emit modes, kernel template expansion |
| [Instruction Syntax and Examples](reference/examples.md) | Text-format syntax for every instruction with annotated usage examples |

API documentation is generated from inline Rust doc comments:

```bash
cargo doc --workspace --no-deps --open
```

## License

See LICENSE file for details.
