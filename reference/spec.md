# CQAM Machine Specification

## 1. Overview

The Classical-Quantum Abstract Machine (CQAM) is a register-based virtual machine
with a 32-bit fixed-width instruction word architecture. It combines classical
integer, floating-point, and complex arithmetic with an ensemble/probability
quantum model and a hybrid classical-quantum execution layer.

Design philosophy:
- Quantum state is represented as probability distributions (`QDist<u16>`), not
  complex amplitude vectors. This avoids exponential memory scaling.
- All instructions encode into a single 32-bit word with an 8-bit opcode prefix.
- The machine supports five register files, two memory banks, a hardware call
  stack, and a two-level interrupt model.

## 2. Machine State

### 2.1 Program Counter (PC)

- Type: `usize` (index into instruction memory)
- The executor is the sole authority on PC advancement.
- Non-jump instructions advance PC by 1 after execution.
- Jump instructions (JMP, JIF, CALL, HCEXEC) set PC directly.

### 2.2 Register Files

| File | Notation | Count | Element Type | Description |
|------|----------|-------|--------------|-------------|
| IntRegFile | R0-R15 | 16 | i64 | Integer arithmetic, comparisons, predicates |
| FloatRegFile | F0-F15 | 16 | f64 | Floating-point arithmetic |
| ComplexRegFile | Z0-Z15 | 16 | (f64, f64) | Complex number arithmetic |
| Quantum registers | Q0-Q7 | 8 | Option\<QDist\<u16\>\> | Quantum probability distributions |
| HybridRegFile | H0-H7 | 8 | HybridValue | Measurement results (Dist, Int, Float, Complex, Empty) |

### 2.3 Memory Banks

| Bank | Address Type | Size | Element Type | Description |
|------|-------------|------|--------------|-------------|
| CMEM | u16 | 65536 cells | i64 | Classical memory, heap-allocated |
| QMEM | u8 | 256 slots | Option\<QDist\<u16\>\> | Quantum memory (8-qubit distributions) |

CMEM is accessed by ILdm, IStr, FLdm, FStr, ZLdm, ZStr.
QMEM is accessed by QLoad, QStore.

### 2.4 Call Stack

- Type: `Vec<usize>` (hardware stack for CALL/RET)
- CALL pushes PC+1 onto the stack and jumps to the target label.
- RET pops the top address and jumps to it. If the stack is empty, RET acts as HALT.
- ISR handlers also push the current PC for return via RET.

## 3. Program Status Word (PSW)

### 3.1 Classical Condition Flags

| Flag | Description |
|------|-------------|
| ZF | Zero flag: set when result == 0 |
| NF | Negative flag: set when result < 0 |
| OF | Overflow flag (reserved, always false) |
| PF | Predicate flag: set from comparison results |

### 3.2 Quantum State Flags

| Flag | Description |
|------|-------------|
| QF | Quantum active: set after any QPREP/QKERNEL execution |
| SF | Superposition present |
| EF | Entanglement present |
| DF | Decohered: set after measurement |
| CF | Collapsed distribution |

### 3.3 Hybrid Flags

| Flag | Description |
|------|-------------|
| HF | Hybrid mode active |
| forked | HFORK has been executed |
| merged | HMERGE has been executed |

### 3.4 Trap/Interrupt Flags

| Flag | Description |
|------|-------------|
| trap_halt | HALT requested (terminates execution loop) |
| trap_arith | Arithmetic fault (division by zero, etc.) |
| int_quantum_err | Quantum fidelity violation |
| int_sync_fail | Hybrid synchronization failure |

Priority order for pending traps: trap_halt > trap_arith > int_quantum_err > int_sync_fail.

## 4. Interrupt Model

### 4.1 Two-Level Hierarchy

- **NMI (Non-Maskable):** Halt, IllegalPC -- always fire regardless of the
  interrupt enable flag.
- **Maskable:** Arithmetic, QuantumError, SyncFailure -- gated by the
  `enable_interrupts` configuration flag. When interrupts are disabled, maskable
  traps are silently ignored.

### 4.2 ISR Vector Table

The ISR table maps trap types to handler addresses (instruction indices).
When a trap fires and has a registered handler:
1. The current PC is pushed onto the call stack.
2. Execution jumps to the handler address.
3. The handler returns via RET to resume normal execution.

### 4.3 Default Behavior (no handler registered)

| Trap | Default Action |
|------|---------------|
| Halt | Sets trap_halt |
| IllegalPC | Logs error, sets trap_halt |
| Arithmetic | Logs error, sets trap_arith + trap_halt |
| QuantumError | Logs warning, sets int_quantum_err + trap_halt |
| SyncFailure | Logs warning, sets int_sync_fail |

## 5. Execution Model

### 5.1 Instruction Fetch-Execute Cycle

The runner loop iterates while `PC < program.len()` and `!psw.trap_halt`.
Each iteration:
1. Fetch the instruction at the current PC.
2. Dispatch to the executor, which performs the operation and returns `Result`.
3. Apply the resource cost delta for the instruction.
4. Advance the PC (unless a jump instruction already set it).

### 5.2 Label Resolution

Labels are resolved once during `ExecutionContext` construction into a
`HashMap<String, usize>`. All subsequent label lookups are O(1).

## 6. Quantum Model

### 6.1 QDist\<u16\> -- Probability Distribution

- Domain: `Vec<u16>` (basis states)
- Probabilities: `Vec<f64>` (real-valued, normalized to sum to 1.0)
- No complex amplitudes; ensemble/probability semantics.

### 6.2 Measurement

- Stochastic: `measure()` samples probabilistically.
- Deterministic: `measure_deterministic()` returns the mode (for testing).

### 6.3 Fidelity Metrics

- `superposition_metric()`: Shannon entropy normalized to [0,1].
- `entanglement_metric()`: multi-state correlation measure.
- Thresholds are configurable; violations trigger QuantumError interrupt.

### 6.4 Kernels (5 implemented)

| ID | Name | Description |
|----|------|-------------|
| 0 | init | Re-initialize distribution to uniform superposition |
| 1 | entangle | Create inter-qubit correlations |
| 2 | fourier | DFT-like phase transformation |
| 3 | diffuse | Grover diffusion (inversion about the mean) |
| 4 | grover_iter | Complete Grover iteration (oracle + diffusion) |

## 7. Hybrid Execution Model

### 7.1 HFORK / HMERGE

HFORK marks the beginning of a parallel execution region by setting the hybrid
mode and fork flags in the PSW. HMERGE ends the region by setting the merge
flag. Current implementation is flag-based (not thread-based).

### 7.2 HCEXEC

Conditional execution based on PSW flags. Reads the specified flag ID from the
PSW and jumps to the target label if the flag is set.

### 7.3 HREDUCE

14 reduction functions organized into three categories:

| IDs | Category | Output | Functions |
|-----|----------|--------|-----------|
| 0-5 | Float-to-Int | R[dst] (i64) | round, floor, ceil, trunc, abs, negate |
| 6-9 | Complex-to-Float | F[dst] (f64) | magnitude, phase, real, imag |
| 10-13 | Distribution | F[dst] or R[dst] | mean, mode, argmax, variance |

## 8. Resource Tracking

Each instruction has an associated `ResourceDelta` with five fields:

| Field | Description |
|-------|-------------|
| time | Execution cycles consumed |
| space | Register/memory slots written |
| superposition | Superposition created or consumed |
| entanglement | Entanglement created or consumed |
| interference | Interference effects (from measurement) |

The `ResourceTracker` accumulates deltas across execution for reporting.
