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
  count for that program, and `#! threads N` to declare the default thread count
  for HFORK parallel regions.

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
| QMEM | u8 | 256 slots | Option\<QuantumRegister\> | Quantum memory (teleportation semantics) |

CMEM is accessed by ILdm, IStr, FLdm, FStr, ZLdm, ZStr and their register-indirect
variants (ILdx, IStrx, FLdx, FStrx, ZLdx, ZStrx).
QMEM is accessed by QLoad and QStore. Both operations consume one Bell pair from
`bell_pair_budget` (see section 2.5). The state exists in exactly one location
before and after each operation.

### 2.4 Call Stack

- Type: `Vec<usize>` (hardware stack for CALL/RET)
- CALL pushes PC+1 onto the stack and jumps to the target label.
- RET pops the top address and jumps to it. If the stack is empty, RET acts as HALT.
- ISR handlers also push the current PC for return via RETI.

### 2.5 Bell Pair Budget

- Type: `u32` (counter on ExecutionContext, initialized from VmConfig)
- Each QSTORE or QLOAD consumes one Bell pair (decrements by 1).
- Default value: 256 (one Bell pair per QMEM slot).
- A budget of 0 means unlimited: no budget check is performed.
- When the budget is non-zero and reaches 0, the next QSTORE or QLOAD sets
  `psw.int_quantum_err = true` and raises `CqamError::BellPairExhausted`.
  If a QuantumError ISR handler is registered (via `SETIV`), execution jumps
  to the handler; otherwise the default action (trap_halt) applies.
- Configurable via the `bell_pair_budget` field in `VmConfig` / TOML config.

## 3. Program Status Word (PSW)

### 3.1 Classical Condition Flags

| Flag | Description |
|------|-------------|
| ZF | Zero flag: set when result == 0 |
| NF | Negative flag: set when result < 0 |
| OF | Overflow flag (reserved, always false) |
| PF | Predicate flag: set from comparison results |

### 3.2 Quantum State Flags

SF, EF, and IF are intent-based flags. They are set according to the identity of
the kernel applied, not by dynamic state inspection: SF is set by kernels that
create superposition (UNIT, QFFT, QIFT, DIFF, GROV, DROT, PHSH); EF is set by
kernels that create entanglement (ENTG, GROV, CTLU) and by QPREP with BELL/GHZ
distributions; IF is set by kernels that exploit interference (QFFT, QIFT, DIFF,
GROV).

| Flag | ID | Description |
|------|----|-------------|
| QF | 4 | Quantum active: set after any QPREP/QKERNEL execution |
| SF | 5 | Superposition created: set by kernels that create superposition |
| EF | 6 | Entanglement created: set by kernels that create entanglement |
| IF | 12 | Interference: set by kernels that exploit interference |
| DF | — | Decohered: set after measurement |
| CF | — | Collapsed distribution |

### 3.3 Hybrid Flags

| Flag | Description |
|------|-------------|
| HF | Hybrid mode active |
| forked | HFORK has been executed |
| merged | HMERGE has been executed |
| AF (ID 13) | Atomic section active: set on the elected leader thread between HATMS and HATME; cleared at HATME and HMERGE |

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

The parser recognizes two `#!` pragma forms. Parsed values are stored in
`ProgramMetadata` and applied by the runner before execution, subject to CLI
override.

| Pragma | Syntax | Effect |
|--------|--------|--------|
| `qubits` | `#! qubits N` | Default qubit count for `QPREP` and related instructions. Overridden by `--qubits`. |
| `threads` | `#! threads N` | Default thread count for `HFORK` (1-256). Overridden by `--threads`. |

Thread count precedence: CLI `--threads` > `#! threads N` pragma > default (1).

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

CQAM enforces physical realism: all observation is destructive or partial.
Non-destructive reading of a quantum state (sampling without collapse) has no
physical basis and is not supported by the ISA.

- **Full measurement (QOBSERVE):** Stochastic; samples via the Born rule.
  Produces a collapsed outcome and a `HybridValue::Dist` containing `[(k, 1.0)]`.
  Destructive: Q[src] is set to None and PSW.DF is set. Supports three modes
  (DIST, PROB, AMP) controlled by the `mode` field; all modes consume Q[src].
- **Single-qubit measurement (QMEAS):** Measures one qubit via a projective
  measurement, stores the 0/1 outcome in an integer register. The quantum
  register is updated to the post-measurement state (projected and renormalized)
  but is not fully consumed; remaining qubits retain their coherence.
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

QOBSERVE supports three extraction modes, selected by the `mode` field. All
modes are destructive: Q[src] is consumed (set to None) and PSW.DF is marked.

| Mode | ID | Output Type | Semantics |
|------|----|-------------|-----------|
| DIST | 0 | Dist(Vec<(u16, f64)>) | Full diagonal probability distribution (default mode). |
| PROB | 1 | Float(f64) | Probability of a single basis state at index R[ctx0]. Returns p_k = |alpha_k|^2 or rho_kk. |
| AMP  | 2 | Complex(f64, f64) | Quantum register element at (row, col) where row=R[ctx0], col=R[ctx1]. |

When partial classical information is needed without fully consuming a quantum
register, use QMEAS to measure individual qubits. The register is updated to
the post-measurement (projected) state but is not set to None.

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

### 8.1 SPMD Execution Model

CQAM parallel execution uses the Single Program Multiple Data (SPMD) model:
all threads execute the same instruction stream simultaneously, differentiated
only by their thread identity. The thread count N is determined by pragma or
CLI (see section 5.3). Thread IDs are 0-based; thread 0 is the primary thread.

Programs query the execution context with two instructions:
- `ICCFG R_dst` — loads the configured thread count into R[dst].
- `ITID R_dst` — loads the current thread's ID (0-based) into R[dst]; returns
  0 outside an HFORK/HMERGE block.

Both instructions encode in the RR format with the source field set to 0.

### 8.2 HFORK / HMERGE

`HFORK` begins a parallel execution region:
1. Requires QF=0. If any quantum register is occupied, HFORK raises
   `ForkError` because live quantum state must be transferred to the
   `SharedQuantumFile` before threading begins.
2. The existing Q registers are moved into a `SharedQuantumFile`: an array
   of 8 per-register mutexes (`Arc<Mutex<Option<QuantumRegister>>>`). Threads
   block on contention for the same register slot but can access different
   slots concurrently.
3. If a `.shared` section is declared, the relevant CMEM cells are copied into
   a `SharedMemory` object with snapshot-commit consistency (see section 8.4).
4. A `ThreadBarrier` is constructed for N threads to support HATMS/HATME.
5. N-1 worker threads are spawned via `std::thread::Builder`. Each worker
   receives a cloned `ExecutionContext` with PC set to HFORK+1, its thread ID
   set, and references to the shared resources.
6. All N threads (including thread 0) proceed at HFORK+1.

When N=1, HFORK is a no-op aside from setting the hybrid PSW flags, allowing
single-threaded programs to execute SPMD code without modification.

`HMERGE` ends the parallel region:
1. Requires QF=0 (all quantum work must be completed and observed).
2. Worker threads detect HMERGE by observing `psw.merged = true` at the top
   of their execution loop and exit.
3. Thread 0 calls `join_all()`, collecting results from all worker handles.
4. Quantum registers are restored from the `SharedQuantumFile`.
5. The final committed shared memory state is written back into thread 0's CMEM.

### 8.3 HATMS / HATME — Atomic Sections

Atomic sections provide a safe, serialized region for shared-memory updates
inside an HFORK/HMERGE block.

`HATMS` (Hybrid Atomic Section Start):
1. Raises `ForkError` if called outside a forked region.
2. All N threads arrive at a full `ThreadBarrier`. The first thread to arrive
   is elected leader. All threads block until the last one arrives.
3. The elected leader proceeds with `psw.af = true` and `in_atomic_section = true`.
4. Non-leaders set `skip_to_hatme = true` and skip all instructions until
   reaching HATME, where they resume normal execution.

`HATME` (Hybrid Atomic Section End):
1. The leader commits the current `SharedMemory` live data as the new snapshot
   (visible to all threads on their next snapshot read).
2. The leader clears `in_atomic_section` and `psw.af`.
3. A second full barrier synchronizes all threads: non-leaders that were
   skipping resume here, and all threads proceed past HATME together.

The PSW flag `AF` (ID 13) is set only on the elected leader for the duration
of the atomic section. It can be tested with `JMPF AF, label`.

Constraints enforced at runtime:
- Writes to `.shared` CMEM cells outside an atomic section raise
  `SharedMemoryViolation`.
- Quantum instructions (QPREP, QKERNEL, etc.) inside an atomic section raise
  `QuantumInAtomicSection`.

### 8.4 Shared Memory Consistency

`SharedMemory` implements a snapshot-commit model over a declared CMEM region.
Two independent copies are maintained internally:
- **live data** (`RwLock<Vec<i64>>`): written exclusively by the leader inside
  an atomic section.
- **snapshot** (`RwLock<Vec<i64>>`): a frozen copy promoted from live data at
  each `HATME`.

Read semantics:
- Outside atomic section: returns the snapshot value (last committed state).
- Inside atomic section (leader only): returns the live value.

This ensures non-leader threads see a consistent view of shared data between
atomic sections and do not observe partial writes from a concurrent leader.

At `HMERGE`, the final live data is written back to thread 0's CMEM via
`write_back()`.

### 8.5 JMPF

Conditional execution based on PSW flags. Uses flag name syntax:
`JMPF FLAG_NAME, target` (e.g., `JMPF EF, entangled_path`). The flag name
is assembled to the corresponding flag ID in the binary encoding. Jumps to
the target label if the named flag is set.

### 8.6 HREDUCE

Syntax: `HREDUCE MNEM, H_src, R/F/Z_dst` (e.g., `HREDUCE ARGMX, H0, R2`).

17 reduction functions in five-letter mnemonics, organized into four categories:

| IDs | Category | Output | Mnemonics |
|-----|----------|--------|-----------|
| 0-5 | Float-to-Int | R[dst] (i64) | ROUND, FLOOR, CEILI, TRUNC, ABSOL, NEGAT |
| 6-9 | Complex-to-Float | F[dst] (f64) | MAGNI, PHASE, REALP, IMAGP |
| 10-13 | Distribution | F[dst] or R[dst] | MEANT (F), MODEV (R), ARGMX (R), VARNC (F) |
| 14-15 | Complex-to-Complex | Z[dst] (f64, f64) | CONJZ, NEGTZ |
| 16 | Expectation | F[dst] (f64) | EXPCT: sum_k eigenvalue_k * p_k |

The `EXPCT` function (ID 16) reads `n` eigenvalues as f64 from CMEM starting
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
Sigma = (PC, R, F, Z, Q, H, CMEM, QMEM, PSW, CS, TID, N_threads, BPB)
```

where:
- `PC : N` -- program counter (instruction index)
- `R : [0..15] -> Z` -- integer register file (64-bit signed integers)
- `F : [0..15] -> R` -- floating-point register file (64-bit IEEE 754)
- `Z : [0..15] -> C` -- complex register file (pairs of f64)
- `Q : [0..7] -> QuantumRegister | NULL` -- quantum register file
- `H : [0..7] -> HybridValue | EMPTY` -- hybrid register file
- `CMEM : [0..65535] -> Z` -- classical memory (64-bit cells)
- `QMEM : [0..255] -> QuantumRegister | NULL` -- quantum memory (consume-on-use)
- `PSW : PSW_State` -- program status word (all flags)
- `CS : List<N>` -- call stack (return addresses)
- `TID : N` -- thread identity (0-based index; 0 in single-threaded context)
- `N_threads : N` -- configured thread count (from pragma or CLI)
- `BPB : N | unlimited` -- Bell pair budget; 0 encodes unlimited

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

**Increment (IINC):**
```
                  v = R[src] + 1
  -------------------------------------------------------
  sigma --IINC(dst, src)--> sigma[R[dst] := v,
                                   PSW := update_arith(PSW, v),
                                   PC := PC + 1]
```

**Decrement (IDEC):**
```
                  v = R[src] - 1
  -------------------------------------------------------
  sigma --IDEC(dst, src)--> sigma[R[dst] := v,
                                   PSW := update_arith(PSW, v),
                                   PC := PC + 1]
```

**Integer Register Copy (IMOV):**
```
                  v = R[src]
  -------------------------------------------------------
  sigma --IMOV(dst, src)--> sigma[R[dst] := v,
                                   PSW.ZF := (v == 0),
                                   PSW.SF := (v < 0),
                                   PC := PC + 1]
```

**Float Register Copy (FMOV):**
```
                  v = F[src]
  -------------------------------------------------------
  sigma --FMOV(dst, src)--> sigma[F[dst] := v,
                                   PC := PC + 1]
```

PSW is not modified by FMOV.

**Complex Register Copy (ZMOV):**
```
                  v = Z[src]
  -------------------------------------------------------
  sigma --ZMOV(dst, src)--> sigma[Z[dst] := v,
                                   PC := PC + 1]
```

PSW is not modified by ZMOV.

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

**Thread Count Query (ICCFG):**
```
  -------------------------------------------------------
  sigma --ICCFG(dst)--> sigma[R[dst] := sigma.N_threads,
                               PSW.ZF := (N_threads == 0),
                               PC := PC + 1]
```

**Thread Identity Query (ITID):**
```
  -------------------------------------------------------
  sigma --ITID(dst)--> sigma[R[dst] := sigma.TID,
                              PSW.ZF := (TID == 0),
                              PC := PC + 1]
```

**Atomic Section Start (HATMS):**
```
    PSW.forked = true
    barrier.wait(TID) => BarrierWaitResult { is_leader }
  -------------------------------------------------------
  sigma --HATMS-->
    sigma[PSW.af := is_leader,
          in_atomic_section := is_leader,
          skip_to_hatme := !is_leader,
          PC := PC + 1]
```

Non-leader threads set `skip_to_hatme` and skip forward to HATME. The full
barrier ensures all N threads have arrived before any proceeds.

**Atomic Section End (HATME):**
```
    in_atomic_section = true (leader)
    SharedMemory.commit_snapshot()
    barrier.wait(TID)   // second full barrier
  -------------------------------------------------------
  sigma --HATME-->
    sigma[PSW.af := false,
          in_atomic_section := false,
          skip_to_hatme := false,
          PC := PC + 1]
```

All threads (leader and non-leaders) synchronize at the second barrier inside
HATME before any proceeds past it.

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
    H[src] = val     result = reduce(val, func_mnem)
  -------------------------------------------------------
  sigma --HREDUCE(func_mnem, src, dst)--> sigma[target_reg[dst] := result,
                                                PC := PC + 1]
```

**Halt (HALT):**
```
  -------------------------------------------------------
  sigma --HALT--> sigma[PSW.trap_halt := true]
```

**Quantum Store -- Teleportation (QSTORE):**
```
    Q[src] != NULL     BPB > 0  (or BPB = 0 meaning unlimited)
    handle = Q[src]
  -------------------------------------------------------
  sigma --QSTORE(src_q, addr)-->
    sigma[QMEM[addr] := handle,
          Q[src_q]   := NULL,
          BPB        := if BPB > 0 then BPB - 1 else 0,
          PC         := PC + 1]

    Q[src] != NULL     BPB = 1  (budget exactly exhausted)
  -------------------------------------------------------
  sigma --QSTORE(src_q, addr)--> sigma[PSW.int_quantum_err := true]
    => CqamError::BellPairExhausted { instruction: "QSTORE" }
```

Note: Q[src_q] is consumed; the state moves to QMEM[addr]. If a noise model
is active, a depolarizing channel proportional to `(1 - bell_pair_fidelity)`
is applied to QMEM[addr] after the move.

**Quantum Load -- Teleportation (QLOAD):**
```
    QMEM[addr] != NULL     BPB > 0  (or BPB = 0 meaning unlimited)
    handle = QMEM[addr]
  -------------------------------------------------------
  sigma --QLOAD(dst_q, addr)-->
    sigma[Q[dst_q]   := handle,
          QMEM[addr] := NULL,
          BPB        := if BPB > 0 then BPB - 1 else 0,
          PC         := PC + 1]

    QMEM[addr] != NULL     BPB = 1  (budget exactly exhausted)
  -------------------------------------------------------
  sigma --QLOAD(dst_q, addr)--> sigma[PSW.int_quantum_err := true]
    => CqamError::BellPairExhausted { instruction: "QLOAD" }
```

Note: QMEM[addr] is consumed; the state moves to Q[dst_q]. Teleportation noise
is applied in the same way as for QSTORE. A QSTORE followed by a QLOAD is a
move round-trip: the state returns to the Q register file, but two Bell pairs
have been consumed and noise has been applied twice (once per operation).

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
    QKERNEL DIAG, Q1, Q0, R0, R1
```

## 12. Shared and Private Memory Sections

### 12.1 `.shared` Section

The `.shared` section declares a CMEM region shared across all threads in an
HFORK/HMERGE block. It is declared at the top level alongside `.data` and
`.code`. At most one `.shared` section may appear per program.

Syntax:

```
.shared
base 0x8000
size 16
shared_counter:
    .i64 0
```

- `base` sets the CMEM address of the shared region (must be a u16 value).
- `size` sets the region length in cells.
- Labels declared inside the section follow the same `@label` / `@label.len`
  reference syntax as `.data` labels.

Consistency: snapshot-commit (see section 8.4). The initial cell values are
copied from the program's CMEM at HFORK time. After HMERGE, the final
committed state is written back to thread 0's CMEM.

### 12.2 `.private` Section

The `.private` section declares per-thread scratch space that is independent
across all threads. At most one `.private` section may appear per program.

Syntax:

```
.private
size 64
```

`size` specifies the number of cells. Each thread receives an isolated copy;
writes by one thread are never visible to another. Private memory is not
involved in snapshot-commit consistency and is not written back at HMERGE.

### 12.3 Section Interaction Rules

| Section | Visible to all threads | Needs atomic section for writes | Written back at HMERGE |
|---------|----------------------|--------------------------------|------------------------|
| `.data` / general CMEM | Thread-local copy | N/A (no sharing) | No (each thread independent) |
| `.shared` | Yes (snapshot outside atomic, live inside) | Yes | Yes (from thread 0) |
| `.private` | No (per-thread copy) | N/A | No |

## 13. Parallel Execution Programming Guide

### 13.1 SPMD Pattern

All threads execute the same instruction stream. The canonical idiom for
data-parallel work:

```
#! threads 4

.shared
base 0x8000
size 4

.code
    HFORK
    ITID  R0              # R0 = this thread's ID (0-3)
    ICCFG R1              # R1 = total threads (4)

    # ... per-thread work using R0 as lane index ...

    HMERGE
    HALT
```

### 13.2 Atomic Section Pattern

Use HATMS/HATME to perform serialized reductions into shared memory:

```
    HFORK
    ITID  R0
    # ... compute per-thread result in R2 ...

    HATMS                 # all threads rendezvous; leader elected
    JMPF AF, do_update    # only leader takes this branch
    JMP after_update

LABEL: do_update
    ILDM R3, @shared_counter    # R3 = current shared value (live data)
    IADD R3, R3, R2             # accumulate
    ISTR R3, @shared_counter
LABEL: after_update

    HATME                 # leader commits; all threads resume

    HMERGE
    HALT
```

After HMERGE, `@shared_counter` in thread 0's CMEM holds the accumulated
result from the leader's last committed atomic section.

### 13.3 Quantum State in Parallel Regions

Quantum registers are moved to a `SharedQuantumFile` at HFORK and restored at
HMERGE. Within the parallel region:
- Access to Q[k] acquires the per-register mutex; concurrent access to
  different register indices is allowed.
- Quantum operations (QKERNEL, QOBSERVE, etc.) are permitted in the parallel
  region but NOT inside HATMS/HATME atomic sections.
- All Q registers must be consumed (QF=0) before HFORK and before HMERGE.

### 13.4 CLI Thread Override

The `--threads <n>` flag on `cqam-run` overrides the pragma value:

```
cqam-run myprogram.cqam --threads 8
```

Valid range: 1-256. Specifying `--threads 1` reverts to single-threaded
execution (HFORK/HMERGE become flag-only no-ops, HATMS/HATME become trivial
single-thread atomic sections).
