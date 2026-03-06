# CQAM ISA Reference Card

This document is the authoritative quick-reference for the CQAM instruction
set architecture. It is derived directly from `cqam-core/src/instruction.rs`
and `cqam-core/src/opcode.rs`. For the bit-level encoding of every format see
`reference/opcodes.md`.

---

## 1. Instruction Set Summary

Every instruction encodes into a fixed-width 32-bit word. The top 8 bits are
the opcode; the remaining 24 bits carry operands according to one of the
encoding formats listed in section 3.

All register operands are numeric indices. The register file prefix (R, F, Z,
Q, H) is determined by the instruction mnemonic, not by the operand encoding.
In text-format source, register names include the prefix (e.g. `R0`, `F3`,
`Q1`).

### 1.1 No-operation and pseudo-instructions

| Mnemonic | Operands | Description | Encoding |
|----------|----------|-------------|----------|
| `NOP` | — | No operation. No side effects. | N |
| `LABEL:` name | name | Label definition (text source only). Sets a jump target; occupies one word in unstripped binary. | L |

### 1.2 Integer arithmetic (R-file: 16 x i64)

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `IADD` | dst, lhs, rhs | R[dst] = R[lhs] + R[rhs] (wrapping) | RRR |
| `ISUB` | dst, lhs, rhs | R[dst] = R[lhs] - R[rhs] (wrapping) | RRR |
| `IMUL` | dst, lhs, rhs | R[dst] = R[lhs] * R[rhs] (wrapping) | RRR |
| `IDIV` | dst, lhs, rhs | R[dst] = R[lhs] / R[rhs]; traps if rhs == 0 | RRR |
| `IMOD` | dst, lhs, rhs | R[dst] = R[lhs] % R[rhs]; traps if rhs == 0 | RRR |

### 1.3 Integer bitwise (R-file)

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `IAND` | dst, lhs, rhs | R[dst] = R[lhs] & R[rhs] | RRR |
| `IOR` | dst, lhs, rhs | R[dst] = R[lhs] \| R[rhs] | RRR |
| `IXOR` | dst, lhs, rhs | R[dst] = R[lhs] ^ R[rhs] | RRR |
| `INOT` | dst, src | R[dst] = !R[src] (bitwise NOT) | RR |
| `ISHL` | dst, src, amt | R[dst] = R[src] << amt; amt in 0..63 | RRS |
| `ISHR` | dst, src, amt | R[dst] = R[src] >> amt (arithmetic); amt in 0..63 | RRS |

### 1.4 Integer memory and comparison (R-file)

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `ILDI` | dst, imm16 | R[dst] = sign_extend(imm16) | RI |
| `ILDM` | dst, addr16 | R[dst] = CMEM[addr16] | RA |
| `ISTR` | src, addr16 | CMEM[addr16] = R[src] | RA |
| `ILDX` | dst, addr_reg | R[dst] = CMEM[R[addr_reg]] | RR |
| `ISTRX` | src, addr_reg | CMEM[R[addr_reg]] = R[src] | RR |
| `IEQ` | dst, lhs, rhs | R[dst] = (R[lhs] == R[rhs]) ? 1 : 0 | RRR |
| `ILT` | dst, lhs, rhs | R[dst] = (R[lhs] < R[rhs]) ? 1 : 0 | RRR |
| `IGT` | dst, lhs, rhs | R[dst] = (R[lhs] > R[rhs]) ? 1 : 0 | RRR |

### 1.5 Float arithmetic and memory (F-file: 16 x f64)

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `FADD` | dst, lhs, rhs | F[dst] = F[lhs] + F[rhs] | RRR |
| `FSUB` | dst, lhs, rhs | F[dst] = F[lhs] - F[rhs] | RRR |
| `FMUL` | dst, lhs, rhs | F[dst] = F[lhs] * F[rhs] | RRR |
| `FDIV` | dst, lhs, rhs | F[dst] = F[lhs] / F[rhs] | RRR |
| `FLDI` | dst, imm16 | F[dst] = imm16 as f64 | RI |
| `FLDM` | dst, addr16 | F[dst] = f64::from_bits(CMEM[addr16] as u64) | RA |
| `FSTR` | src, addr16 | CMEM[addr16] = F[src].to_bits() as i64 | RA |
| `FLDX` | dst, addr_reg | F[dst] = f64::from_bits(CMEM[R[addr_reg]]) | RR |
| `FSTRX` | src, addr_reg | CMEM[R[addr_reg]] = F[src].to_bits() | RR |
| `FEQ` | dst, lhs, rhs | **R**[dst] = (F[lhs] == F[rhs]) ? 1 : 0 | RRR |
| `FLT` | dst, lhs, rhs | **R**[dst] = (F[lhs] < F[rhs]) ? 1 : 0 | RRR |
| `FGT` | dst, lhs, rhs | **R**[dst] = (F[lhs] > F[rhs]) ? 1 : 0 | RRR |

Note: float comparison results are written to the **integer** register file.

### 1.6 Complex arithmetic and memory (Z-file: 16 x (f64, f64))

Each Z register holds a complex number (re, im). `ZLdm`/`ZStr` access two
consecutive CMEM cells (addr and addr+1 for re and im respectively).

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `ZADD` | dst, lhs, rhs | Z[dst] = Z[lhs] + Z[rhs] | RRR |
| `ZSUB` | dst, lhs, rhs | Z[dst] = Z[lhs] - Z[rhs] | RRR |
| `ZMUL` | dst, lhs, rhs | Z[dst] = Z[lhs] * Z[rhs] | RRR |
| `ZDIV` | dst, lhs, rhs | Z[dst] = Z[lhs] / Z[rhs]; traps if rhs == 0 | RRR |
| `ZLDI` | dst, re8, im8 | Z[dst] = (re8 as f64, im8 as f64) | ZI |
| `ZLDM` | dst, addr16 | Z[dst] = (CMEM[addr], CMEM[addr+1]) as f64 | RA |
| `ZSTR` | src, addr16 | CMEM[addr] = Z[src].re; CMEM[addr+1] = Z[src].im | RA |
| `ZLDX` | dst, addr_reg | Z[dst] from CMEM[R[addr_reg]] and CMEM[R[addr_reg]+1] | RR |
| `ZSTRX` | src, addr_reg | CMEM[R[addr_reg]] = Z[src].re; CMEM[R[addr_reg]+1] = Z[src].im | RR |

### 1.7 Type conversion

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `CVTIF` | dst_f, src_i | F[dst_f] = R[src_i] as f64 | RR |
| `CVTFI` | dst_i, src_f | R[dst_i] = F[src_f] as i64 (truncation) | RR |
| `CVTFZ` | dst_z, src_f | Z[dst_z] = (F[src_f], 0.0) | RR |
| `CVTZF` | dst_f, src_z | F[dst_f] = Z[src_z].re | RR |

### 1.8 Control flow

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `JMP` | target | PC = address_of(target) | J |
| `JIF` | pred, target | if R[pred] != 0: PC = address_of(target) | JR |
| `CALL` | target | push PC+1; PC = address_of(target) | J |
| `RET` | — | pop call stack; PC = saved address (HALT if empty) | N |
| `HALT` | — | Sets trap_halt in PSW; terminates execution | N |

### 1.9 Quantum operations (Q-file: 8 x DensityMatrix)

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `QPREP` | dst, dist_id | Q[dst] = new quantum state with distribution dist_id | QP |
| `QKERNEL` | dst, src, kernel_id, ctx0, ctx1 | Q[dst] = kernel(Q[src], R[ctx0], R[ctx1]) | Q |
| `QOBSERVE` | dst_h, src_q, mode, ctx0, ctx1 | H[dst_h] = observe(Q[src_q], mode, R[ctx0], R[ctx1]); Q[src_q] = None | QO_EXT |
| `QLOAD` | dst_q, addr8 | Q[dst_q] = QMEM[addr8] (clone) | QS |
| `QSTORE` | src_q, addr8 | QMEM[addr8] = Q[src_q] (clone) | QS |
| `QSAMPLE` | dst_h, src_q, mode, ctx0, ctx1 | H[dst_h] = sample(Q[src_q], mode, R[ctx0], R[ctx1]); non-destructive | QO_EXT |
| `QKERNELF` | dst, src, kernel_id, fctx0, fctx1 | Q[dst] = kernel(Q[src], F[fctx0], F[fctx1]) | Q |
| `QKERNELZ` | dst, src, kernel_id, zctx0, zctx1 | Q[dst] = kernel(Q[src], Z[zctx0], Z[zctx1]) | Q |
| `QPREPR` | dst, dist_reg | Q[dst] = new_qdist(R[dist_reg] as u8) | QR |
| `QENCODE` | dst, src_base, count, file_sel | Q[dst] = from_statevector(regs[src_base..+count]) | QE |
| `QHADM` | dst, src, mask_reg | Apply H to qubits selected by R[mask_reg] bitmask | QMK |
| `QFLIP` | dst, src, mask_reg | Apply X to qubits selected by R[mask_reg] bitmask | QMK |
| `QPHASE` | dst, src, mask_reg | Apply Z to qubits selected by R[mask_reg] bitmask | QMK |

### 1.10 Hybrid operations (H-file: 8 x HybridValue)

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `HFORK` | — | Spawn parallel execution threads; set PSW.forked | N |
| `HMERGE` | — | Join all forked threads; set PSW.merged | N |
| `HCEXEC` | flag_id, target | if PSW.flag[flag_id]: PC = address_of(target) | JR |
| `HREDUCE` | src, dst, func_id | Reduce H[src] to classical value; write to R or F register | HR |

### 1.11 Interrupt handling

| Mnemonic | Operands | Operation | Encoding |
|----------|----------|-----------|----------|
| `SETIV` | trap_id, target | Register target as the ISR handler for trap_id | JR |
| `RETI` | — | Return from interrupt handler; clear maskable trap flags | N |

---

## 2. Register Files

| File | Notation | Count | Element type | Used by |
|------|----------|-------|--------------|---------|
| Integer | R0-R15 | 16 | `i64` | I-prefix, comparison results |
| Float | F0-F15 | 16 | `f64` | F-prefix, CVTIF output |
| Complex | Z0-Z15 | 16 | `(f64, f64)` | Z-prefix |
| Quantum | Q0-Q7 | 8 | `Option<DensityMatrix>` | QPREP, QKERNEL, QKERNELF, QKERNELZ, QPREPR, QENCODE, QOBSERVE, QSAMPLE, QHADM, QFLIP, QPHASE |
| Hybrid | H0-H7 | 8 | `HybridValue` | QOBSERVE output, QSAMPLE output, HREDUCE input |

`HybridValue` is a tagged union: `Empty`, `Int(i64)`, `Float(f64)`,
`Complex(f64, f64)`, or `Dist(Vec<(u16, f64)>)` (measurement outcome).

---

## 3. Instruction Encoding

All instructions are 32-bit little-endian words. The top 8 bits (bits 31-24)
are always the opcode. The remaining 24 bits carry operands according to the
following formats.

### 3.1 Format summary

| Format | Bits 31..24 | Bits 23..0 | Used by |
|--------|-------------|------------|---------|
| **N** | opcode[8] | — (zero) | NOP, RET, HALT, HFORK, HMERGE, RETI |
| **RRR** | opcode[8] | dst[4] lhs[4] rhs[4] _[12] | Integer/float/complex 3-reg ops |
| **RR** | opcode[8] | dst[4] src[4] _[16] | INOT, CVT*, indirect mem |
| **RRS** | opcode[8] | dst[4] src[4] amt[6] _[10] | ISHL, ISHR |
| **RI** | opcode[8] | dst[4] _[4] imm16[16] | ILDI, FLDI |
| **ZI** | opcode[8] | dst[4] _[4] re8[8] im8[8] | ZLDI |
| **RA** | opcode[8] | reg[4] _[4] addr16[16] | ILDM, ISTR, FLDM, FSTR, ZLDM, ZSTR |
| **J** | opcode[8] | addr24[24] | JMP, CALL |
| **JR** | opcode[8] | pred[4] _[4] addr16[16] | JIF, HCEXEC, SETIV |
| **QP** | opcode[8] | dst[3] dist[3] _[18] | QPREP |
| **Q** | opcode[8] | dst[3] src[3] kernel[5] ctx0[4] ctx1[4] _[5] | QKERNEL |
| **QO_EXT** | opcode[8] | dst_h[3] src_q[3] mode[2] ctx0[4] ctx1[4] _[8] | QOBSERVE, QSAMPLE |
| **QS** | opcode[8] | qreg[3] _[5] addr8[8] _[8] | QLOAD, QSTORE |
| **HR** | opcode[8] | src[4] dst[4] func[4] _[12] | HREDUCE |
| **QR** | opcode[8] | dst_q[3] _[1] dist_reg[4] _[16] | QPREPR |
| **QE** | opcode[8] | dst_q[3] _[1] src_base[4] count[4] file_sel[2] _[10] | QENCODE |
| **QMK** | opcode[8] | dst_q[3] src_q[3] _[4] mask_reg[4] _[10] | QHADM, QFLIP, QPHASE |
| **L** | opcode[8] | label_id[16] _[8] | LABEL pseudo-instruction |

### 3.2 Opcode byte assignments

| Range | Domain |
|-------|--------|
| 0x00 | NOP |
| 0x01-0x11 | Integer operations (IADD..IGT) |
| 0x12-0x1B | Float operations (FADD..FGT) |
| 0x1C-0x22 | Complex operations (ZADD..ZSTR) |
| 0x23-0x26 | Type conversions (CVTIF..CVTZF) |
| 0x27-0x2C | Control flow (JMP..LABEL) |
| 0x2D-0x2E | Interrupt handling (RETI, SETIV) |
| 0x30-0x34 | Quantum operations (QPREP..QSTORE) |
| 0x35-0x3E | Register-indirect memory + hybrid operations |
| 0x40-0x4E | Extended quantum operations (QSAMPLE..QSWAP) |
| 0x4F-0x57 | Mixed-state, partial-trace, reset, and float math |

---

## 4. Named Constants

### 4.1 Distribution IDs (`dist_id` module, used by `QPREP`)

| Name | Value | Description |
|------|-------|-------------|
| `UNIFORM` | 0 | Equal probability over all 2^n basis states (H^n\|0>) |
| `ZERO` | 1 | Delta distribution at \|0...0> (computational zero state) |
| `BELL` | 2 | Two-qubit Bell state \|Phi+> = (\|00> + \|11>) / sqrt(2) |
| `GHZ` | 3 | n-qubit GHZ state (\|0...0> + \|1...1>) / sqrt(2) |

### 4.2 Kernel IDs (`kernel_id` module, used by `QKERNEL`)

| Name | Value | Unitary | Description |
|------|-------|---------|-------------|
| `INIT` | 0 | H^n | Uniform superposition (ignores input state) |
| `ENTANGLE` | 1 | CNOT_{0,1} | CNOT gate between qubit 0 (control) and qubit 1 (target) |
| `FOURIER` | 2 | QFT | Quantum Fourier Transform: QFT[j][k] = exp(2pi i jk/N)/sqrt(N) |
| `DIFFUSE` | 3 | D = 2\|s><s\| - I | Grover diffusion operator; D[j][k] = 2/N - delta(j,k) |
| `GROVER_ITER` | 4 | D * O | One Grover iteration: oracle phase-flip at ctx0, then diffusion |
| `ROTATE` | 5 | exp(i*theta*k) | Diagonal rotation; theta from F-file via QKERNELF |
| `PHASE_SHIFT` | 6 | exp(i*|z|*k) | Phase shift; amplitude from Z-file via QKERNELZ |

### 4.3 PSW Flag IDs (`flag_id` module, used by `HCEXEC`)

| Name | Value | PSW field | Description |
|------|-------|-----------|-------------|
| `ZF` | 0 | `psw.zf` | Zero: last arithmetic result was zero |
| `NF` | 1 | `psw.nf` | Negative: last arithmetic result was negative |
| `OF` | 2 | `psw.of` | Overflow (reserved; not fully implemented) |
| `PF` | 3 | `psw.pf` | Predicate: set by comparison instructions |
| `QF` | 4 | `psw.qf` | Quantum active: at least one Q register is occupied |
| `SF` | 5 | `psw.sf` | Superposition present after last QKERNEL |
| `EF` | 6 | `psw.ef` | Entanglement present after last QKERNEL |
| `HF` | 7 | `psw.hf` | Hybrid mode: inside an HFORK/HMERGE block |

### 4.4 Trap IDs (`trap_id` module, used by `SETIV`)

| Name | Value | Type | Description |
|------|-------|------|-------------|
| `ARITHMETIC` | 0 | Maskable | Division by zero or overflow |
| `QUANTUM_ERROR` | 1 | Maskable | Quantum fidelity below threshold |
| `SYNC_FAILURE` | 2 | Maskable | Hybrid synchronisation failure |

Note: `HALT` and `IllegalPC` are non-maskable (NMI) traps and cannot be
overridden with `SETIV`.

### 4.5 Reduction Function IDs (`reduce_fn` module, used by `HREDUCE`)

Output register file depends on the function ID:
- IDs 0-5: result written to integer register file (R).
- IDs 6-13: result written to float register file (F).
- IDs 14-15: result written to complex register file (Z).

| Name | ID | Input | Output | Formula |
|------|----|-------|--------|---------|
| `ROUND` | 0 | Float/Int H value | R | round to nearest integer |
| `FLOOR` | 1 | Float/Int H value | R | floor toward -infinity |
| `CEIL` | 2 | Float/Int H value | R | ceiling toward +infinity |
| `TRUNC` | 3 | Float/Int H value | R | truncate toward zero |
| `ABS` | 4 | Float/Int H value | R | absolute value as integer |
| `NEGATE` | 5 | Float/Int H value | R | negate as integer |
| `MAGNITUDE` | 6 | Complex H value | F | sqrt(re^2 + im^2) |
| `PHASE` | 7 | Complex H value | F | atan2(im, re) |
| `REAL` | 8 | Complex H value | F | real part |
| `IMAG` | 9 | Complex H value | F | imaginary part |
| `MEAN` | 10 | Dist H value | F | sum(x_k * p_k) |
| `MODE` | 11 | Dist H value | R | most probable basis state |
| `ARGMAX` | 12 | Dist H value | R | index of most probable state |
| `VARIANCE` | 13 | Dist H value | F | sum(p_k * (x_k - mean)^2) |
| `CONJ_Z` | 14 | Complex H value | Z | Z[dst] = (re, -im) |
| `NEGATE_Z` | 15 | Complex H value | Z | Z[dst] = (-re, -im) |

### 4.6 Observation Mode IDs (`observe_mode` module)

| Name | Value | Output Type | Description |
|------|-------|-------------|-------------|
| `DIST` | 0 | Dist(Vec<(u16, f64)>) | Full diagonal probability distribution |
| `PROB` | 1 | Float(f64) | Single basis-state probability at index R[ctx0] |
| `AMP` | 2 | Complex(f64, f64) | Density matrix element dm[R[ctx0], R[ctx1]] |

### 4.7 File Selector IDs (`file_sel` module, used by `QENCODE`)

| Name | Value | Register File | Element Type |
|------|-------|---------------|-------------|
| `R_FILE` | 0 | R (integer) | i64 cast to (f64, 0.0) |
| `F_FILE` | 1 | F (float) | f64 as (val, 0.0) |
| `Z_FILE` | 2 | Z (complex) | (f64, f64) used directly |

---

## 5. Memory Map

### 5.1 Classical Memory (CMEM)

| Property | Value |
|----------|-------|
| Size | 65536 cells |
| Cell type | `i64` (64-bit signed integer) |
| Address type | `u16` (0x0000 to 0xFFFF) |
| Initial state | All zero |
| Access instructions | `ILDM`, `ISTR`, `FLDM`, `FSTR`, `ZLDM`, `ZSTR`, `ILDX`, `ISTRX`, `FLDX`, `FSTRX`, `ZLDX`, `ZSTRX` |

Float values are stored bit-for-bit: `f64::to_bits() as i64` on write,
`f64::from_bits(cell as u64)` on read. Complex values occupy two consecutive
cells (real part at addr, imaginary part at addr+1).

### 5.2 Quantum Memory (QMEM)

| Property | Value |
|----------|-------|
| Size | 256 slots |
| Slot type | `Option<DensityMatrix>` |
| Address type | `u8` (0x00 to 0xFF) |
| Initial state | All empty (None) |
| Access instructions | `QLOAD`, `QSTORE` |

QMEM is separate from the quantum register file (Q0-Q7). `QSTORE` clones a
live Q register into a QMEM slot; `QLOAD` clones a QMEM slot into a Q register.

---

## 6. Binary File Format (`.cqb`)

```
Offset  Size   Field
------  ----   -----
0       4      Magic: b"CQAM"
4       2      Version: u16 LE (currently 1)
6       2      Entry point: u16 LE (word offset of first non-label instruction)
8       4      Code length: u32 LE (number of instruction words)
12      N*4    Code: N x u32 LE instruction words
12+N*4  ...    Optional debug section (starts with b"CQDB")
```

The debug section layout:

```
Offset  Size       Field
------  ----       -----
0       4          Debug magic: b"CQDB"
4       2          Entry count: u16 LE
6..     variable   Entries: [id: u16 LE][name_len: u16 LE][name: UTF-8 bytes]
```

See `reference/spec.md` for the full specification.
