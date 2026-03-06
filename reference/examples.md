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

## Integer Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| ILDI | `ILDI R0, 42` | Load immediate: R0 = 42 |
| ILDM | `ILDM R1, 100` | Load from memory: R1 = CMEM[100] |
| ISTR | `ISTR R0, 200` | Store to memory: CMEM[200] = R0 |
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
| FADD | `FADD F2, F0, F1` | Float add: F2 = F0 + F1 |
| FSUB | `FSUB F3, F1, F0` | Float subtract: F3 = F1 - F0 |
| FMUL | `FMUL F4, F0, F1` | Float multiply: F4 = F0 * F1 |
| FDIV | `FDIV F5, F1, F0` | Float divide: F5 = F1 / F0 |
| FEQ | `FEQ R3, F0, F1` | Float equality (result in R): R3 = (F0 == F1) ? 1 : 0 |
| FLT | `FLT R3, F0, F1` | Float less than (result in R): R3 = (F0 < F1) ? 1 : 0 |
| FGT | `FGT R3, F0, F1` | Float greater than (result in R): R3 = (F0 > F1) ? 1 : 0 |

## Complex Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| ZLDI | `ZLDI Z0, 3, -2` | Load complex immediate: Z0 = (3.0, -2.0) |
| ZLDM | `ZLDM Z0, 100` | Load complex from CMEM[100..101] |
| ZSTR | `ZSTR Z0, 200` | Store complex to CMEM[200..201] |
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

## Quantum Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| QPREP | `QPREP Q0, 0` | Prepare Q0 with distribution (0=uniform, 1=zero, 2=bell, 3=ghz) |
| QKERNEL | `QKERNEL Q1, Q0, 2, R0, R1` | Apply kernel 2 (fourier) to Q0, result in Q1, context R0/R1 |
| QOBSERVE | `QOBSERVE H0, Q1, 0, R0, R0` | Measure Q1 into H0 (mode=DIST, full distribution) |
| QLOAD | `QLOAD Q0, 10` | Load Q0 from QMEM[10] |
| QSTORE | `QSTORE Q0, 10` | Store Q0 to QMEM[10] |
| QSAMPLE  | `QSAMPLE H1, Q0, 0, R0, R0` | Non-destructive sample of Q0 distribution into H1 |
| QKERNELF | `QKERNELF Q1, Q0, 5, F0, F1` | Apply kernel 5 (rotate) with float params F0, F1 |
| QKERNELZ | `QKERNELZ Q1, Q0, 6, Z0, Z1` | Apply kernel 6 (phase_shift) with complex params Z0, Z1 |
| QPREPR   | `QPREPR Q0, R0`               | Prepare Q0 with dist ID from R0 (R0=0 -> uniform) |
| QENCODE  | `QENCODE Q0, F0, 4, 1`        | Encode F0..F3 (4 float regs) as quantum amplitudes |
| QHADM    | `QHADM Q1, Q0, R2`            | Apply Hadamard to qubits selected by bitmask R2 |
| QFLIP    | `QFLIP Q1, Q0, R2`            | Apply X (bit-flip) to qubits selected by bitmask R2 |
| QPHASE   | `QPHASE Q1, Q0, R2`           | Apply Z (phase-flip) to qubits selected by bitmask R2 |

## Hybrid Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| HFORK | `HFORK` | Fork hybrid execution (set fork flags) |
| HMERGE | `HMERGE` | Merge hybrid branches (set merge flags) |
| HCEXEC | `HCEXEC 3, label` | Conditional jump if PSW flag 3 (PF) is set |
| HREDUCE | `HREDUCE H0, R2, 11` | Reduce H0 using func 11 (mode), store in R2. 16 reduction functions available. |

## System Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| NOP | `NOP` | No operation |
| HALT | `HALT` | Terminate program execution |

## QOBSERVE and QSAMPLE Mode Examples

The observation instructions support three modes via the third operand:

### Mode 0 (DIST): Full distribution (default)

    QPREP Q0, 0              # Uniform distribution
    QOBSERVE H0, Q0, 0, R0, R0   # H0 = Dist([(0,0.25),(1,0.25),(2,0.25),(3,0.25)])

ctx0 and ctx1 are ignored in DIST mode.

### Mode 1 (PROB): Single probability

    QPREP Q0, 2              # Bell state
    ILDI R0, 0                # Query basis state |0>
    QSAMPLE H0, Q0, 1, R0, R0    # H0 = Float(0.5)

ctx0 selects the basis state index; ctx1 is ignored.

### Mode 2 (AMP): Density matrix element

    QPREP Q0, 2              # Bell state
    ILDI R0, 0                # row = 0
    ILDI R1, 3                # col = 3 (|11> = state 3)
    QSAMPLE H0, Q0, 2, R0, R1    # H0 = Complex(0.5, 0.0)

ctx0 selects the row; ctx1 selects the column.

## Masked Gate Operations

Masked gates apply single-qubit gates to qubits selected by a classical bitmask:

    QPREP Q0, 1              # |00> state
    ILDI R0, 1                # mask = 0b01 (qubit 0 only)
    QHADM Q1, Q0, R0         # Hadamard on qubit 0 only -> (|0>+|1>)/sqrt(2) x |0>

    ILDI R1, 3                # mask = 0b11 (both qubits)
    QFLIP Q2, Q0, R1          # X on both qubits: |00> -> |11>

    QPHASE Q3, Q1, R0         # Z on qubit 0 of the superposition state

## Classical-to-Quantum Amplitude Encoding

QENCODE reads consecutive registers from a selected file and constructs a
normalized quantum state:

    # Encode 4 float registers as a 2-qubit state
    FLDI F0, 1                # amplitude for |00>
    FLDI F1, 0                # amplitude for |01>
    FLDI F2, 0                # amplitude for |10>
    FLDI F3, 1                # amplitude for |11>
    QENCODE Q0, F0, 4, 1     # Q0 = normalized |psi> from F0..F3, file_sel=1 (F-file)
    # Q0 is now (|00> + |11>) / sqrt(2)

    # Encode from complex registers for phase control
    ZLDI Z0, 1, 0             # (1.0, 0.0)
    ZLDI Z1, 0, 1             # (0.0, 1.0) = i
    QENCODE Q1, Z0, 2, 2     # Q1 = normalized from Z0..Z1, file_sel=2 (Z-file)
    # Q1 encodes a state with a relative phase of pi/2

## Register-Parameterized Preparation

QPREPR reads the distribution ID from an integer register at runtime:

    ILDI R0, 2                # dist_id = BELL
    QPREPR Q0, R0             # Equivalent to QPREP Q0, 2

This enables data-driven state preparation in loops:

    ILDI R5, 0                # loop counter
    ILDI R6, 4                # limit (4 distributions)
    LABEL: prep_loop
    QPREPR Q0, R5             # Prepare with dist_id = R5
    QSAMPLE H0, Q0, 0, R0, R0   # Sample without destroying
    IADD R5, R5, R3           # R5 += 1 (R3 = 1)
    ILT R7, R5, R6            # R7 = (R5 < 4)
    JIF R7, prep_loop

---
For more details, see `cqam-core/src/instruction.rs` and `reference/opcodes.md`.
