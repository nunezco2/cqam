# CQAM: Classical-Quantum Abstract Machine


Santiago Núñez-Corrales, PhD

*National Center for Supercomputing Applications*

*University of Illinois Urbana-Champaign*


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
   register-to-register operations. Transcendental functions (FSIN, FCOS,
   FATAN2, FSQRT) support angle computations for rotation-based algorithms.
2. **Quantum state preparation.** `QPREP` initialises a quantum register to a
   named state (uniform superposition, zero state, Bell pair, GHZ state).
   `QPREPN` selects the qubit count at runtime from an integer register.
   `QMIXED` assembles an explicit mixed state from weighted statevectors stored
   in classical memory.
3. **Quantum evolution.** `QKERNEL` applies a unitary transformation — selected
   by kernel ID and parameterised by classical registers. Kernels (Fourier,
   inverse Fourier, Grover iteration, diffusion, entanglement, rotation,
   phase-shift) encapsulate multi-gate circuits as atomic ISA-level operations.
   Individual qubit-level gates (QCNOT, QCZ, QSWAP, QROT, QHADM, QFLIP,
   QPHASE) provide fine-grained control. `QCUSTOM` applies a user-defined
   unitary read from classical memory. `QTENSOR` composes two registers via
   tensor product.
4. **Measurement.** `QOBSERVE` performs a projective measurement under the Born
   rule, collapsing the state and storing the outcome in the hybrid register
   file. `QMEAS` measures a single qubit and stores the 0/1 result in an
   integer register without consuming the quantum register.
5. **State inspection.** `QSAMPLE` non-destructively reads probabilities from a
   quantum register. `QPTRACE` computes the partial trace over subsystem B,
   producing a reduced density matrix. `QRESET` resets a single qubit to |0>.
6. **Classical post-processing.** `HREDUCE` applies one of 17 reduction
   functions (mean, mode, variance, magnitude, phase, expectation value, etc.)
   to extract a classical scalar from a measurement distribution, depositing it
   in an integer, float, or complex register.
7. **Hybrid control flow.** `HFORK` and `HMERGE` delimit parallel execution
   regions. `HCEXEC` provides conditional branching on PSW flags set by quantum
   operations.
8. **Interrupt-driven error handling.** A two-level interrupt model supports
   handler registration via `SETIV` and return via `RETI`, enabling the
   classical control layer to respond to arithmetic faults, quantum fidelity
   violations, and synchronisation failures.

All instructions encode into a fixed-width 32-bit word with an 8-bit opcode
prefix. The ISA comprises 80+ instructions across six functional groups.

## Quantum Simulation Model

CQAM provides two quantum simulation backends, selected automatically based on
the operation being performed and optionally overridden by the `--density-matrix`
flag.

**Statevector backend (default).** Pure quantum states are represented as a
length-2^n complex amplitude vector |psi>. Gate applications are O(2^n) per
operation. This is the default for all standard preparations and kernel
operations.

**Density matrix backend.** Mixed quantum states are represented as 2^n x 2^n
Hermitian, trace-1, positive semi-definite matrices rho. Gate applications
are O(4^n) per operation via the conjugation rho' = U rho U†. Required for
partial trace (QPTRACE), explicit mixing (QMIXED), and purity-based fidelity
monitoring.

The `QuantumRegister` enum dispatches between `Pure(Statevector)` and
`Mixed(DensityMatrix)` variants at runtime. Operations that require a density
matrix automatically promote a Pure register to Mixed. When `--density-matrix`
is passed on the command line (or `force_density_matrix = true` in the config),
all registers start as Mixed regardless of the operation.

Fidelity metrics available on density matrices:

- `purity()`: Tr(rho^2), equals 1.0 for pure states, < 1.0 for mixed states.
- `von_neumann_entropy()`: true S(rho) = -Tr(rho log rho) computed via Jacobi
  eigendecomposition of rho, normalized to [0, 1].
- `diagonal_entropy()`: Shannon entropy of the measurement probability
  distribution (fast approximation, does not require eigendecomposition).
- `entanglement_entropy()`: von Neumann entropy of the reduced density matrix
  after partial trace, quantifying bipartite entanglement.

Eight quantum kernels are provided as ISA-level operations:

| Kernel ID | Name | Description |
|-----------|------|-------------|
| 0 | init | Re-initialise to uniform superposition |
| 1 | entangle | CNOT-based entanglement circuit |
| 2 | fourier | Quantum Fourier Transform |
| 3 | diffuse | Grover diffusion (inversion about the mean) |
| 4 | grover_iter | Complete Grover iteration (oracle + diffusion) |
| 5 | rotate | Diagonal rotation: U\[k\]\[k\] = exp(i \* theta \* k) |
| 6 | phase_shift | Phase shift: U\[k\]\[k\] = exp(i \* \|z\| \* k), z from complex register |
| 7 | fourier_inv | Inverse Quantum Fourier Transform |

## Architecture

```
                  ┌─────────────────────────────────┐
                  │       Classical Subsystem        │
                  │                                  │
                  │  R0-R15  (16 x i64)   integers  │
                  │  F0-F15  (16 x f64)   floats    │
                  │  Z0-Z15  (16 x C64)   complex   │
                  │                                  │
                  │  CMEM    (64K x i64)   memory   │
                  │  PSW     condition/trap flags    │
                  │  CS      call stack              │
                  │  ISR     interrupt vector table  │
                  ├──────────────────────────────────┤
                  │     Classical-Quantum Bridge     │
                  │                                  │
                  │  H0-H7  (8 x HybridValue)       │
                  │    QOBSERVE ↓   ↑ HREDUCE       │
                  ├──────────────────────────────────┤
                  │       Quantum Subsystem          │
                  │                                  │
                  │  Q0-Q7  (8 x QuantumRegister)   │
                  │    Pure(Statevector) O(2^n)      │
                  │    Mixed(DensityMatrix) O(4^n)   │
                  │  QMEM   (256 x QuantumRegister) │
                  │                                  │
                  │  Kernels: Fourier, Fourier_inv,  │
                  │   Grover, Diffuse, Rotate,       │
                  │   PhaseShift, Entangle, Init     │
                  └─────────────────────────────────┘
```

This layered architecture enforces the no-cloning constraint: quantum state
cannot be copied into the classical subsystem without measurement.

## Crate Structure

The CQAM toolchain is implemented as a Rust workspace comprising seven crates:

| Crate | Role | Description |
|-------|------|-------------|
| `cqam-core` | ISA definition | Instruction enum, text parser, pragma support, 32-bit opcode encoding and decoding, error types, register and memory abstractions |
| `cqam-sim` | Quantum backend | Statevector and density matrix backends, `QuantumRegister` dispatch enum, complex arithmetic, the `Kernel` trait, eight concrete kernel implementations |
| `cqam-vm` | Execution engine | Instruction dispatch, program status word, fork/merge parallelism, ISR table, resource accounting, purity-based fidelity monitoring |
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
| `--qubits <n>` | Default qubits per quantum register (1-16, overrides config) |
| `--max-cycles <n>` | Maximum instruction cycles before forced halt |
| `--density-matrix` | Force density-matrix backend for all quantum registers |
| `--print-final-state` | Dump all non-zero registers and memory after execution |
| `--psw` | Print the Program State Word |
| `--resources` | Print cumulative resource usage counters |
| `--verbose` | Print configuration and execution summary |
| `--version` | Show version |
| `--help` | Show help message |

The simulator configuration file (TOML) supports:

```toml
fidelity_threshold = 0.95   # minimum purity before QuantumError interrupt
max_cycles         = 1000   # instruction cycle limit
enable_interrupts  = true   # enable maskable interrupt dispatch
default_qubits     = 2      # default qubits per QPREP
```

Programs may also embed a qubit hint via the `#! qubits N` pragma on the first
line, which sets the default qubit count for that program and overrides the
config file (but not the `--qubits` CLI flag).

Example:

```bash
cargo run --bin cqam-run -- examples/qrng.cqam --print-final-state --resources
cargo run --bin cqam-run -- examples/vqe_loop.cqam --density-matrix --verbose
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

| Option | Description |
|--------|-------------|
| `--debug` | Include debug symbol table (label names) in `.cqb` output |
| `--strip` | Remove label pseudo-instructions from the binary word stream |

Example:

```bash
cargo run --bin cqam-as -- --assemble examples/qrng.cqam -o out.cqb --debug
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
cargo run --bin cqam2qasm -- examples/qrng.cqam -o qrng.qasm --expand
cargo run --bin cqam2qasm -- examples/swap_test.cqam --fragment
```

## Example Programs

The `examples/` directory contains 18 programs demonstrating a range of
classical, quantum, and hybrid computations:

| File | Description |
|------|-------------|
| `grover_16q.cqam` | Grover search on a 16-qubit register |
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
cargo run --bin cqam-run -- examples/qrng.cqam --print-final-state
cargo run --bin cqam-run -- examples/vqe_loop.cqam --resources --verbose
```

## Reference Documentation

| Document | Contents |
|----------|----------|
| [ISA Reference Card](reference/isa.md) | Complete instruction table, encoding formats, all named constants, binary file format |
| [Machine Specification](reference/spec.md) | Register files, memory banks, PSW, interrupt model, quantum simulation model (dual backends, fidelity metrics), hybrid execution model, formal operational semantics |
| [Binary Encoding Reference](reference/opcodes.md) | 32-bit word layout, opcode table, bit-field assignments for all encoding formats |
| [QASM Generation Semantics](reference/qasm.md) | Codegen pipeline, emit modes, kernel template expansion |
| [Instruction Examples](reference/examples.md) | Text-format syntax for every instruction with annotated usage examples |

API documentation from inline Rust doc comments:

```bash
cargo doc --workspace --no-deps --open
```

## Acknowledgments

This project was partially funded by the IBM-Illinois Discovery Accelerator Institute.

## Related literature

Núñez-Corrales, S., Di Matteo, O., Dumbell, J., Edwards, M., Giusto, E., Pakin, S. and Stirbu, V., 2025, August. [Productive Quantum Programming Needs Better Abstract Machines](https://ieeexplore.ieee.org/abstract/document/11250286). *In 2025 IEEE International Conference on Quantum Computing and Engineering (QCE)* (Vol. 1, pp. 816-826). IEEE.

## License

Apache 2.0.
