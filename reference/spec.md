# CQAM Machine Specification

## 1. Overview

The Classical-Quantum Abstract Machine (CQAM) is a register-based virtual machine
with a 32-bit fixed-width instruction word architecture. It combines classical
integer, floating-point, and complex arithmetic with a hybrid quantum simulation
model and a classical-quantum execution bridge.

Design philosophy:
- Quantum state is internally represented by a unified `QuantumRegister` enum
  that dispatches between a pure-state `Statevector` (O(2^n) memory, default)
  and a mixed-state `DensityMatrix` (O(4^n) memory, required for decoherence
  and partial-trace operations). The VM promotes Pure to Mixed automatically
  when a mixed-state operation is applied.
- All instructions encode into a single 32-bit word with an 8-bit opcode prefix.
- The machine supports five register files, two memory banks, a hardware call
  stack, and a two-level interrupt model.
- Programs may use `#! qubits N` pragma directives to specify the default qubit
  count for that program.

## 2. Machine State

### 2.1 Program Counter (PC)

- Type: `usize` (index into instruction memory)
- The executor is the sole authority on PC advancement.
- Non-jump instructions advance PC by 1 after execution.
- Jump instructions (JMP, JIF, CALL, JMPF) set PC directly.

### 2.2 Register Files

| File | Notation | Count | Element Type | Description |
|------|----------|-------|--------------|-------------|
| IntRegFile | R0-R15 | 16 | i64 | Integer arithmetic, comparisons, predicates |
| FloatRegFile | F0-F15 | 16 | f64 | Floating-point arithmetic |
| ComplexRegFile | Z0-Z15 | 16 | (f64, f64) | Complex number arithmetic |
| Quantum registers | Q0-Q7 | 8 | Option\<QuantumRegister\> | Pure or mixed quantum state |
| HybridRegFile | H0-H7 | 8 | HybridValue | Measurement results (Dist, Int, Float, Complex, Empty) |

### 2.3 Memory Banks

| Bank | Address Type | Size | Element Type | Description |
|------|-------------|------|--------------|-------------|
| CMEM | u16 | 65536 cells | i64 | Classical memory, heap-allocated |
| QMEM | u8 | 256 slots | Option\<QuantumRegister\> | Quantum memory |

CMEM is accessed by ILdm, IStr, FLdm, FStr, ZLdm, ZStr and their register-indirect
variants (ILdx, IStrx, FLdx, FStrx, ZLdx, ZStrx).
QMEM is accessed by QLoad, QStore.

### 2.4 Call Stack

- Type: `Vec<usize>` (hardware stack for CALL/RET)
- CALL pushes PC+1 onto the stack and jumps to the target label.
- RET pops the top address and jumps to it. If the stack is empty, RET acts as HALT.
- ISR handlers also push the current PC for return via RETI.

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
3. The handler returns via RETI to resume normal execution.

Handlers are registered via `SETIV trap_id, label`. RETI pops the saved PC and
clears all maskable trap flags before resuming.

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

### 5.3 Pragma Directives

The parser recognizes `#! qubits N` pragma lines. The parsed qubit count is
stored in `ProgramMetadata` and applied by the runner as the default qubit
count for `QPREP` and related instructions, subject to CLI override.

## 6. Quantum Model

### 6.1 QuantumRegister -- Dual-Backend Representation

Each quantum register slot Q[k] holds an `Option<QuantumRegister>`. The
`QuantumRegister` enum has two variants:

- **`Pure(Statevector)`**: A length-2^n complex amplitude vector. Stores
  |psi> = sum_k alpha_k |k>. Gate application is O(2^n). This is the default
  representation for all state preparations and kernel operations.
- **`Mixed(DensityMatrix)`**: A 2^n x 2^n Hermitian, trace-1, positive
  semi-definite matrix rho. Gate application is O(4^n) via rho' = U rho U†.
  Required for partial trace (QPTRACE), explicit mixing (QMIXED), and
  purity-based fidelity monitoring.

Auto-promotion rules:
- QPTRACE always produces a Mixed result.
- QMIXED always produces a Mixed result.
- Tensor product of (Pure, Pure) produces Pure; any Mixed operand yields Mixed.
- Kernel application: Pure uses the fast statevector path; if the kernel does
  not support statevector mode, it promotes the register to Mixed and retries.
- The `--density-matrix` CLI flag (or `force_density_matrix = true` in config)
  causes all QPREP/QPREPN/QPREPR/QPrepN constructions to create Mixed registers.

### 6.2 Measurement

- **Full measurement (QOBSERVE):** Stochastic; samples via the Born rule.
  Produces a collapsed state and a `HybridValue::Dist` containing `[(k, 1.0)]`.
  Destructive: Q[src] is set to None and PSW.DF is set.
- **Non-destructive sample (QSAMPLE):** Extracts probabilities from Q[src]
  without consuming it. Q[src] remains available for further operations.
- **Single-qubit measurement (QMEAS):** Measures one qubit, stores the 0/1
  outcome in an integer register. The quantum register is updated to the
  post-measurement state (projected and renormalized) but is not consumed.
- **Deterministic measurement:** `measure_deterministic()` returns argmax of
  |alpha_k|^2 or rho_kk (for testing only; not exposed as an ISA instruction).

### 6.3 Fidelity Metrics

Fidelity metrics are computed on `DensityMatrix` (Mixed registers). Pure
registers report purity = 1.0 by definition.

- `purity()`: Tr(rho^2) = sum_{i,j} |rho[i][j]|^2. Equals 1.0 for pure
  states; strictly less than 1.0 for mixed states.
- `von_neumann_entropy()`: True S(rho) = -Tr(rho log rho), computed via
  Jacobi eigendecomposition of rho to obtain eigenvalues {lambda_k}.
  S = -sum_k lambda_k * log(lambda_k) (zero terms skipped). Normalized to [0,1]
  by dividing by log(dim). Equals 0 for pure states; equals 1 for maximally
  mixed states.
- `diagonal_entropy()`: Shannon entropy of the diagonal probabilities
  p_k = rho_kk. Fast approximation; does not capture off-diagonal coherences.
  Formerly named `von_neumann_entropy()`.
- `entanglement_entropy()`: Von Neumann entropy of the reduced density matrix
  Tr_B(rho), obtained via partial trace. Quantifies bipartite entanglement.
- `concentration_metric()`: Inverse Herfindahl index on a QDist; measures
  how concentrated the measurement outcome distribution is.

Purity-based fidelity monitoring: after each QKERNEL or QOBSERVE, if purity
falls below `SimConfig::fidelity_threshold`, the VM sets `int_quantum_err`.

### 6.4 Kernels (11 implemented)

| ID | Name              | Description |
|----|-------------------|-------------|
| 0  | init              | Re-initialize to uniform superposition H^n|0> |
| 1  | entangle          | CNOT cascade for GHZ-like entanglement |
| 2  | fourier           | Quantum Fourier Transform |
| 3  | diffuse           | Grover diffusion (inversion about the mean) |
| 4  | grover_iter       | Complete Grover iteration (oracle phase-flip + diffusion) |
| 5  | rotate            | Diagonal rotation: U[k][k] = exp(i * theta * k); theta from F-file |
| 6  | phase_shift       | Phase shift: U[k][k] = exp(i * |z| * k); amplitude from Z-file |
| 7  | fourier_inv       | Inverse Quantum Fourier Transform |
| 8  | controlled_u      | Controlled-U: applies any sub-kernel conditioned on a control qubit, supports C-U^{2^k} |
| 9  | diagonal_unitary  | Arbitrary diagonal unitary from CMEM complex pairs: d_k = (re, im) at CMEM[base+2k], CMEM[base+2k+1] |
| 10 | permutation       | Basis-state permutation from CMEM: sigma(k) at CMEM[base+k] as plain i64 |

### 6.5 Observation Modes

QOBSERVE and QSAMPLE support three extraction modes, selected by the `mode`
field:

| Mode | ID | Output Type | Semantics |
|------|----|-------------|-----------|
| DIST | 0 | Dist(Vec<(u16, f64)>) | Full diagonal probability distribution (default mode). |
| PROB | 1 | Float(f64) | Probability of a single basis state at index R[ctx0]. Returns p_k = |alpha_k|^2 or rho_kk. |
| AMP  | 2 | Complex(f64, f64) | Quantum register element at (row, col) where row=R[ctx0], col=R[ctx1]. |

QOBSERVE is destructive: it consumes Q[src] (sets to None) and marks PSW.DF.
QSAMPLE is non-destructive: Q[src] remains available for further operations.

### 6.6 Masked Gate Operations

Three masked gate instructions apply single-qubit Pauli gates to selected
qubits of a quantum register. The selection mask is read from an integer
register R[mask_reg]. For each bit i (0-indexed from LSB) that is set in the
mask, the corresponding gate is applied to qubit i. Bits beyond num_qubits
are silently ignored.

| Instruction | Gate | Matrix |
|-------------|------|--------|
| QHADM | Hadamard | (1/sqrt(2)) [[1,1],[1,-1]] |
| QFLIP | Pauli-X | [[0,1],[1,0]] |
| QPHASE | Pauli-Z | [[1,0],[0,-1]] |

### 6.7 Qubit-Level Gate Operations

| Instruction | Description |
|-------------|-------------|
| QCNOT | CNOT gate: ctrl=R[ctrl_qubit_reg], tgt=R[tgt_qubit_reg]. Traps if ctrl==tgt or either index >= num_qubits. |
| QCZ | Controlled-Z gate: ctrl=R[ctrl_qubit_reg], tgt=R[tgt_qubit_reg]. |
| QSWAP | SWAP gate: swaps qubits at R[qubit_a_reg] and R[qubit_b_reg]. |
| QROT | Parameterized single-qubit rotation R_axis(theta): axis in {X=0,Y=1,Z=2}, theta from F[angle_freg], qubit from R[qubit_reg]. |
| QMEAS | Measure one qubit: R[dst_r] = 0 or 1; Q[src_q] updated to post-measurement state (not consumed). |

### 6.8 Mixed-State Operations

| Instruction | Description |
|-------------|-------------|
| QMIXED | Build rho = sum_i w_i |psi_i><psi_i| from CMEM. Always produces Mixed. |
| QPREPN | Prepare with runtime-specified qubit count: Q[dst] = new_state(dist, num_qubits=R[qubit_count_reg]). |
| QPTRACE | Partial trace: Q[dst] = Tr_B(Q[src]); num_qubits_a=R[reg]; non-destructive; always Mixed. |
| QRESET | Reset one qubit to |0>: measure the qubit; if outcome=1, apply X to flip. Q[src] updated, not consumed. |
| QTENSOR | Tensor product: Q[dst] = Q[src0] tensor Q[src1]; both sources are consumed. |
| QCUSTOM | Custom unitary: Q[dst] = U * Q[src] * U†; U read from CMEM[R[base_addr_reg]], dim from R[dim_reg]. |

## 7. Float Math Instructions

CQAM provides four transcendental float operations for use in angle computations
and classical preprocessing of quantum parameters:

| Instruction | Description |
|-------------|-------------|
| FSIN  | F[dst] = sin(F[src]) |
| FCOS  | F[dst] = cos(F[src]) |
| FATAN2 | F[dst] = atan2(F[lhs], F[rhs]) (lhs=y, rhs=x, standard math convention) |
| FSQRT | F[dst] = sqrt(F[src]); traps if F[src] < 0 |

## 8. Hybrid Execution Model

### 8.1 HFORK / HMERGE

HFORK marks the beginning of a parallel execution region by setting the hybrid
mode and fork flags in the PSW. HMERGE ends the region by setting the merge
flag. Current implementation is flag-based (not thread-based).

### 8.2 JMPF

Conditional execution based on PSW flags. Reads the specified flag ID from the
PSW and jumps to the target label if the flag is set.

### 8.3 HREDUCE

17 reduction functions organized into four categories:

| IDs | Category | Output | Functions |
|-----|----------|--------|-----------|
| 0-5 | Float-to-Int | R[dst] (i64) | round, floor, ceil, trunc, abs, negate |
| 6-9 | Complex-to-Float | F[dst] (f64) | magnitude, phase, real, imag |
| 10-13 | Distribution | F[dst] or R[dst] | mean, mode, argmax, variance |
| 14-15 | Complex-to-Complex | Z[dst] (f64, f64) | conj_z, negate_z |
| 16 | Expectation | F[dst] (f64) | expect: sum_k eigenvalue_k * p_k |

The `expect` function (ID 16) reads `n` eigenvalues as f64 from CMEM starting
at R[ctx], where n is the distribution length, and computes the weighted sum
against the distribution probabilities.

## 9. Resource Tracking

Each instruction has an associated `ResourceDelta` with five fields:

| Field | Description |
|-------|-------------|
| time | Execution cycles consumed |
| space | Register/memory slots written |
| superposition | Superposition created or consumed |
| entanglement | Entanglement created or consumed |
| interference | Interference effects (from measurement) |

The `ResourceTracker` accumulates deltas across execution for reporting.

## 10. Formal Operational Semantics

### 10.1 Machine State

The machine state is a tuple:

```
Sigma = (PC, R, F, Z, Q, H, CMEM, QMEM, PSW, CS)
```

where:
- `PC : N` -- program counter (instruction index)
- `R : [0..15] -> Z` -- integer register file (64-bit signed integers)
- `F : [0..15] -> R` -- floating-point register file (64-bit IEEE 754)
- `Z : [0..15] -> C` -- complex register file (pairs of f64)
- `Q : [0..7] -> QuantumRegister | NULL` -- quantum register file
- `H : [0..7] -> HybridValue | EMPTY` -- hybrid register file
- `CMEM : [0..65535] -> Z` -- classical memory (64-bit cells)
- `QMEM : [0..255] -> QuantumRegister | NULL` -- quantum memory
- `PSW : PSW_State` -- program status word (all flags)
- `CS : List<N>` -- call stack (return addresses)

### 10.2 State Transition Function

The single-step transition function is:

```
step : Sigma x Program -> Sigma
step(sigma, P) = dispatch(sigma, P[sigma.PC])
```

### 10.3 Transition Rules

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

**Interrupt Handler Setup (SETIV):**
```
                  addr = labels(target)
  -------------------------------------------------------
  sigma --SETIV(trap_id, target)--> sigma[ISR[trap_id] := addr,
                                          PC := PC + 1]
```

**Interrupt Return (RETI):**
```
                  addr = CS.top     CS' = CS.pop
  -------------------------------------------------------
  sigma --RETI--> sigma[PC := addr,
                         CS := CS',
                         PSW := clear_maskable_traps(PSW)]
```

**Quantum Preparation (QPREP):**
```
                  qr = new_quantum_register(dist, default_qubits, force_dm)
  -------------------------------------------------------
  sigma --QPREP(dst, dist)--> sigma[Q[dst] := qr,
                                     PC := PC + 1]
```

**Quantum Preparation with Runtime Qubit Count (QPREPN):**
```
                  n = R[qubit_count_reg]
                  qr = new_quantum_register(dist, n, force_dm)
  -------------------------------------------------------
  sigma --QPREPN(dst, dist, qubit_count_reg)--> sigma[Q[dst] := qr,
                                                       PC := PC + 1]
```

**Quantum Kernel (QKERNEL):**
```
    Q[src] != NULL     qr' = apply_kernel(Q[src], k, R[c0], R[c1])
  -------------------------------------------------------
  sigma --QKERNEL(dst, src, k, c0, c1)--> sigma[Q[dst] := qr',
                                                  PSW := update_qmeta(PSW, qr'),
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

**Single-Qubit Measurement (QMEAS):**
```
    Q[src_q] != NULL     (outcome, qr') = measure_qubit(Q[src_q], R[qubit_reg])
  -------------------------------------------------------
  sigma --QMEAS(dst_r, src_q, qubit_reg)--> sigma[R[dst_r] := outcome as i64,
                                                    Q[src_q] := qr',
                                                    PC := PC + 1]
```

**CNOT Gate (QCNOT):**
```
    Q[src] != NULL     ctrl = R[ctrl_qubit_reg]     tgt = R[tgt_qubit_reg]
    qr' = apply_cnot(Q[src], ctrl, tgt)
  -------------------------------------------------------
  sigma --QCNOT(dst, src, ctrl_qubit_reg, tgt_qubit_reg)--> sigma[Q[dst] := qr',
                                                                    PC := PC + 1]
```

**Partial Trace (QPTRACE):**
```
    Q[src] != NULL     na = R[num_qubits_a_reg]
    qr' = partial_trace_b(Q[src], na)     // always Mixed
  -------------------------------------------------------
  sigma --QPTRACE(dst, src, num_qubits_a_reg)--> sigma[Q[dst] := qr',
                                                         PC := PC + 1]
```

Note: Q[src] is NOT consumed.

**Tensor Product (QTENSOR):**
```
    Q[src0] != NULL     Q[src1] != NULL
    qr' = tensor_product(Q[src0], Q[src1])
  -------------------------------------------------------
  sigma --QTENSOR(dst, src0, src1)--> sigma[Q[dst] := qr',
                                            Q[src0] := NULL,
                                            Q[src1] := NULL,
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
    Q[src] != NULL
  -------------------------------------------------------
  sigma --QSAMPLE(dst_h, src_q, mode, ctx0, ctx1)-->
    sigma[H[dst_h] := extract(Q[src], mode, R[ctx0], R[ctx1]),
          PC := PC + 1]
```

Note: Q[src_q] is NOT consumed.

**Quantum Kernel with Float Context (QKERNELF):**
```
    Q[src] != NULL     qr' = apply_kernel(Q[src], k, F[fctx0], F[fctx1])
  -------------------------------------------------------
  sigma --QKERNELF(dst, src, k, fctx0, fctx1)-->
    sigma[Q[dst] := qr',
          PSW := update_qmeta(PSW, qr'),
          PC := PC + 1]
```

**Quantum Kernel with Complex Context (QKERNELZ):**

Same form as QKERNELF, context parameters read from Z-file: Z[zctx0], Z[zctx1].

**Register-Parameterized Preparation (QPREPR):**
```
                  dist_id = R[dist_reg] as u8
                  qr = new_quantum_register(dist_id, default_qubits, force_dm)
  -------------------------------------------------------
  sigma --QPREPR(dst, dist_reg)--> sigma[Q[dst] := qr,
                                          PC := PC + 1]
```

**Amplitude Encoding (QENCODE):**
```
    psi = read_regs(file_sel, src_base, count)
    qr = Pure(normalize(psi))
  -------------------------------------------------------
  sigma --QENCODE(dst, src_base, count, file_sel)-->
    sigma[Q[dst] := qr,
          PC := PC + 1]
```

count must be a power of 2. file_sel selects R (0), F (1), or Z (2).
QENCODE always produces a Pure register.

### 10.4 Execution Semantics

A program P executes from initial state sigma_0:

```
run(sigma_0, P) = sigma_n
  where sigma_n = step^n(sigma_0, P)
  and   sigma_n.PSW.trap_halt = true
  or    sigma_n.PC >= |P|
```

### 10.5 Resource Accounting

Each transition produces a resource delta:

```
step_r : Sigma x Program -> (Sigma, ResourceDelta)
R_total = sum_{i=0}^{n-1} delta_i
```

## 11. Data Section

CQAM programs support a `.data` section for declaring initialized CMEM contents
at assembly time. The data section is processed before `.code` and populates
CMEM cells that are available to the program at address 0 onward.

### 11.1 Directives

| Directive | Syntax | Description |
|-----------|--------|-------------|
| `.org` | `.org N` | Set allocation pointer to address N |
| `.ascii` | `.ascii "string"` | One ASCII byte per cell, NUL-terminated |
| `.i64` | `.i64 v1, v2, ...` | Literal i64 values |
| `.f64` | `.f64 v1, v2, ...` | f64 values stored as `to_bits() as i64` |
| `.c64` | `.c64 z1, z2, ...` | Complex values in `aJb` format; 2 cells per entry |

### 11.2 Label Resolution

Labels declared in `.data` (e.g., `mydata:`) create two reference forms
available in `.code`:
- `@mydata` -- resolves to the CMEM base address of that label
- `@mydata.len` -- resolves to the logical entry count (for `.c64`, the number
  of complex entries, not the raw cell count; for `.ascii`, the byte count
  excluding the NUL terminator)

### 11.3 `.c64` Format

Complex literals use `realJimag` format. Both parts support scientific notation.
A trailing comma continues the directive on the next line:

```
.c64 1.0J0.0               # 1 + 0i
.c64 -1.5J2.5              # -1.5 + 2.5i
.c64 1.5e-3J-2.0e1         # scientific notation
.c64 0J1.0                 # pure imaginary

.c64 1.0J0.0,  1.0J0.0,
     1.0J0.0, -1.0J0.0     # continuation across lines
```

Each `.c64` entry occupies two consecutive CMEM cells: the real part as
`f64::to_bits() as i64` at `base + 2k`, and the imaginary part at
`base + 2k + 1`.

### 11.4 Example

```
.data
    .org 200
diag:
    .c64 1.0J0.0, -1.0J0.0,
         1.0J0.0,  1.0J0.0

    .org 1000
msg:
    .ascii "Result = %d\n"

.code
    ILDI R0, @diag         # R0 = 200 (CMEM base address of diag)
    ILDI R1, @diag.len     # R1 = 4  (complex entry count)
    QKERNEL Q1, Q0, 9, R0, R1
```
