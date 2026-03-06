# CQAM Machine Specification

## 1. Overview

The Classical-Quantum Abstract Machine (CQAM) is a register-based virtual machine
with a 32-bit fixed-width instruction word architecture. It combines classical
integer, floating-point, and complex arithmetic with an ensemble/probability
quantum model and a hybrid classical-quantum execution layer.

Design philosophy:
- Quantum state is internally represented as density matrices (`DensityMatrix`),
  enabling correct simulation of superposition, interference, and entanglement.
  Measurement results are represented as probability distributions (`QDist<u16>`).
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
| Quantum registers | Q0-Q7 | 8 | Option\<DensityMatrix\> | Quantum state as density matrices |
| HybridRegFile | H0-H7 | 8 | HybridValue | Measurement results (Dist, Int, Float, Complex, Empty) |

### 2.3 Memory Banks

| Bank | Address Type | Size | Element Type | Description |
|------|-------------|------|--------------|-------------|
| CMEM | u16 | 65536 cells | i64 | Classical memory, heap-allocated |
| QMEM | u8 | 256 slots | Option\<DensityMatrix\> | Quantum memory (density matrices) |

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

### 6.1 DensityMatrix -- Quantum State Representation

Quantum state is represented by density matrices (`DensityMatrix`): 2^n x 2^n
Hermitian, positive semi-definite matrices with Tr(rho) = 1. Stored as flat
row-major `Vec<C64>` where `C64 = (f64, f64)`.

Invariants maintained by all operations:
- `data.len() == dim * dim` where `dim = 2^num_qubits`
- `Tr(rho) = 1.0` (within floating-point tolerance)
- `rho` is Hermitian: `rho[i][j] = conj(rho[j][i])`
- `rho` is positive semi-definite

The `QDist<u16>` type remains for measurement-outcome distributions used in
hybrid reduction operations.

### 6.2 Measurement

- Stochastic: `DensityMatrix::measure_all()` samples via the Born rule.
- Deterministic: `measure_deterministic()` returns the argmax (for testing).
- Measurement produces a collapsed state `|k><k|` and a `HybridValue::Dist`
  containing `[(k, 1.0)]`.

### 6.3 Fidelity Metrics

- `von_neumann_entropy()`: diagonal entropy normalized to [0,1].
- `purity()`: Tr(rho^2), equals 1.0 for pure states.
- `concentration_metric()`: inverse Herfindahl index (on QDist), measures
  distribution concentration.
- `entanglement_entropy()`: von Neumann entropy of the reduced density matrix
  after partial trace, measures bipartite entanglement.
- Thresholds are configurable; violations trigger QuantumError interrupt.

### 6.4 Kernels (7 implemented)

| ID | Name | Description |
|----|------|-------------|
| 0 | init | Re-initialize distribution to uniform superposition |
| 1 | entangle | Create inter-qubit correlations |
| 2 | fourier | DFT-like phase transformation |
| 3 | diffuse | Grover diffusion (inversion about the mean) |
| 4 | grover_iter | Complete Grover iteration (oracle + diffusion) |
| 5 | rotate | Diagonal rotation: U[k][k] = exp(i * theta * k); theta from F-file |
| 6 | phase_shift | Phase shift: U[k][k] = exp(i * |z| * k); amplitude from Z-file |

### 6.5 Observation Modes

QOBSERVE and QSAMPLE support three extraction modes, selected by the `mode`
field:

| Mode | ID | Output Type | Semantics |
|------|----|-------------|-----------|
| DIST | 0 | Dist(Vec<(u16, f64)>) | Full diagonal probability distribution (default mode; returns all basis-state probabilities). |
| PROB | 1 | Float(f64) | Probability of a single basis state at index R[ctx0]. Returns rho[k][k]. |
| AMP  | 2 | Complex(f64, f64) | Density matrix element rho[row][col] where row=R[ctx0], col=R[ctx1]. |

QOBSERVE is destructive: it consumes Q[src] (sets to None) and marks PSW.DF.
QSAMPLE is non-destructive: Q[src] remains available for further operations.

### 6.6 Masked Gate Operations

Three masked gate instructions apply single-qubit Pauli gates to selected
qubits of a quantum register. The selection mask is read from an integer
register R[mask_reg]. For each bit i (0-indexed from LSB) that is set in the
mask, the corresponding gate is applied to qubit i of the density matrix.
Bits beyond num_qubits are silently ignored.

| Instruction | Gate | Matrix |
|-------------|------|--------|
| QHADM | Hadamard | (1/sqrt(2)) [[1,1],[1,-1]] |
| QFLIP | Pauli-X | [[0,1],[1,0]] |
| QPHASE | Pauli-Z | [[1,0],[0,-1]] |

Gate matrices are private to `cqam-vm/src/qop.rs`. There is no public gate
module; the ISA instruction mnemonic determines the gate type.

## 7. Hybrid Execution Model

### 7.1 HFORK / HMERGE

HFORK marks the beginning of a parallel execution region by setting the hybrid
mode and fork flags in the PSW. HMERGE ends the region by setting the merge
flag. Current implementation is flag-based (not thread-based).

### 7.2 HCEXEC

Conditional execution based on PSW flags. Reads the specified flag ID from the
PSW and jumps to the target label if the flag is set.

### 7.3 HREDUCE

16 reduction functions organized into four categories:

| IDs | Category | Output | Functions |
|-----|----------|--------|-----------|
| 0-5 | Float-to-Int | R[dst] (i64) | round, floor, ceil, trunc, abs, negate |
| 6-9 | Complex-to-Float | F[dst] (f64) | magnitude, phase, real, imag |
| 10-13 | Distribution | F[dst] or R[dst] | mean, mode, argmax, variance |
| 14-15 | Complex-to-Complex | Z[dst] (f64, f64) | conj_z, negate_z |

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

## 9. Formal Operational Semantics

### 9.1 Machine State

The machine state is a tuple:

```
Sigma = (PC, R, F, Z, Q, H, CMEM, QMEM, PSW, CS)
```

where:
- `PC : N` -- program counter (instruction index)
- `R : [0..15] -> Z` -- integer register file (64-bit signed integers)
- `F : [0..15] -> R` -- floating-point register file (64-bit IEEE 754)
- `Z : [0..15] -> C` -- complex register file (pairs of f64)
- `Q : [0..7] -> DensityMatrix | NULL` -- quantum register file
- `H : [0..7] -> HybridValue | EMPTY` -- hybrid register file
- `CMEM : [0..65535] -> Z` -- classical memory (64-bit cells)
- `QMEM : [0..255] -> DensityMatrix | NULL` -- quantum memory
- `PSW : PSW_State` -- program status word (all flags)
- `CS : List<N>` -- call stack (return addresses)

### 9.2 State Transition Function

The single-step transition function is:

```
step : Sigma x Program -> Sigma
step(sigma, P) = dispatch(sigma, P[sigma.PC])
```

### 9.3 Transition Rules

Notation: `sigma' = sigma[field := value]` denotes a state identical to
`sigma` except that `field` is updated to `value`.

**Arithmetic (IADD):**
```
                  v = R[lhs] + R[rhs]
  -------------------------------------------------------
  sigma --IADD(dst, lhs, rhs)--> sigma[R[dst] := v,
                                       PSW := update_arith(PSW, v),
                                       PC := PC + 1]
```

**Unconditional Jump (JMP):**
```
                  addr = labels(target)
  -------------------------------------------------------
  sigma --JMP(target)--> sigma[PC := addr]
```

**Conditional Jump (JIF):**
```
          R[pred] != 0       addr = labels(target)
  -------------------------------------------------------
  sigma --JIF(pred, target)--> sigma[PC := addr]

          R[pred] == 0
  -------------------------------------------------------
  sigma --JIF(pred, target)--> sigma[PC := PC + 1]
```

**Subroutine Call (CALL):**
```
                  addr = labels(target)
  -------------------------------------------------------
  sigma --CALL(target)--> sigma[CS := (PC + 1) :: CS,
                                PC := addr]
```

**Quantum Preparation (QPREP):**
```
                  dm = init_density_matrix(dist)
  -------------------------------------------------------
  sigma --QPREP(dst, dist)--> sigma[Q[dst] := dm,
                                     PC := PC + 1]
```

**Quantum Kernel (QKERNEL):**
```
    Q[src] != NULL     dm' = kernel_k.apply(Q[src], R[c0], R[c1])
  -------------------------------------------------------
  sigma --QKERNEL(dst, src, k, c0, c1)--> sigma[Q[dst] := dm',
                                                  PSW := update_qmeta(PSW, dm'),
                                                  PC := PC + 1]
```

**Quantum Observation (QOBSERVE):**
```
    Q[src] != NULL     (v, _) = measure_all(Q[src])
  -------------------------------------------------------
  sigma --QOBSERVE(dst_h, src_q)--> sigma[H[dst_h] := Dist([(v, 1.0)]),
                                           Q[src_q] := NULL,
                                           PSW.DF := true,
                                           PC := PC + 1]
```

**Hybrid Reduce (HREDUCE):**
```
    H[src] = val     result = reduce(val, func)
  -------------------------------------------------------
  sigma --HREDUCE(src, dst, func)--> sigma[target_reg[dst] := result,
                                           PC := PC + 1]
```

**Halt (HALT):**
```
  -------------------------------------------------------
  sigma --HALT--> sigma[PSW.trap_halt := true]
```

**Non-destructive Sample (QSAMPLE):**
```
    Q[src] != NULL     probs = diagonal(Q[src])
  -------------------------------------------------------
  sigma --QSAMPLE(dst_h, src_q, mode, ctx0, ctx1)-->
    sigma[H[dst_h] := extract(Q[src], mode, R[ctx0], R[ctx1]),
          PC := PC + 1]
```

Note: Q[src_q] is NOT consumed.

**Quantum Kernel with Float Context (QKERNELF):**
```
    Q[src] != NULL     dm' = kernel_k.apply(Q[src])
  -------------------------------------------------------
  sigma --QKERNELF(dst, src, k, fctx0, fctx1)-->
    sigma[Q[dst] := dm',
          PSW := update_qmeta(PSW, dm'),
          PC := PC + 1]
```

Context parameters are read from the F-file: F[fctx0], F[fctx1].

**Quantum Kernel with Complex Context (QKERNELZ):**

Same form as QKERNELF, context from Z-file: Z[zctx0], Z[zctx1].

**Register-Parameterized Preparation (QPREPR):**
```
                  dist_id = R[dist_reg] as u8
                  dm = init_density_matrix(dist_id)
  -------------------------------------------------------
  sigma --QPREPR(dst, dist_reg)--> sigma[Q[dst] := dm,
                                          PC := PC + 1]
```

**Amplitude Encoding (QENCODE):**
```
    psi = read_regs(file_sel, src_base, count)
    dm = |psi><psi| / <psi|psi>
  -------------------------------------------------------
  sigma --QENCODE(dst, src_base, count, file_sel)-->
    sigma[Q[dst] := dm,
          PC := PC + 1]
```

count must be a power of 2. file_sel selects R (0), F (1), or Z (2).

**Masked Hadamard (QHADM):**
```
    Q[src] != NULL     mask = R[mask_reg]
    dm' = apply_H_to_selected_qubits(Q[src], mask)
  -------------------------------------------------------
  sigma --QHADM(dst, src, mask_reg)--> sigma[Q[dst] := dm',
                                              PSW := update_qmeta(PSW, dm'),
                                              PC := PC + 1]
```

**Masked Bit Flip (QFLIP):**

Same form as QHADM, applying Pauli-X instead of Hadamard.

**Masked Phase Flip (QPHASE):**

Same form as QHADM, applying Pauli-Z instead of Hadamard.

### 9.4 Execution Semantics

A program P executes from initial state sigma_0:

```
run(sigma_0, P) = sigma_n
  where sigma_n = step^n(sigma_0, P)
  and   sigma_n.PSW.trap_halt = true
  or    sigma_n.PC >= |P|
```

### 9.5 Resource Accounting

Each transition produces a resource delta:

```
step_r : Sigma x Program -> (Sigma, ResourceDelta)
R_total = sum_{i=0}^{n-1} delta_i
```
