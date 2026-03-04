# CQAM Binary Opcode Reference

## 1. Instruction Word Format

All instructions are 32 bits wide (4 bytes). Bit numbering is big-endian:
bits [31:24] contain the 8-bit opcode.

## 2. Encoding Formats

| Format | Layout | Used By |
|--------|--------|---------|
| N | `[opcode:8][_:24]` | NOP, RET, HALT, HFORK, HMERGE |
| RRR | `[opcode:8][dst:4][lhs:4][rhs:4][_:12]` | Arithmetic, comparison |
| RR | `[opcode:8][dst:4][src:4][_:16]` | INOT, CVTxx |
| RRS | `[opcode:8][dst:4][src:4][amt:6][_:10]` | ISHL, ISHR |
| RI | `[opcode:8][dst:4][_:4][imm16:16]` | ILDI, FLDI |
| ZI | `[opcode:8][dst:4][_:4][re:8][im:8]` | ZLDI |
| RA | `[opcode:8][reg:4][_:4][addr16:16]` | Memory load/store |
| J | `[opcode:8][addr24:24]` | JMP, CALL |
| JR | `[opcode:8][pred:4][_:4][addr16:16]` | JIF, HCEXEC |
| QP | `[opcode:8][dst_q:3][dist:3][_:18]` | QPREP |
| Q | `[opcode:8][dst:3][src:3][kern:5][c0:4][c1:4][_:5]` | QKERNEL |
| QO | `[opcode:8][dst_h:3][src_q:3][_:18]` | QOBSERVE |
| QS | `[opcode:8][qreg:3][_:5][addr:8][_:8]` | QLOAD, QSTORE |
| HR | `[opcode:8][src:4][dst:4][func:4][_:12]` | HREDUCE |
| L | `[opcode:8][label_id:16][_:8]` | LABEL pseudo |

## 3. Opcode Table

| Hex  | Mnemonic | Format | Description |
|------|----------|--------|-------------|
| 0x00 | NOP      | N      | No operation |
| 0x01 | IADD     | RRR    | R[dst] = R[lhs] + R[rhs] |
| 0x02 | ISUB     | RRR    | R[dst] = R[lhs] - R[rhs] |
| 0x03 | IMUL     | RRR    | R[dst] = R[lhs] * R[rhs] |
| 0x04 | IDIV     | RRR    | R[dst] = R[lhs] / R[rhs] |
| 0x05 | IMOD     | RRR    | R[dst] = R[lhs] % R[rhs] |
| 0x06 | IAND     | RRR    | R[dst] = R[lhs] & R[rhs] |
| 0x07 | IOR      | RRR    | R[dst] = R[lhs] \| R[rhs] |
| 0x08 | IXOR     | RRR    | R[dst] = R[lhs] ^ R[rhs] |
| 0x09 | INOT     | RR     | R[dst] = ~R[src] |
| 0x0A | ISHL     | RRS    | R[dst] = R[src] << amt |
| 0x0B | ISHR     | RRS    | R[dst] = R[src] >> amt |
| 0x0C | ILDI     | RI     | R[dst] = sign_extend(imm16) |
| 0x0D | ILDM     | RA     | R[dst] = CMEM[addr] |
| 0x0E | ISTR     | RA     | CMEM[addr] = R[src] |
| 0x0F | IEQ      | RRR    | R[dst] = (R[lhs] == R[rhs]) ? 1 : 0 |
| 0x10 | ILT      | RRR    | R[dst] = (R[lhs] < R[rhs]) ? 1 : 0 |
| 0x11 | IGT      | RRR    | R[dst] = (R[lhs] > R[rhs]) ? 1 : 0 |
| 0x12 | FADD     | RRR    | F[dst] = F[lhs] + F[rhs] |
| 0x13 | FSUB     | RRR    | F[dst] = F[lhs] - F[rhs] |
| 0x14 | FMUL     | RRR    | F[dst] = F[lhs] * F[rhs] |
| 0x15 | FDIV     | RRR    | F[dst] = F[lhs] / F[rhs] |
| 0x16 | FLDI     | RI     | F[dst] = imm16 as f64 |
| 0x17 | FLDM     | RA     | F[dst] = f64::from_bits(CMEM[addr]) |
| 0x18 | FSTR     | RA     | CMEM[addr] = F[src].to_bits() |
| 0x19 | FEQ      | RRR    | R[dst] = (F[lhs] == F[rhs]) ? 1 : 0 |
| 0x1A | FLT      | RRR    | R[dst] = (F[lhs] < F[rhs]) ? 1 : 0 |
| 0x1B | FGT      | RRR    | R[dst] = (F[lhs] > F[rhs]) ? 1 : 0 |
| 0x1C | ZADD     | RRR    | Z[dst] = Z[lhs] + Z[rhs] |
| 0x1D | ZSUB     | RRR    | Z[dst] = Z[lhs] - Z[rhs] |
| 0x1E | ZMUL     | RRR    | Z[dst] = Z[lhs] * Z[rhs] |
| 0x1F | ZDIV     | RRR    | Z[dst] = Z[lhs] / Z[rhs] |
| 0x20 | ZLDI     | ZI     | Z[dst] = (re, im) as complex |
| 0x21 | ZLDM     | RA     | Z[dst] = complex from CMEM[addr..addr+1] |
| 0x22 | ZSTR     | RA     | CMEM[addr..addr+1] = Z[src] |
| 0x23 | CVTIF    | RR     | F[dst] = R[src] as f64 |
| 0x24 | CVTFI    | RR     | R[dst] = F[src] as i64 |
| 0x25 | CVTFZ    | RR     | Z[dst] = (F[src], 0.0) |
| 0x26 | CVTZF    | RR     | F[dst] = Z[src].real |
| 0x27 | JMP      | J      | PC = addr24 |
| 0x28 | JIF      | JR     | if R[pred] != 0: PC = addr16 |
| 0x29 | CALL     | J      | push PC+1; PC = addr24 |
| 0x2A | RET      | N      | PC = pop(); halt if empty |
| 0x2B | HALT     | N      | Set trap_halt |
| 0x2C | LABEL    | L      | Pseudo: label_id marker |
| 0x30 | QPREP    | QP     | Q[dst] = new_dist(dist_id) |
| 0x31 | QKERNEL  | Q      | Q[dst] = kernel(Q[src], R[c0], R[c1]) |
| 0x32 | QOBSERVE | QO     | H[dst] = measure(Q[src]) |
| 0x33 | QLOAD    | QS     | Q[dst] = QMEM[addr] |
| 0x34 | QSTORE   | QS     | QMEM[addr] = Q[src] |
| 0x38 | HFORK    | N      | Set hybrid fork flags |
| 0x39 | HMERGE   | N      | Set hybrid merge flags |
| 0x3A | HCEXEC   | JR     | if PSW.flag: PC = addr16 |
| 0x3B | HREDUCE  | HR     | dst = reduce(H[src], func) |

Reserved ranges: 0x2D-0x2F (control flow), 0x35-0x37 (quantum), 0x3C-0xFF (future).

## 4. Distribution IDs (QPREP dist field)

| ID | Name    | Description |
|----|---------|-------------|
| 0  | uniform | Equal probability over all basis states |
| 1  | zero    | Delta distribution at \|0\> |
| 2  | bell    | Correlated pair: P(00) = P(11) = 0.5 |
| 3  | ghz     | GHZ state: P(0000) = P(1111) = 0.5 |

## 5. Kernel IDs (QKERNEL kernel field)

| ID | Name       | Description |
|----|------------|-------------|
| 0  | init       | Re-initialize to uniform superposition |
| 1  | entangle   | Create inter-qubit correlations |
| 2  | fourier    | DFT-like phase transformation |
| 3  | diffuse    | Grover diffusion operator |
| 4  | grover_iter| Oracle + diffusion step |

## 6. PSW Flag IDs (HCEXEC flag field)

| ID | Name | Description |
|----|------|-------------|
| 0  | ZF   | Zero flag |
| 1  | NF   | Negative flag |
| 2  | OF   | Overflow flag |
| 3  | PF   | Predicate flag |
| 4  | QF   | Quantum active |
| 5  | SF   | Superposition present |
| 6  | EF   | Entanglement present |
| 7  | HF   | Hybrid mode active |

## 7. Reduction Function IDs (HREDUCE func field)

| ID | Name      | Output File | Description |
|----|-----------|-------------|-------------|
| 0  | round     | R (i64) | Round to nearest integer |
| 1  | floor     | R (i64) | Floor toward negative infinity |
| 2  | ceil      | R (i64) | Ceiling toward positive infinity |
| 3  | trunc     | R (i64) | Truncate toward zero |
| 4  | abs       | R (i64) | Absolute value |
| 5  | negate    | R (i64) | Negate |
| 6  | magnitude | F (f64) | Complex magnitude |
| 7  | phase     | F (f64) | Complex phase (atan2) |
| 8  | real      | F (f64) | Real part of complex |
| 9  | imag      | F (f64) | Imaginary part of complex |
| 10 | mean      | F (f64) | Distribution mean |
| 11 | mode      | R (i64) | Distribution mode (most probable) |
| 12 | argmax    | R (i64) | Index of most probable state |
| 13 | variance  | F (f64) | Distribution variance |
