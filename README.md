# CQAM: Classical-Quantum Abstract Machine

## Overview

CQAM is a register-based virtual machine and instruction set architecture
designed to model the integration of classical and quantum computing within a
unified execution environment. Unlike gate-level quantum assembly languages
(OpenQASM, Quil), CQAM operates at the *systems* level: it defines a complete
machine model with classical register files, memory banks, a hardware call
stack, an interrupt controller, and a program status word alongside first-class
quantum registers and kernel-based quantum operations.

The architecture is motivated by the observation that practical quantum
computation is inherently hybrid — quantum processors do not operate in
isolation but are orchestrated by classical control logic that prepares inputs,
dispatches quantum kernels, interprets measurement results, and makes branching
decisions conditioned on those results.

The full lifecycle of a hybrid computation in CQAM:

1. **Classical setup.** Integer, floating-point, and complex arithmetic
   instructions prepare parameters and loop counters using conventional
   register-to-register operations.
2. **Quantum state preparation.** `QPREP` initialises a quantum register to a
   named state (uniform superposition, zero state, Bell pair, GHZ state),
   producing a density matrix in the quantum register file.
3. **Quantum evolution.** `QKERNEL` applies a unitary transformation — selected
   by kernel ID and parameterised by classical registers — via the conjugation
   rho' = U rho U†. Kernels (Fourier, Grover iteration, diffusion, entanglement,
   rotation, phase-shift, inverse QFT) encapsulate multi-gate circuits as
   atomic ISA-level operations.
4. **Measurement.** `QOBSERVE` performs a projective measurement under the Born
   rule, collapsing the density matrix to a basis state and storing the outcome
   in the hybrid register file.
5. **Classical post-processing.** `HREDUCE` applies one of 16 reduction
   functions (mean, mode, variance, magnitude, phase, etc.) to extract a
   classical scalar from the measurement distribution, depositing it in an
   integer or floating-point register.
6. **Hybrid control flow.** `HFORK` and `HMERGE` delimit parallel execution
   regions. `HCEXEC` provides conditional branching on PSW flags set by
   quantum operations.
7. **Interrupt-driven error handling.** A two-level interrupt model supports
   handler registration via `SETIV` and return via `RETI`, enabling the
   classical control layer to respond to arithmetic faults, quantum fidelity
   violations, and synchronisation failures.

All instructions encode into a fixed-width 32-bit word with an 8-bit opcode
prefix. The ISA comprises 70+ instructions across six functional groups.

## Quantum Simulation Model

CQAM represents quantum state using the **density matrix** formalism. Each
quantum register Q[k] holds a 2^n x 2^n complex Hermitian matrix rho
satisfying Tr(rho) = 1. This representation is strictly more general than the
statevector formalism: it correctly models mixed states, decoherence, and
partial-trace operations needed for entanglement quantification. Quantum gates
are applied as unitary conjugations rho' = U rho U†. Measurement extracts the
diagonal probabilities p_k = Re(rho_kk) and collapses rho to the projector
|k><k| corresponding to the sampled outcome.

Seven quantum kernels are provided as ISA-level operations:

| Kernel ID | Name | Description |
|-----------|------|-------------|
| 0 | init | Re-initialise to uniform superposition |
| 1 | entangle | CNOT-based entanglement circuit |
| 2 | fourier | Quantum Fourier Transform |
| 3 | diffuse | Grover diffusion (inversion about the mean) |
| 4 | grover_iter | Complete Grover iteration (oracle + diffusion) |
| 5 | rotate | Diagonal rotation: U[k][k] = exp(i * theta * k) |
| 6 | phase_shift | Phase shift: U[k][k] = exp(i * |z| * k) |

## Architecture

```
                  ┌─────────────────────────────────┐
                  │       Classical Subsystem        │
                  │                                  │
                  │  R0-R15  (16 × i64)   integers  │
                  │  F0-F15  (16 × f64)   floats    │
                  │  Z0-Z15  (16 × C64)   complex   │
                  │                                  │
                  │  CMEM    (64K × i64)   memory   │
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
                  │  Kernels: Fourier, Grover,       │
                  │   Diffuse, Rotate, PhaseShift,   │
                  │   Entangle, Init                 │
                  └─────────────────────────────────┘
```

This layered architecture enforces the no-cloning constraint: quantum state
cannot be copied into the classical subsystem without measurement.

## Crate Structure

The CQAM toolchain is implemented as a Rust workspace comprising seven crates:

| Crate | Role | Description |
|-------|------|-------------|
| `cqam-core` | ISA definition | Instruction enum, text parser, 32-bit opcode encoding and decoding, error types, register and memory abstractions |
| `cqam-sim` | Quantum backend | Density matrix representation, complex arithmetic, the `Kernel` trait, seven concrete kernel implementations |
| `cqam-vm` | Execution engine | Instruction dispatch, program status word, fork/merge parallelism, ISR table, resource accounting |
| `cqam-run` | CLI runner | Program loader, execution driver, state and resource reporting |
| `cqam-as` | Assembler | Two-pass assembler (label resolution + encoding), binary `.cqb` format reader/writer, disassembler |
| `cqam-codegen` | Code generation | Three-stage OpenQASM 3.0 emission pipeline (scan, declare, emit) with kernel template expansion |
| `cqam2qasm` | CLI translator | Command-line interface for CQAM-to-OpenQASM 3.0 translation |

## Build and Test

Requires Rust 2024 edition (rustc 1.85+).

```bash
# Compile all crates
cargo build --workspace

# Run the full test suite
cargo test --workspace

# Static analysis
cargo clippy --workspace

# Generate and view API documentation
cargo doc --workspace --no-deps --open
```

## Usage

### cqam-run: Execute a CQAM program

```
cargo run --bin cqam-run -- <file.cqam> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--config <path>` | Path to TOML simulator configuration file |
| `--qubits <n>` | Default qubits per quantum register (overrides config) |
| `--max-cycles <n>` | Maximum instruction cycles before forced halt |
| `--print-final-state` | Dump all non-zero registers and memory after execution |
| `--psw` | Print the Program State Word |
| `--resources` | Print cumulative resource usage counters |
| `--verbose` | Print configuration and execution summary |

Example:

```bash
cargo run --bin cqam-run -- examples/grover.cqam --print-final-state --resources
```

### cqam-as: Assemble or disassemble

```
# Assemble text source to binary .cqb
cargo run --bin cqam-as -- --assemble <file.cqam> [-o <output.cqb>] [--debug] [--strip]

# Disassemble binary .cqb back to text
cargo run --bin cqam-as -- --disassemble <file.cqb> [-o <output.cqam>]
```

The `.cqb` binary format uses a fixed 12-byte header (magic, version, entry
point, code length) followed by 32-bit instruction words and an optional debug
symbol section for label restoration during disassembly.

Example:

```bash
cargo run --bin cqam-as -- --assemble examples/arithmetic.cqam -o out.cqb --debug
cargo run --bin cqam-as -- --disassemble out.cqb
```

### cqam2qasm: Convert to OpenQASM 3.0

```
cargo run --bin cqam2qasm -- <file.cqam> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-o <file>` | Output file path (default: stdout) |
| `--fragment` | Emit body only — no header, no declarations, no gate stubs |
| `--expand` | Expand kernel invocations to gate-level QASM templates |
| `--no-expand` | Emit kernel stubs instead of expanding templates |
| `--doc` | Print the CQAM instruction reference |

Example:

```bash
cargo run --bin cqam2qasm -- examples/grover.cqam -o grover.qasm --expand
cargo run --bin cqam2qasm -- examples/quantum_observe.cqam --fragment
```

## Example Programs

The `examples/` directory contains 23 programs demonstrating a range of
classical, quantum, and hybrid computations:

| File | Description |
|------|-------------|
| `hybrid_fork.cqam` | Parallel execution with HFORK/HMERGE and conditional branching via HCEXEC |
| `grover_16q.cqam` | Grover search on a 16-qubit register |
| `bell_state.cqam` | Bell state preparation, measurement, and mode/mean extraction |
| `ghz_verify.cqam` | GHZ state preparation and fidelity verification |
| `qft_16q.cqam` | Quantum Fourier Transform on a 16-qubit register |
| `phase_estimation.cqam` | Quantum phase estimation algorithm |
| `amplitude_estimation.cqam` | Quantum amplitude estimation |
| `deutsch_jozsa.cqam` | Deutsch-Jozsa oracle evaluation |
| `bernstein_vazirani.cqam` | Bernstein-Vazirani secret-finding algorithm |
| `simon.cqam` | Simon's period-finding algorithm |
| `shor_period.cqam` | Shor's period-finding subroutine (rotation-kernel approximation) |
| `quantum_counting.cqam` | Quantum counting via phase estimation |
| `quantum_teleport.cqam` | Quantum teleportation protocol |
| `superdense_coding.cqam` | Superdense coding: 2 classical bits per qubit |
| `swap_test.cqam` | SWAP test for state overlap estimation |
| `quantum_walk.cqam` | Discrete quantum walk on a line |
| `qrng.cqam` | Quantum random number generator |
| `error_detection.cqam` | Quantum error detection with ancilla qubits |
| `vqe_loop.cqam` | Variational quantum eigensolver classical-quantum loop |
| `qaoa.cqam` | Quantum Approximate Optimization Algorithm |

Run any example:

```bash
cargo run --bin cqam-run -- examples/bell_state.cqam --print-final-state
cargo run --bin cqam-run -- examples/vqe_loop.cqam --resources --verbose
```

## Reference Documentation

| Document | Contents |
|----------|----------|
| [ISA Reference Card](reference/isa.md) | Complete instruction table, encoding formats, all named constants, binary file format |
| [Machine Specification](reference/spec.md) | Register files, memory banks, PSW, interrupt model, formal operational semantics |
| [Binary Encoding Reference](reference/opcodes.md) | 32-bit word layout, opcode table, bit-field assignments for all encoding formats |
| [QASM Generation Semantics](reference/qasm.md) | Codegen pipeline, emit modes, kernel template expansion |
| [Instruction Examples](reference/examples.md) | Text-format syntax for every instruction with annotated usage examples |

API documentation from inline Rust doc comments:

```bash
cargo doc --workspace --no-deps --open
```

## License

See LICENSE file for details.
