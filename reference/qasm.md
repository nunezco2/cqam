# CQAM QASM Generation Semantics

## 1. Overview

The `cqam-codegen` crate translates CQAM instruction sequences into OpenQASM 3.0
output. The emission follows a three-phase pipeline:

1. **SCAN** -- Walk all instructions, collect used register indices and metadata.
2. **DECLARE** -- Emit one type declaration per used register (standalone mode only).
3. **EMIT** -- Translate each instruction to QASM body lines.

The entry point is `emit_qasm_program()` in `cqam-codegen/src/qasm.rs`.

## 2. Emit Modes

| Mode | Description |
|------|-------------|
| Standalone | Full program: OPENQASM header, includes, declarations, kernel gate definitions, body, footer. |
| Fragment | Body only: no header, no includes, no declarations. Suitable for embedding in a larger QASM program. |

## 3. Register Declaration Mapping

In standalone mode, the emitter declares only registers that appear in the program:

| CQAM Register | QASM Declaration |
|---------------|-----------------|
| R{n} | `int[64] R{n};` |
| F{n} | `float[64] F{n};` |
| Z{n} | `float[64] Z{n}_re;` and `float[64] Z{n}_im;` |
| Q{n} | `qubit[16] q{n};` |
| H{n} | `bit[16] H{n};` |
| CMEM (if used) | `array[int[64], 65536] CMEM;` |

Complex registers are lowered to paired floats (real and imaginary parts).

## 4. Instruction Translation

### 4.1 Integer Arithmetic

| CQAM | QASM |
|------|------|
| `IADD R2, R0, R1` | `R2 = R0 + R1;` |
| `ISUB R3, R1, R0` | `R3 = R1 - R0;` |
| `IMUL R4, R0, R1` | `R4 = R0 * R1;` |
| `IDIV R4, R2, R1` | `R4 = R2 / R1;` |
| `IMOD R5, R1, R0` | `R5 = R1 % R0;` |

### 4.2 Integer Bitwise

| CQAM | QASM |
|------|------|
| `IAND R2, R0, R1` | `R2 = R0 & R1;` |
| `IOR R2, R0, R1` | `R2 = R0 \| R1;` |
| `IXOR R2, R0, R1` | `R2 = R0 ^ R1;` |
| `INOT R2, R0` | `R2 = ~R0;` |
| `ISHL R2, R0, 4` | `R2 = R0 << 4;` |
| `ISHR R2, R0, 4` | `R2 = R0 >> 4;` |

### 4.3 Integer Memory

| CQAM | QASM |
|------|------|
| `ILDI R0, 42` | `R0 = 42;` |
| `ILDM R0, 100` | `R0 = CMEM[100];` |
| `ISTR R0, 100` | `CMEM[100] = R0;` |

### 4.4 Integer Comparison

| CQAM | QASM |
|------|------|
| `IEQ R3, R0, R1` | `R3 = (R0 == R1) ? 1 : 0;` |
| `ILT R3, R0, R1` | `R3 = (R0 < R1) ? 1 : 0;` |
| `IGT R3, R0, R1` | `R3 = (R0 > R1) ? 1 : 0;` |

### 4.5 Float Arithmetic

Same pattern as integer, using F-prefixed registers. Float comparison results
are written to integer registers (since the result is boolean 0 or 1).

### 4.6 Complex Arithmetic

Complex operations are lowered to paired float operations:

| CQAM | QASM |
|------|------|
| `ZADD Z2, Z0, Z1` | `Z2_re = Z0_re + Z1_re;` and `Z2_im = Z0_im + Z1_im;` |
| `ZMUL Z2, Z0, Z1` | Standard (a+bi)(c+di) expansion |
| `ZLDI Z0, 3, -2` | `Z0_re = 3.0;` and `Z0_im = -2.0;` |

### 4.7 Type Conversion

| CQAM | QASM |
|------|------|
| `CVTIF F0, R0` | `F0 = float[64](R0);` |
| `CVTFI R0, F0` | `R0 = int[64](F0);` |
| `CVTFZ Z0, F0` | `Z0_re = F0;` and `Z0_im = 0.0;` |
| `CVTZF F0, Z0` | `F0 = Z0_re;` |

### 4.8 Control Flow

| CQAM | QASM |
|------|------|
| `JMP target` | `goto target;` |
| `JIF R0, target` | `if (R0 != 0) goto target;` |
| `CALL target` | `// CALL target [no QASM equivalent]` |
| `RET` | `// RET [no QASM equivalent]` |
| `HALT` | `// HALT` |
| `LABEL: name` | `name:` |

### 4.9 Quantum Operations

| CQAM | QASM |
|------|------|
| `QPREP Q0, 0` | `reset q0;` followed by distribution comment |
| `QKERNEL Q1, Q0, 2, R0, R1` | Kernel header comment + gate call or expanded template |
| `QOBSERVE H0, Q1, 0, R0, R0` | `H0 = measure q1;` (mode=DIST). PROB and AMP modes emit annotation comments. |
| `QLOAD Q0, 10` | Comment (no QASM equivalent) |
| `QSTORE Q0, 10` | Comment (no QASM equivalent) |
| `QSAMPLE H0, Q0, 0, R0, R0` | `// @cqam.qsample: H0 = sample(q0, dist)` (no QASM equivalent) |
| `QKERNELF Q1, Q0, 5, F0, F1` | Kernel header comment + gate call (same as QKERNEL but params from F-file) |
| `QKERNELZ Q1, Q0, 6, Z0, Z1` | Kernel header comment + gate call (params from Z-file) |
| `QPREPR Q0, R0` | `reset q0;` followed by distribution comment (same as QPREP, dist ID from register) |
| `QENCODE Q0, F0, 4, 1` | `// @cqam.qencode: q0 = encode(F0..F3, file=F)` (no QASM equivalent) |
| `QHADM Q1, Q0, R2` | `// @cqam.qhadm: apply H to q0 masked by R2, result in q1` |
| `QFLIP Q1, Q0, R2` | `// @cqam.qflip: apply X to q0 masked by R2, result in q1` |
| `QPHASE Q1, Q0, R2` | `// @cqam.qphase: apply Z to q0 masked by R2, result in q1` |

### 4.10 Hybrid Operations

All hybrid operations emit CQAM-specific annotation comments:

| CQAM | QASM |
|------|------|
| `HFORK` | `// @cqam.hfork: begin parallel execution region` |
| `HMERGE` | `// @cqam.hmerge: end parallel execution region, merge results` |
| `HCEXEC 3, target` | `// @cqam.hcexec: if PSW.PF goto target` |
| `HREDUCE H0, R2, 11` | `// @cqam.hreduce: R2 = mode(H0)` |

## 5. Kernel Template Expansion

When `EmitConfig.expand_templates` is true, QKERNEL instructions inline the
content of the corresponding template file from the template directory.

### 5.1 Template Resolution

Templates are loaded from `{template_dir}/{kernel_name}.qasm` where
`kernel_name` is returned by `kernel_name(kernel_id)`.

### 5.2 Variable Substitution

| Placeholder | Substitution | Description |
|-------------|-------------|-------------|
| `{{DST}}` | `q{dst}` | Destination quantum register |
| `{{SRC}}` | `q{src}` | Source quantum register |
| `{{PARAM0}}` | `R{ctx0}` | First classical context register |
| `{{PARAM1}}` | `R{ctx1}` | Second classical context register |

### 5.3 Template Files

| Kernel | File | Description |
|--------|------|-------------|
| init (ID 0) | `init.qasm` | Hadamard gates for uniform superposition |
| entangle (ID 1) | `entangle.qasm` | CNOT cascade for GHZ-like entanglement |
| fourier (ID 2) | `fourier.qasm` | QFT circuit with controlled-phase gates |
| diffuse (ID 3) | `diffuse.qasm` | Grover diffusion (inversion about the mean) |
| grover_iter (ID 4) | `grover_iter.qasm` | Oracle phase-flip + diffusion |
| rotate (ID 5) | `rotate.qasm` | Diagonal rotation gate |
| phase_shift (ID 6) | `phase_shift.qasm` | Phase shift gate |

Note: Template files for rotate and phase_shift are optional extensions.
When no template is found, the emitter generates a stub or annotation comment.

## 6. Kernel Gate Stubs

When `expand_templates` is false and mode is Standalone, the emitter generates
gate definition stubs for each referenced kernel:

```qasm
gate init q {
    // init kernel logic
}
```

These stubs are omitted in Fragment mode or when template expansion is enabled.
