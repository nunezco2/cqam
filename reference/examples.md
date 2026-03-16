# CQAM Instruction Reference Examples

This document provides examples and explanations for each instruction type
in the CQAM architecture using the current flat-prefix ISA syntax.

## Syntax

All instructions use space-delimited operands with comma separation:

```
MNEMONIC dst, src1, src2
```

Register prefixes indicate the register file:
- `R0`-`R15` -- integer registers
- `F0`-`F15` -- float registers
- `Z0`-`Z15` -- complex registers
- `Q0`-`Q7` -- quantum registers
- `H0`-`H7` -- hybrid registers

Comments use `#` or `//`. Blank lines are ignored.

## Pragma Directives

Programs may include a qubit hint on the first line using the `#!` pragma
syntax. This sets the default qubit count for QPREP and related instructions
in that program, overriding the config file but not the `--qubits` CLI flag:

```
#! qubits 4
QPREP Q0, 0          # Prepares a 4-qubit uniform state
```

## Integer Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| ILDI | `ILDI R0, 42` | Load immediate: R0 = 42 |
| ILDM | `ILDM R1, 100` | Load from memory: R1 = CMEM[100] |
| ISTR | `ISTR R0, 200` | Store to memory: CMEM[200] = R0 |
| ILDX | `ILDX R1, R0` | Indirect load: R1 = CMEM[R0] |
| ISTRX | `ISTRX R0, R1` | Indirect store: CMEM[R1] = R0 |
| IADD | `IADD R2, R0, R1` | Integer add: R2 = R0 + R1 |
| ISUB | `ISUB R3, R1, R0` | Integer subtract: R3 = R1 - R0 |
| IMUL | `IMUL R4, R0, R1` | Integer multiply: R4 = R0 * R1 |
| IDIV | `IDIV R5, R4, R1` | Integer divide: R5 = R4 / R1 |
| IMOD | `IMOD R6, R1, R0` | Integer modulo: R6 = R1 % R0 |
| IAND | `IAND R2, R0, R1` | Bitwise AND: R2 = R0 & R1 |
| IOR | `IOR R2, R0, R1` | Bitwise OR: R2 = R0 \| R1 |
| IXOR | `IXOR R2, R0, R1` | Bitwise XOR: R2 = R0 ^ R1 |
| INOT | `INOT R2, R0` | Bitwise NOT: R2 = ~R0 |
| ISHL | `ISHL R2, R0, 4` | Shift left: R2 = R0 << 4 |
| ISHR | `ISHR R2, R0, 4` | Shift right: R2 = R0 >> 4 |
| IEQ | `IEQ R3, R0, R1` | Equality: R3 = (R0 == R1) ? 1 : 0 |
| ILT | `ILT R3, R0, R1` | Less than: R3 = (R0 < R1) ? 1 : 0 |
| IGT | `IGT R3, R0, R1` | Greater than: R3 = (R0 > R1) ? 1 : 0 |

## Float Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| FLDI | `FLDI F0, 7` | Load immediate: F0 = 7.0 |
| FLDM | `FLDM F1, 100` | Load from memory: F1 = f64::from_bits(CMEM[100]) |
| FSTR | `FSTR F0, 200` | Store to memory: CMEM[200] = F0.to_bits() |
| FLDX | `FLDX F1, R0` | Indirect load: F1 = f64::from_bits(CMEM[R0]) |
| FSTRX | `FSTRX F0, R1` | Indirect store: CMEM[R1] = F0.to_bits() |
| FADD | `FADD F2, F0, F1` | Float add: F2 = F0 + F1 |
| FSUB | `FSUB F3, F1, F0` | Float subtract: F3 = F1 - F0 |
| FMUL | `FMUL F4, F0, F1` | Float multiply: F4 = F0 * F1 |
| FDIV | `FDIV F5, F1, F0` | Float divide: F5 = F1 / F0 |
| FEQ | `FEQ R3, F0, F1` | Float equality (result in R): R3 = (F0 == F1) ? 1 : 0 |
| FLT | `FLT R3, F0, F1` | Float less than (result in R): R3 = (F0 < F1) ? 1 : 0 |
| FGT | `FGT R3, F0, F1` | Float greater than (result in R): R3 = (F0 > F1) ? 1 : 0 |
| FSIN | `FSIN F1, F0` | Sine: F1 = sin(F0) |
| FCOS | `FCOS F1, F0` | Cosine: F1 = cos(F0) |
| FATAN2 | `FATAN2 F2, F0, F1` | Arctangent: F2 = atan2(F0, F1) (F0=y, F1=x) |
| FSQRT | `FSQRT F1, F0` | Square root: F1 = sqrt(F0); traps if F0 < 0 |

### Transcendental function example

    # Compute rotation angle for Ry(pi/3)
    FLDI F0, 1               # numerator = 1
    FLDI F1, 3               # denominator = 3
    FDIV F2, F0, F1          # F2 = 1/3
    FSIN F3, F2              # F3 = sin(1/3) [just an example; typically use FLDM for pi]
    FCOS F4, F2              # F4 = cos(1/3)
    QROT Q1, Q0, R0, 1, F2  # Ry(1/3) on qubit R0

## Complex Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| ZLDI | `ZLDI Z0, 3, -2` | Load complex immediate: Z0 = (3.0, -2.0) |
| ZLDM | `ZLDM Z0, 100` | Load complex from CMEM[100..101] |
| ZSTR | `ZSTR Z0, 200` | Store complex to CMEM[200..201] |
| ZLDX | `ZLDX Z0, R0` | Indirect complex load from CMEM[R0]..+1 |
| ZSTRX | `ZSTRX Z0, R1` | Indirect complex store to CMEM[R1]..+1 |
| ZADD | `ZADD Z2, Z0, Z1` | Complex add |
| ZSUB | `ZSUB Z3, Z1, Z0` | Complex subtract |
| ZMUL | `ZMUL Z4, Z0, Z1` | Complex multiply: (a+bi)(c+di) |
| ZDIV | `ZDIV Z5, Z1, Z0` | Complex divide |

## Type Conversion Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| CVTIF | `CVTIF F0, R0` | Integer to float: F0 = R0 as f64 |
| CVTFI | `CVTFI R0, F0` | Float to integer (truncation): R0 = F0 as i64 |
| CVTFZ | `CVTFZ Z0, F0` | Float to complex: Z0 = (F0, 0.0) |
| CVTZF | `CVTZF F0, Z0` | Complex to float (real part): F0 = Z0.real |

## Control Flow Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| LABEL | `LABEL: my_label` | Define a jump target |
| JMP | `JMP my_label` | Unconditional jump |
| JIF | `JIF R0, my_label` | Conditional jump if R0 != 0 |
| CALL | `CALL subroutine` | Call subroutine (push PC+1, jump) |
| RET | `RET` | Return from subroutine (pop PC) |
| HALT | `HALT` | Terminate execution |

## System / Interrupt Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| NOP | `NOP` | No operation |
| SETIV | `SETIV 0, arith_handler` | Register handler for trap 0 (arithmetic) at label |
| RETI | `RETI` | Return from interrupt, clear maskable traps |

### Interrupt handler example

    # Register arithmetic fault handler before any arithmetic
    SETIV 0, on_arith_fault  # trap_id 0 = arithmetic

    ILDI R0, 10
    ILDI R1, 0
    IDIV R2, R0, R1          # division by zero -> fires trap 0 -> jumps to handler

    JMP done

    LABEL: on_arith_fault
    # Handle arithmetic fault here
    ILDI R2, -1              # store sentinel
    RETI                     # return from handler, execution resumes after IDIV

    LABEL: done
    HALT

## Quantum State Preparation

| Instruction | Example | Description |
|-------------|---------|-------------|
| QPREP | `QPREP Q0, 0` | Prepare Q0 with distribution (0=uniform, 1=zero, 2=bell, 3=ghz) |
| QPREPR | `QPREPR Q0, R0` | Prepare with dist ID from R0 at runtime |
| QPREPN | `QPREPN Q0, 0, R1` | Prepare uniform state with R1 qubits |
| QENCODE | `QENCODE Q0, F0, 4, 1` | Encode F0..F3 (4 float regs) as quantum amplitudes |
| QMIXED | `QMIXED Q0, R5, R6` | Build mixed state from CMEM[R5], R6 entries |

### QPREPN example

    ILDI R1, 4               # num_qubits = 4
    QPREPN Q0, 0, R1         # Q0 = uniform 4-qubit state (16-dimensional)

### QMIXED example

    # Prepare a 50/50 mixture of |0> and |+> (2-qubit, 1-qubit illustrative)
    # See reference/opcodes.md section 12 for CMEM layout details
    QMIXED Q0, R5, R6        # Q0 = sum_i w_i |psi_i><psi_i|

## Quantum Kernel Operations

QKERNEL uses mnemonic-first operand order: `QKERNEL MNEM, Q_dst, Q_src, R_ctx0, R_ctx1`.

| Instruction | Example | Description |
|-------------|---------|-------------|
| QKERNEL | `QKERNEL QFFT, Q1, Q0, R0, R1` | Apply QFT kernel to Q0, context from R0/R1 |
| QKERNELF | `QKERNELF DROT, Q1, Q0, F0, F1` | Apply diagonal rotation with float params F0, F1 |
| QKERNELZ | `QKERNELZ PHSH, Q1, Q0, Z0, Z1` | Apply phase-shift with complex params Z0, Z1 |

Four-letter kernel mnemonics: UNIT (init), ENTG (entangle), QFFT (fourier),
DIFF (diffuse), GROV (grover_iter), DROT (rotate), PHSH (phase_shift),
QIFT (fourier_inv), CTLU (controlled_u), DIAG (diagonal_unitary), PERM (permutation).

### CTLU example (controlled_u, kernel 8)

    # Controlled rotation: control qubit 0, sub-kernel=ROTATE (DROT)
    # Parameter block at CMEM[200..203]:
    #   [200]=5 (sub_kernel_id for DROT), [201]=0 (power k; applies C-U^{2^k}),
    #   [202]=theta_re bits,              [203]=theta_im bits
    ILDI R8, 0               # control qubit index
    ILDI R9, 200             # CMEM base of parameter block
    QKERNEL CTLU, Q1, Q0, R8, R9

### DIAG example (diagonal_unitary, kernel 9)

    # Apply diagonal phases from .c64 data declared in .data section
    # diag: .c64 1.0J0.0, -1.0J0.0, 1.0J0.0, 1.0J0.0
    ILDI R0, @diag           # CMEM base of diagonal entries
    ILDI R1, @diag.len       # number of complex entries (= dimension)
    QKERNEL DIAG, Q1, Q0, R0, R1

### PERM example (permutation, kernel 10)

    # Apply cyclic permutation sigma = [1, 2, 3, 0] from CMEM[100..103]
    ILDI R0, 100             # CMEM base of permutation table
    ILDI R1, 0               # unused (dimension inferred from register size)
    QKERNEL PERM, Q1, Q0, R0, R1

## Qubit-Level Gate Operations

| Instruction | Example | Description |
|-------------|---------|-------------|
| QCNOT | `QCNOT Q1, Q0, R0, R1` | CNOT: ctrl=R0, tgt=R1, applied within Q0 |
| QCZ | `QCZ Q1, Q0, R0, R1` | Controlled-Z: ctrl=R0, tgt=R1 |
| QSWAP | `QSWAP Q1, Q0, R0, R1` | SWAP qubits R0 and R1 within Q0 |
| QROT | `QROT Q1, Q0, R0, 1, F0` | Ry rotation (axis=1) by F0 radians on qubit R0 |
| QHADM | `QHADM Q1, Q0, R2` | Hadamard on qubits selected by bitmask R2 |
| QFLIP | `QFLIP Q1, Q0, R2` | Pauli-X on qubits selected by bitmask R2 |
| QPHASE | `QPHASE Q1, Q0, R2` | Pauli-Z on qubits selected by bitmask R2 |
| QCUSTOM | `QCUSTOM Q1, Q0, R3, R4` | Custom unitary U from CMEM[R3], dim=R4 |

### QCNOT example

    QPREP Q0, 1              # |00> state (2 qubits)
    ILDI R0, 0               # ctrl = qubit 0
    ILDI R1, 1               # tgt  = qubit 1
    QHADM Q1, Q0, R0         # Hadamard on qubit 0 -> (|00>+|10>)/sqrt(2)
    QCNOT Q2, Q1, R0, R1     # CNOT -> Bell state (|00>+|11>)/sqrt(2)

### QROT example (Rx rotation)

    QPREP Q0, 1              # |0> state (1 qubit)
    ILDI R0, 0               # target qubit = 0
    FLDI F0, 1               # angle = 1.0 radian
    QROT Q1, Q0, R0, 0, F0  # Rx(1.0) on qubit 0

### QSWAP example

    ILDI R0, 0               # qubit a = 0
    ILDI R1, 1               # qubit b = 1
    QSWAP Q1, Q0, R0, R1    # swap qubits 0 and 1 in Q0

## Measurement Instructions

All observation in CQAM is destructive or partial. Non-destructive observation
has no physical basis and is not supported by the ISA. Use QOBSERVE for full
destructive measurement or QMEAS for partial single-qubit measurement.

| Instruction | Example | Description |
|-------------|---------|-------------|
| QOBSERVE | `QOBSERVE H0, Q1, 0, R0, R0` | Measure Q1 into H0 (destructive, mode=DIST); Q1 consumed |
| QMEAS | `QMEAS R3, Q0, R0` | Measure qubit R0 of Q0; outcome (0 or 1) -> R3; Q0 updated (not consumed) |

### QMEAS example

    QPREP Q0, 0              # uniform 2-qubit superposition
    ILDI R0, 0               # measure qubit 0
    QMEAS R1, Q0, R0         # R1 = 0 or 1; Q0 = post-measurement state
    ILDI R0, 1               # now measure qubit 1
    QMEAS R2, Q0, R0         # R2 = 0 or 1

## Observation Mode Examples

QOBSERVE supports three modes for extracting classical information from a
quantum register. All modes are destructive: Q[src] is consumed.

### Mode 0 (DIST): Full distribution (default)

    QPREP Q0, 0              # Uniform 2-qubit superposition
    QOBSERVE H0, Q0, 0, R0, R0   # H0 = Dist([(0,0.25),(1,0.25),(2,0.25),(3,0.25)]); Q0 consumed

ctx0 and ctx1 are ignored in DIST mode.

### Mode 1 (PROB): Single basis-state probability

    QPREP Q0, 2              # Bell state (fresh copy needed per query, since QOBSERVE is destructive)
    ILDI R0, 0               # Query basis state |0>
    QOBSERVE H0, Q0, 1, R0, R0   # H0 = Float(0.5); Q0 consumed

ctx0 selects the basis state index; ctx1 is ignored. Each PROB query
consumes the register; prepare a fresh state for each distinct query.

### Mode 2 (AMP): Density matrix element

    QPREP Q0, 2              # Bell state (fresh copy needed per query)
    ILDI R0, 0               # row = 0
    ILDI R1, 3               # col = 3 (|11> = state 3)
    QOBSERVE H0, Q0, 2, R0, R1   # H0 = Complex(0.5, 0.0); Q0 consumed

ctx0 selects the row; ctx1 selects the column. Prepare a fresh state for
each density matrix element query.

## Quantum Memory Instructions

QSTORE and QLOAD implement quantum teleportation. Each operation consumes one
Bell pair from the VM's Bell pair budget (default 256). The source is destroyed
in each case: the state exists in exactly one location at all times, consistent
with the no-cloning theorem. A QSTORE followed by a QLOAD is a move round-trip,
not a copy. When a noise model is active, each operation applies a depolarizing
channel proportional to `(1 - bell_pair_fidelity)` on the transferred state.

| Instruction | Example | Description |
|-------------|---------|-------------|
| QSTORE | `QSTORE Q0, 10` | Teleport Q0 into QMEM[10]; Q0 is consumed; costs one Bell pair |
| QLOAD | `QLOAD Q0, 10` | Teleport QMEM[10] into Q0; QMEM[10] is emptied; costs one Bell pair |

### QSTORE / QLOAD move example

    QPREP Q0, 0              # prepare 2-qubit uniform superposition in Q0
    QKERNEL QFFT, Q1, Q0, R0, R1  # apply QFT; result in Q1
    QSTORE Q1, 5             # teleport Q1 to QMEM[5]; Q1 is now None
    # ... intervening classical computation ...
    QLOAD Q2, 5              # teleport QMEM[5] back into Q2; QMEM[5] is now empty
    QOBSERVE H0, Q2, 0, R0, R0   # measure Q2; Q2 consumed

One Bell pair is spent on QSTORE and one on QLOAD (two total for the round trip).

## Masked Gate Operations

Masked gates apply single-qubit gates to qubits selected by a classical bitmask:

    QPREP Q0, 1              # |00> state
    ILDI R0, 1               # mask = 0b01 (qubit 0 only)
    QHADM Q1, Q0, R0         # Hadamard on qubit 0 only -> (|0>+|1>)/sqrt(2) tensor |0>

    ILDI R1, 3               # mask = 0b11 (both qubits)
    QFLIP Q2, Q0, R1         # X on both qubits: |00> -> |11>

    QPHASE Q3, Q1, R0        # Z on qubit 0 of the superposition state

## Tensor Product and Partial Trace

### QTENSOR example

    QPREP Q0, 1              # 1-qubit state |0>
    QPREP Q1, 1              # 1-qubit state |0>
    QHADM Q2, Q0, R0         # Q2 = |+> (mask qubit 0)
    QTENSOR Q3, Q2, Q1       # Q3 = |+> tensor |0> = (|00>+|10>)/sqrt(2); Q2, Q1 consumed

### QPTRACE example

    QPREP Q0, 2              # Bell state (2 qubits)
    ILDI R0, 1               # num_qubits_a = 1 (keep subsystem A)
    QPTRACE Q1, Q0, R0      # Q1 = Tr_B(Bell state) = I/2 (maximally mixed 1-qubit state)
    # Q0 is NOT consumed; Q1 is always Mixed (DensityMatrix)

### QRESET example

    QPREP Q0, 0              # Uniform 2-qubit superposition
    ILDI R0, 1               # target qubit = 1
    QRESET Q1, Q0, R0       # Q1 = state with qubit 1 guaranteed |0>

## Classical-to-Quantum Amplitude Encoding

QENCODE reads consecutive registers from a selected file and constructs a
normalized quantum state:

    # Encode 4 float registers as a 2-qubit state
    FLDI F0, 1               # amplitude for |00>
    FLDI F1, 0               # amplitude for |01>
    FLDI F2, 0               # amplitude for |10>
    FLDI F3, 1               # amplitude for |11>
    QENCODE Q0, F0, 4, 1    # Q0 = normalized |psi> from F0..F3, file_sel=1 (F-file)
    # Q0 is now (|00> + |11>) / sqrt(2)

    # Encode from complex registers for phase control
    ZLDI Z0, 1, 0            # (1.0, 0.0)
    ZLDI Z1, 0, 1            # (0.0, 1.0) = i
    QENCODE Q1, Z0, 2, 2    # Q1 = normalized from Z0..Z1, file_sel=2 (Z-file)
    # Q1 encodes a state with a relative phase of pi/2

## Register-Parameterized Preparation

QPREPR reads the distribution ID from an integer register at runtime:

    ILDI R0, 2               # dist_id = BELL
    QPREPR Q0, R0            # Equivalent to QPREP Q0, 2

This enables data-driven state preparation in loops:

    ILDI R5, 0               # loop counter
    ILDI R6, 4               # limit (4 distributions)
    ILDI R3, 1               # increment
    LABEL: prep_loop
    QPREPR Q0, R5            # Prepare with dist_id = R5
    QOBSERVE H0, Q0, 0, R0, R0  # Measure and consume Q0
    IADD R5, R5, R3          # R5 += 1
    ILT R7, R5, R6           # R7 = (R5 < 4)
    JIF R7, prep_loop        # re-prepare fresh state each iteration

## Hybrid Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| HFORK | `HFORK` | Fork hybrid execution (set fork flags) |
| HMERGE | `HMERGE` | Merge hybrid branches (set merge flags) |
| JMPF | `JMPF EF, label` | Conditional jump if PSW flag EF is set |
| HREDUCE | `HREDUCE MODEV, H0, R2` | Reduce H0 using MODEV (mode), store result in R2 |

HREDUCE uses mnemonic-first operand order: `HREDUCE MNEM, H_src, R/F/Z_dst`.
Five-letter reduction mnemonics: ROUND, FLOOR, CEILI, TRUNC, ABSOL, NEGAT,
MAGNI, PHASE, REALP, IMAGP, MEANT, MODEV, ARGMX, VARNC, CONJZ, NEGTZ, EXPCT.

### Expectation value example (EXPCT)

The `EXPCT` function computes sum_k eigenvalue_k * p_k, reading n eigenvalues
from CMEM starting at R[ctx]. Usage: `HREDUCE EXPCT, H_src, F_dst`. The
instruction passes R[ctx] as context via the standard HR encoding
(see reference/opcodes.md).

## Data Section

The `.data` section allows declaring initialized CMEM contents at assembly time.
Labels in `.data` are referenced in `.code` via `@label` (base address) and
`@label.len` (logical entry count).

    .data
        .org 200
    diag:
        .c64 1.0J0.0, -1.0J0.0,
             1.0J0.0,  1.0J0.0

        .org 1000
    msg:
        .ascii "Result = %d\n"

    .code
        ILDI R0, @diag         # R0 = 200 (base address)
        ILDI R1, @diag.len     # R1 = 4 (complex entry count)
        QKERNEL DIAG, Q1, Q0, R0, R1

### .c64 complex literal format

    .c64 1.0J0.0               # 1 + 0i
    .c64 -1.5J2.5              # -1.5 + 2.5i
    .c64 1.5e-3J-2.0e1         # scientific notation
    .c64 0J1.0                 # pure imaginary

A trailing comma continues on the next line:

    .c64 1.0J0.0,  1.0J0.0,
         1.0J0.0, -1.0J0.0

Each entry occupies two consecutive CMEM cells: real part at `base + 2k`,
imaginary part at `base + 2k + 1`, both stored as `f64::to_bits() as i64`.

---
For more details, see `cqam-core/src/instruction.rs` and `reference/opcodes.md`.
