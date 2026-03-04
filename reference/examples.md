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
| QOBSERVE | `QOBSERVE H0, Q1` | Measure Q1 into hybrid register H0 |
| QLOAD | `QLOAD Q0, 10` | Load Q0 from QMEM[10] |
| QSTORE | `QSTORE Q0, 10` | Store Q0 to QMEM[10] |

## Hybrid Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| HFORK | `HFORK` | Fork hybrid execution (set fork flags) |
| HMERGE | `HMERGE` | Merge hybrid branches (set merge flags) |
| HCEXEC | `HCEXEC 3, label` | Conditional jump if PSW flag 3 (PF) is set |
| HREDUCE | `HREDUCE H0, R2, 11` | Reduce H0 using func 11 (mode), store in R2 |

## System Instructions

| Instruction | Example | Description |
|-------------|---------|-------------|
| NOP | `NOP` | No operation |
| HALT | `HALT` | Terminate program execution |

---
For more details, see `cqam-core/src/instruction.rs` and `reference/opcodes.md`.
