# CQAM: Classical-Quantum Abstract Machine


Santiago Núñez-Corrales, PhD - <nunezco2@illinois.edu>

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
   by a four-letter kernel mnemonic (QFFT, ENTG, GROV, DIFF, DROT, PHSH, QIFT,
   CTLU, DIAG, PERM, UNIT) and parameterised by classical registers. Kernels
   (Fourier, inverse Fourier, Grover iteration, diffusion, entanglement,
   rotation, phase-shift, controlled-unitary, diagonal unitary, permutation)
   encapsulate multi-gate circuits as atomic ISA-level operations.
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
   regions. `JMPF FLAG, label` provides conditional branching on named PSW flags
   (ZF, NF, PF, QF, SF, EF, HF, IF) set by quantum and classical operations.
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
partial trace (QPTRACE), explicit mixing (QMIXED), noise channel application,
and purity-based fidelity monitoring.

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

Eleven quantum kernels are provided as ISA-level operations:

| Kernel ID | Mnemonic | Description |
|-----------|----------|-------------|
| 0 | UNIT | Re-initialise to uniform superposition |
| 1 | ENTG | CNOT-based entanglement circuit |
| 2 | QFFT | Quantum Fourier Transform |
| 3 | DIFF | Grover diffusion (inversion about the mean) |
| 4 | GROV | Complete Grover iteration (oracle + diffusion) |
| 5 | DROT | Diagonal rotation: U[k][k] = exp(i * theta * k) |
| 6 | PHSH | Phase shift: U[k][k] = exp(i * |z| * k), z from complex register |
| 7 | QIFT | Inverse Quantum Fourier Transform |
| 8 | CTLU | Controlled-U: applies any sub-kernel conditioned on a control qubit |
| 9 | DIAG | Diagonal unitary: applies arbitrary diagonal phases from CMEM |
| 10 | PERM | Permutation: reorders basis states according to a permutation table in CMEM |

## Data Parallelism

The `cqam-sim` and `cqam-vm` crates use [Rayon](https://github.com/rayon-rs/rayon) for
data-parallel execution of computationally intensive simulation operations. Parallelism is
applied selectively: a threshold of `PAR_THRESHOLD = 256` (corresponding to 8 or more qubits)
gates entry to the thread pool, and all operations fall back to sequential iteration below
that threshold to avoid scheduling overhead on small registers.

Operations parallelized in `cqam-sim`:

- **Density matrix:** `apply_unitary`, `apply_two_qubit_gate`, `partial_trace_b`, `purity`,
  `tensor_product`, `jacobi_eigenvalues`, `diagonal_probabilities`, `von_neumann_entropy`
- **Statevector:** `apply_unitary`, `apply_single_qubit_gate`, `apply_two_qubit_gate`,
  `measure_qubit`, `tensor_product`, `diagonal_probabilities`
- **Kernels:** the `apply_sv` methods of the `grover_iter`, `diffuse`, `rotate`, `phase_shift`,
  `fourier`, `diagonal`, and `permutation` kernels

Operations parallelized in `cqam-vm`:

- **HREDUCE reductions:** MEAN, VARIANCE, MODE, ARGMAX, and EXPECT

Rayon's work-stealing thread pool is cross-platform and runs without modification on Linux,
macOS, and Windows.

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
                  │   PhaseShift, Entangle, Init,    │
                  │   ControlledU, DiagonalUnitary,  │
                  │   Permutation                    │
                  └─────────────────────────────────┘
```

This layered architecture enforces the no-cloning constraint: quantum state
cannot be copied into the classical subsystem without measurement.

## Crate Structure

The CQAM toolchain is implemented as a Rust workspace comprising eight crates:

| Crate | Role | Description |
|-------|------|-------------|
| `cqam-core` | ISA definition | Instruction enum, text parser, pragma support, 32-bit opcode encoding and decoding, error types, register and memory abstractions, `QuantumBackend` trait |
| `cqam-sim` | Quantum backend | Statevector and density matrix backends, `QuantumRegister` dispatch enum, complex arithmetic, the `Kernel` trait, eleven concrete kernel implementations, noise model framework |
| `cqam-vm` | Execution engine | Instruction dispatch, program status word, fork/merge parallelism, ISR table, resource accounting, purity-based fidelity monitoring |
| `cqam-run` | CLI runner | Program loader, execution driver, shot-mode sampling, state and resource reporting |
| `cqam-as` | Assembler | Two-pass assembler (label resolution + encoding), binary `.cqb` format reader/writer, disassembler |
| `cqam-codegen` | Code generation | Three-stage OpenQASM 3.0 emission pipeline (scan, declare, emit) with kernel template expansion |
| `cqam2qasm` | CLI translator | Command-line interface for CQAM-to-OpenQASM 3.0 translation |
| `cqam-dbg` | Debugger | Interactive step-through debugger (in development) |

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
cargo run --bin cqam-run -- <file.cqam|file.cqb> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--config <path>` | Path to TOML simulator configuration file |
| `--qubits <n>` | Default qubits per quantum register (1-16, overrides config) |
| `--max-cycles <n>` | Maximum instruction cycles before forced halt |
| `--density-matrix` | Force density-matrix backend for all quantum registers |
| `--threads <n>` | Default thread count for HFORK parallelism (1-256) |
| `--shots <n>` | Number of shots for QPU-realistic sampling (see Shot Mode) |
| `--noise <model\|path>` | Built-in noise modality name or path to a custom `.toml` file (see Noise Models) |
| `--noise-method <m>` | Noise simulation method: `density-matrix` or `trajectory` (auto-selected if omitted) |
| `--print-final-state` | Dump all non-zero registers and memory after execution |
| `--psw` | Print the Program State Word |
| `--resources` | Print cumulative resource usage counters |
| `--verbose` | Print configuration and execution summary |
| `--version` | Show version |
| `--help` | Show help message |

The simulator configuration file (TOML) supports all of the above settings plus
additional tuning parameters:

```toml
fidelity_threshold  = 0.95   # minimum purity before QuantumError interrupt
max_cycles          = 1000   # instruction cycle limit
enable_interrupts   = true   # enable maskable interrupt dispatch
default_qubits      = 2      # default qubits per QPREP
force_density_matrix = false # force density-matrix backend globally
default_threads     = 1      # default thread count for HFORK
rng_seed            = 42     # fix RNG for reproducible measurements
shots               = 1000   # shot count (equivalent to --shots 1000)
noise_model         = "superconducting"  # built-in noise modality
noise_method        = "density-matrix"  # noise simulation method
```

Programs may also embed a qubit hint via the `#! qubits N` pragma on the first
line, which sets the default qubit count for that program and overrides the
config file (but not the `--qubits` CLI flag). The thread count obeys the same
precedence: CLI flag > `#! threads N` pragma > config file default.

Quick start examples:

```bash
cargo run --bin cqam-run -- examples/basic/qrng.cqam --print-final-state --resources
cargo run --bin cqam-run -- examples/intermediate/vqe_loop.cqam --density-matrix --verbose
cargo run --bin cqam-run -- examples/basic/ghz_verify.cqam --shots 2000 --noise superconducting
```

### Shot Mode

By default, CQAM computes exact probability distributions from the quantum
state. The `--shots N` flag enables QPU-realistic sampling: rather than
returning the full Born-rule distribution, each `QOBSERVE` instruction is
resampled N independent times to produce an empirical histogram stored as a
`HybridValue::Hist` in the H register file.

Shot mode mirrors the behavior of a real QPU, where a circuit is executed
repeatedly to accumulate outcome statistics. This mode is particularly
meaningful when combined with a noise model, since each shot experiences
independent noise realizations under the trajectory method.

```bash
# 1000-shot run with superconducting noise on the trajectory method
cargo run --bin cqam-run -- examples/basic/error_detection.cqam \
    --shots 1000 --noise superconducting --noise-method trajectory

# 500-shot run using the density-matrix noise method (deterministic per shot)
cargo run --bin cqam-run -- examples/basic/qrng.cqam \
    --shots 500 --noise trapped-ion --noise-method density-matrix
```

When `--shots` is active and `--noise-method` is not specified, the runner
auto-selects the noise method: for registers with more than 10 qubits,
`trajectory` is preferred to keep memory tractable; for smaller registers,
`density-matrix` is used for deterministic accuracy.

### Noise Models

CQAM ships five built-in noise modalities covering the major QPU technology
families. Each modality applies a physically motivated combination of Kraus
channels — amplitude damping, phase damping, depolarizing, thermal relaxation,
and readout confusion — with parameters derived from published device
characteristics.

| Modality name | Technology | Reference device class |
|---------------|------------|------------------------|
| `superconducting` | Transmon qubits | IBM Eagle/Heron class |
| `trapped-ion` | Optical qubits in ion chains | Quantinuum H2 class |
| `neutral-atom` | Rydberg tweezer arrays | QuEra/Harvard-MIT class |
| `photonic` | Dual-rail photonic qubits | PsiQuantum/Xanadu class |
| `spin` | Semiconductor spin qubits | Silicon quantum dots |

Activate a built-in modality by name:

```bash
cargo run --bin cqam-run -- program.cqam --noise superconducting
cargo run --bin cqam-run -- program.cqam --noise trapped-ion --shots 1000
```

#### Noise simulation methods

Two noise simulation methods are available:

- **`density-matrix`**: Applies Kraus operators directly to the density matrix
  rho via the channel E(rho) = sum_k K_k rho K_k†. Deterministic and exact
  for a given noise model, but scales as O(4^n).
- **`trajectory`**: Applies stochastic quantum jumps to a statevector. Each
  shot samples an independent trajectory through the channel. Scales as O(2^n)
  per shot and is preferred for high qubit counts with large shot counts.

#### Custom noise profiles via TOML

Any built-in noise model can be customized by writing a TOML file. The
`modality` field selects which noise model struct to instantiate; all remaining
fields override the default parameters for that modality (unspecified fields
retain their defaults).

Example custom superconducting profile:

```toml
# my_device.toml
modality = "superconducting"

t1 = 500e-6              # T1 relaxation time (seconds)
t2 = 350e-6              # T2 dephasing time (seconds)
single_gate_error = 1e-4 # single-qubit gate error (depolarizing)
single_gate_time  = 20e-9
two_gate_error    = 2e-3 # two-qubit gate error
two_gate_time     = 60e-9
readout_error     = [0.005, 0.015]  # [P(1|0), P(0|1)]
thermal_population = 0.005
```

```bash
cargo run --bin cqam-run -- program.cqam --noise my_device.toml --shots 1000
```

Five annotated example profiles are provided in `examples/noise_profiles/`:

| File | Modality | Device scenario |
|------|----------|----------------|
| `superconducting_transmon.toml` | `superconducting` | Next-generation tantalum-capacitor transmon with tunable-coupler CZ gates |
| `trapped_ion_optical.toml` | `trapped-ion` | Cryogenic Yb-171+ chain with optical qubits and Molmer-Sorensen gates |
| `neutral_atom_rydberg.toml` | `neutral-atom` | Cs-133 Rydberg tweezer array with zoned architecture |
| `photonic_fusion.toml` | `photonic` | Fusion-based photonic architecture with high-efficiency SNSPDs |
| `spin_silicon.toml` | `spin` | Silicon quantum dot spin qubits with isotopic purification |

#### Noise channels

The channel library in `cqam-sim/src/noise/channels.rs` provides the
following Kraus operator constructors, all verified to satisfy the
completeness relation sum_k K_k† K_k = I:

| Channel | Function | Use |
|---------|----------|-----|
| Amplitude damping | `amplitude_damping(gamma)` | T1 energy decay: gamma = 1 - exp(-t/T1) |
| Phase damping | `phase_damping(lambda)` | Pure dephasing: lambda = 1 - exp(-t/T_phi) |
| Depolarizing (1q) | `depolarizing_single(p)` | Symmetric single-qubit error |
| Depolarizing (2q) | `depolarizing_two_qubit(p)` | 15-term two-qubit Pauli channel |
| Thermal relaxation | `thermal_relaxation(t1, t2, time, p_exc)` | Generalized amplitude damping + dephasing at finite temperature |
| Photon loss | `photon_loss(eta)` | Amplitude damping with gamma = 1 - eta |
| Bit flip | `bit_flip(p)` | X error with probability p |
| Readout confusion | `apply_readout_confusion(probs, p01, p10)` | Asymmetric measurement errors |

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
cargo run --bin cqam-as -- --assemble examples/basic/qrng.cqam -o out.cqb --debug
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
cargo run --bin cqam2qasm -- examples/basic/qrng.cqam -o qrng.qasm --expand
cargo run --bin cqam2qasm -- examples/basic/swap_test.cqam --fragment
```

## Assembly Language

### Data Section

CQAM programs may include a `.data` section before `.code` for declaring
initialized classical memory (CMEM) contents. The assembler processes data
directives and populates CMEM before execution begins.

```
.data
    .org 200
my_label:
    .c64 1.0J0.0, -1.0J0.0,
         0.5J0.5,  0.0J1.0

    .org 1000
msg:
    .ascii "Hello, CQAM!\n"

.code
    ILDI R0, @my_label       # R0 = 200 (CMEM base address)
    ILDI R1, @my_label.len   # R1 = 4 (number of complex entries)
    ...
```

#### Directives

| Directive | Description |
|-----------|-------------|
| `.org N` | Advance the allocation pointer to CMEM address N |
| `.ascii "str"` | Store one ASCII byte per CMEM cell, NUL-terminated |
| `.asciiz "str"` | Alias for `.ascii` |
| `.i64 v1, v2, ...` | Store literal i64 values |
| `.f64 v1, v2, ...` | Store f64 values as bit-cast i64 |
| `.c64 z1, z2, ...` | Store complex values in `aJb` format (2 CMEM cells per entry) |

#### Complex Literals (`.c64`)

The `.c64` directive stores complex numbers in the format `realJimag` (or
`realJImag` — case-insensitive separator). Each entry occupies two consecutive
CMEM cells: `f64::to_bits(re) as i64` at the base address and
`f64::to_bits(im) as i64` at base+1.

Supported number formats include integers, decimals, and scientific notation:

```
.c64 1.0J0.0              # 1 + 0i
.c64 -1.5J2.5             # -1.5 + 2.5i
.c64 1.5e-3J-2.0e1        # 0.0015 - 20.0i
.c64 0J1.0                # pure imaginary
.c64 3.14J0               # pure real
```

**Line continuation:** A trailing comma continues the `.c64` directive on the
next line. This avoids long lines when declaring large arrays:

```
.c64 1.0J0.0,  1.0J0.0,  1.0J0.0,  1.0J0.0,
     1.0J0.0, -1.0J0.0,  1.0J0.0,  1.0J0.0
```

#### Label References

Labels defined in `.data` can be referenced in `.code` with the `@` prefix:

| Syntax | Resolves to |
|--------|-------------|
| `@label` | CMEM base address of the label |
| `@label.len` | Logical entry count (for `.c64`, the number of complex entries, not CMEM cells) |

### Pragmas

Pragmas appear on the first line of a `.cqam` file and set program-level
metadata consumed by the runner before execution:

| Pragma | Effect |
|--------|--------|
| `#! qubits N` | Default qubit count per `QPREP` for this program |
| `#! threads N` | Default thread count for `HFORK` in this program |

CLI flags override pragmas; pragmas override the config file default.

## Example Programs

The `examples/` directory contains programs organized into four subdirectories:

### `examples/basic/` (15 programs)

Foundational algorithms and ISA feature demonstrations:

| Program | Description |
|---------|-------------|
| `qrng.cqam` | Quantum random number generator |
| `ghz_verify.cqam` | GHZ state preparation and verification |
| `quantum_teleport.cqam` | Quantum teleportation protocol |
| `superdense_coding.cqam` | Superdense coding |
| `swap_test.cqam` | SWAP test for state comparison |
| `bernstein_vazirani.cqam` | Bernstein-Vazirani algorithm |
| `deutsch_jozsa.cqam` | Deutsch-Jozsa algorithm |
| `error_detection.cqam` | Quantum error detection circuit |
| `qft_16q.cqam` | 16-qubit Quantum Fourier Transform |
| `reversible_adder.cqam` | Reversible classical adder |
| `ecall_hello.cqam` | Hello-world ECALL demonstration |
| `test_diagonal.cqam` | Diagonal unitary kernel test |
| `test_permutation.cqam` | Permutation kernel test |
| `test_controlled_sub.cqam` | Controlled-U subkernel test |
| `test_c64_directive.cqam` | Complex data section test |

### `examples/intermediate/` (28 programs)

Complete quantum algorithms with classical control loops:

Grover variants, Shor's algorithm (period finding and modular multiplication),
quantum phase estimation (general, iterative, eigenvalue, and amplitude
estimation), VQE and ADAPT-VQE variational loops, QAOA for MaxCut, HHL linear
system solver, quantum walks (coined and permutation-based), Simon's algorithm,
Durr-Hoyer minimum finding, quantum state tomography, quantum feature map,
quantum singular value transformation (QSVT), Trotter Hamiltonian simulation,
and diagonal Hamiltonian simulation.

### `examples/advanced_nothreads/` (19 programs)

Algorithms emphasizing ISR-driven control, multi-kernel pipelines, expectation
value computation, and adaptive circuits, all using a single thread.

### `examples/threaded/` (3 programs)

Programs using HFORK/HMERGE for multi-threaded quantum state preparation and
classical accumulation:

| Program | Description |
|---------|-------------|
| `parallel_quantum_prep.cqam` | Parallel preparation of independent quantum registers |
| `parallel_accumulate.cqam` | Parallel classical accumulation with shared memory |
| `thread_identity.cqam` | Thread ID and count introspection via ITID/ICCFG |

Run any example:

```bash
cargo run --bin cqam-run -- examples/basic/qrng.cqam --print-final-state
cargo run --bin cqam-run -- examples/intermediate/vqe_loop.cqam --resources --verbose
cargo run --bin cqam-run -- examples/advanced_nothreads/adaptive_grover.cqam --density-matrix
cargo run --bin cqam-run -- examples/threaded/parallel_quantum_prep.cqam --threads 4
```

## Register Architecture

| File | Size | Element type | Description |
|------|------|--------------|-------------|
| R0-R15 | 16 | i64 | Integer general-purpose registers |
| F0-F15 | 16 | f64 | Floating-point general-purpose registers |
| Z0-Z15 | 16 | C64 | Complex general-purpose registers |
| H0-H7 | 8 | HybridValue | Hybrid bridge: holds Scalar, Prob, Dist, or Hist |
| Q0-Q7 | 8 | QRegHandle | Quantum register handles (live quantum states) |
| CMEM | 64K | i64 | Classical random-access memory |
| QMEM | 256 | QRegHandle | Quantum memory (stored quantum states) |
| PSW | 1 | flags | Program Status Word: condition codes, traps, quantum flags |
| CS | - | usize stack | Call stack for subroutine linkage |
| ISR | 16 | usize table | Interrupt service routine vector table |

`HybridValue` variants:
- `Scalar(i64)` — integer scalar from HREDUCE
- `Prob(f64)` — single probability
- `Dist(Vec<(u32, f64)>)` — full Born-rule distribution over basis states
- `Hist(ShotHistogram)` — shot-sampled outcome histogram (from `--shots N`)

## Quantum Backend Interface

The `QuantumBackend` trait in `cqam-core/src/quantum_backend.rs` is the
sole interface between the VM and any quantum execution engine. It uses
an opaque handle model: the VM stores `QRegHandle(u64)` values in Q0-Q7
and QMEM slots, and the backend maps handles to its internal state
representation.

The `SimulationBackend` in `cqam-sim/src/backend.rs` is the reference
implementation, backed by `QuantumRegister` (Pure/Mixed dispatch). Future
QPU backends — targeting IBM Quantum, QuEra, IonQ, Rigetti, or Amazon
Braket — will implement the same trait without requiring changes to the VM.

The trait groups 21 methods into five categories:

| Category | Methods |
|----------|---------|
| State preparation | `prep`, `prep_from_amplitudes`, `prep_mixed` |
| Gate / kernel application | `apply_kernel`, `apply_single_gate`, `apply_two_qubit_gate`, `apply_custom_unitary` |
| Observation / measurement | `observe`, `sample`, `measure_qubit` |
| Composite operations | `tensor_product`, `partial_trace`, `reset_qubit` |
| Handle lifecycle | `clone_state`, `release`, `num_qubits`, `dimension`, `max_qubits`, `set_rng_seed`, `purity`, `is_pure`, `diagonal_probabilities`, `get_element`, `amplitude` |

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

## Disclaimers

This repository was developed with AI coding assistance using Claude Code v2.1.68 under Opus 4.6. The architecture design was entirely devised by the author, and an agentic pipeline was created to translate the architecture into code design, implementation, critique and debugging tasks divided into multiple phases. Agents were driven by conservative and precise prompts in which high friction to change the implementation was the default behavior when facing test errors. Corrections to the code structure, details and extensive review of the source code was performed by the author. Any errors that remain are the author's.

## Acknowledgments

This project was partially funded by the IBM-Illinois Discovery Accelerator Institute.

## Related literature

Núñez-Corrales, S., Di Matteo, O., Dumbell, J., Edwards, M., Giusto, E., Pakin, S. and Stirbu, V., 2025, August. [Productive Quantum Programming Needs Better Abstract Machines](https://ieeexplore.ieee.org/abstract/document/11250286). *In 2025 IEEE International Conference on Quantum Computing and Engineering (QCE)* (Vol. 1, pp. 816-826). IEEE.

## License

Apache 2.0.
