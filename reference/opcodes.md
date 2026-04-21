# CQAM Binary Opcode Reference

## 1. Instruction Word Format

All instructions are 32 bits wide (4 bytes). Bit numbering is big-endian:
bits [31:24] contain the 8-bit opcode.

## 2. Encoding Formats

| Format | Layout | Used By |
|--------|--------|---------|
| N | `[opcode:8][_:24]` | NOP, RET, HALT, HFORK, HMERGE, HATMS, HATME, RETI |
| RRR | `[opcode:8][dst:4][lhs:4][rhs:4][_:12]` | Arithmetic, comparison, FATAN2 |
| RR | `[opcode:8][dst:4][src:4][_:16]` | INOT, CVTxx, FSIN, FCOS, FSQRT |
| RRS | `[opcode:8][dst:4][src:4][amt:6][_:10]` | ISHL, ISHR |
| RI | `[opcode:8][dst:4][_:4][imm16:16]` | ILDI, FLDI |
| ZI | `[opcode:8][dst:4][_:4][re:8][im:8]` | ZLDI |
| RA | `[opcode:8][reg:4][_:4][addr16:16]` | Memory load/store |
| J | `[opcode:8][addr24:24]` | JMP, CALL |
| JR | `[opcode:8][pred:4][_:4][addr16:16]` | JIF, JMPF, SETIV |
| QP | `[opcode:8][dst_q:3][dist:3][_:18]` | QPREP |
| Q | `[opcode:8][dst:3][src:3][kern:5][c0:4][c1:4][_:5]` | QKERNEL, QKERNELF, QKERNELZ |
| QO_EXT | `[opcode:8][dst_h:3][src_q:3][mode:2][ctx0:4][ctx1:4][_:8]` | QOBSERVE |
| QS | `[opcode:8][qreg:3][_:5][addr:8][_:8]` | QLOAD, QSTORE |
| HR | `[opcode:8][src:4][dst:4][func:4][_:12]` | HREDUCE |
| QR | `[opcode:8][dst_q:3][_:1][dist_reg:4][_:16]` | QPREPR |
| QE | `[opcode:8][dst_q:3][_:1][src_base:4][count:4][file_sel:2][_:10]` | QENCODE |
| QMK | `[opcode:8][dst_q:3][src_q:3][_:4][mask_reg:4][_:10]` | QHADM, QFLIP, QPHASE |
| Q2Q | `[opcode:8][dst_q:3][src_q:3][ctrl_reg:4][tgt_reg:4][_:10]` | QCNOT, QCZ, QSWAP |
| QROT_FMT | `[opcode:8][dst_q:3][src_q:3][qubit_reg:4][axis:2][angle_freg:4][_:8]` | QROT |
| QMEAS_FMT | `[opcode:8][dst_r:4][src_q:3][qubit_reg:4][_:13]` | QMEAS |
| QTT | `[opcode:8][dst_q:3][src0_q:3][src1_q:3][_:15]` | QTENSOR |
| QCU | `[opcode:8][dst_q:3][src_q:3][base_addr_reg:4][dim_reg:4][_:10]` | QCUSTOM |
| QMX | `[opcode:8][dst_q:3][_:1][base_addr_reg:4][count_reg:4][_:12]` | QMIXED |
| QPN | `[opcode:8][dst_q:3][dist:3][qubit_count_reg:4][_:14]` | QPREPN |
| QPT | `[opcode:8][dst_q:3][src_q:3][num_qubits_a_reg:4][_:14]` | QPTRACE |
| QRS | `[opcode:8][dst_q:3][src_q:3][qubit_reg:4][_:14]` | QRESET |
| Q2 | `[opcode:8][qa:3][qb:3][_:18]` | QXCH |
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
| 0x2D | RETI     | N      | Pop saved PC, clear maskable traps, resume |
| 0x2E | SETIV    | JR     | Register handler for trap_id at label |
| 0x30 | QPREP    | QP     | Q[dst] = new_state(dist_id) |
| 0x31 | QKERNEL  | Q      | Q[dst] = kernel(Q[src], R[c0], R[c1]) |
| 0x32 | QOBSERVE | QO_EXT | H[dst] = observe(Q[src], mode, ctx0, ctx1); destructive |
| 0x33 | QLOAD    | QS     | Teleport QMEM[addr] into Q[dst]; QMEM[addr] consumed; costs one Bell pair |
| 0x34 | QSTORE   | QS     | Teleport Q[src] into QMEM[addr]; Q[src] consumed; costs one Bell pair |
| 0x35 | ILDX     | RR     | R[dst] = CMEM[R[addr_reg]] |
| 0x36 | ISTRX    | RR     | CMEM[R[addr_reg]] = R[src] |
| 0x37 | FLDX     | RR     | F[dst] = f64::from_bits(CMEM[R[addr_reg]]) |
| 0x38 | HFORK    | N      | Spawn N-1 worker threads (SPMD); requires QF=0; creates SharedQuantumFile, SharedMemory, ThreadBarrier |
| 0x39 | HMERGE   | N      | Join all worker threads; restore Q registers; write back shared memory; requires QF=0 |
| 0x3A | JMPF   | JR     | if PSW.flag: PC = addr16 |
| 0x3B | HREDUCE  | HR     | dst = reduce(H[src], func) |
| 0x3C | FSTRX    | RR     | CMEM[R[addr_reg]] = F[src].to_bits() |
| 0x3D | ZLDX     | RR     | Z[dst] from CMEM[R[addr_reg]]..+1 |
| 0x3E | ZSTRX    | RR     | CMEM[R[addr_reg]]..+1 = Z[src] |
| 0x40 | RESERVED | —      | Formerly QSAMPLE (removed; non-destructive observation violates physical realism) |
| 0x41 | QKERNELF | Q      | Q[dst] = kernel(Q[src], F[fctx0], F[fctx1]) |
| 0x42 | QKERNELZ | Q      | Q[dst] = kernel(Q[src], Z[zctx0], Z[zctx1]) |
| 0x43 | QPREPR   | QR     | Q[dst] = new_state(R[dist_reg]) |
| 0x44 | QENCODE  | QE     | Q[dst] = from_amplitudes(regs[base..+count]) |
| 0x45 | QHADM    | QMK    | Apply H to qubits per R[mask] bitmask |
| 0x46 | QFLIP    | QMK    | Apply X to qubits per R[mask] bitmask |
| 0x47 | QPHASE   | QMK    | Apply Z to qubits per R[mask] bitmask |
| 0x48 | QCNOT    | Q2Q    | Q[dst] = CNOT(Q[src], ctrl=R[ctrl_reg], tgt=R[tgt_reg]) |
| 0x49 | QROT     | QROT_FMT | Q[dst] = R_axis(F[angle_freg])(Q[src], qubit=R[qubit_reg]) |
| 0x4A | QMEAS    | QMEAS_FMT | R[dst] = measure_qubit(Q[src], qubit=R[qubit_reg]); non-destructive |
| 0x4B | QTENSOR  | QTT    | Q[dst] = Q[src0] tensor Q[src1]; both sources consumed |
| 0x4C | QCUSTOM  | QCU    | Q[dst] = U * Q[src] * U†; U from CMEM[R[base_addr_reg]], dim=R[dim_reg] |
| 0x4D | QCZ      | Q2Q    | Q[dst] = CZ(Q[src], ctrl=R[ctrl_reg], tgt=R[tgt_reg]) |
| 0x4E | QSWAP    | Q2Q    | Q[dst] = SWAP(Q[src], a=R[qubit_a_reg], b=R[qubit_b_reg]) |
| 0x4F | QMIXED   | QMX    | Q[dst] = sum_i w_i |psi_i><psi_i|; data in CMEM[R[base]], count=R[count] |
| 0x51 | QPREPN   | QPN    | Q[dst] = new_state(dist, num_qubits=R[qubit_count_reg]) |
| 0x52 | FSIN     | RR     | F[dst] = sin(F[src]) |
| 0x53 | FCOS     | RR     | F[dst] = cos(F[src]) |
| 0x54 | FATAN2   | RRR    | F[dst] = atan2(F[lhs], F[rhs]) (lhs=y, rhs=x) |
| 0x55 | FSQRT    | RR     | F[dst] = sqrt(F[src]); traps if F[src] < 0 |
| 0x56 | QPTRACE  | QPT    | Q[dst] = Tr_B(Q[src]); num_qubits_a=R[reg]; non-destructive |
| 0x57 | QRESET   | QRS    | Q[dst] = reset_qubit(Q[src], qubit=R[qubit_reg]); result guaranteed |0> |
| 0x58 | IQCFG    | RR     | R[dst] = configured qubit count |
| 0x59 | ICCFG    | RR     | R[dst] = configured thread count |
| 0x5A | ITID     | RR     | R[dst] = current thread index (0-based) |
| 0x5B | HATMS    | N      | Atomic section start: full barrier + leader election |
| 0x5C | HATME    | N      | Atomic section end: snapshot commit + barrier resume |
| 0x60 | IINC     | RR     | R[dst] = R[src] + 1; ZF/SF/OF updated |
| 0x61 | IDEC     | RR     | R[dst] = R[src] - 1; ZF/SF/OF updated |
| 0x62 | IMOV     | RR     | R[dst] = R[src]; ZF/SF updated |
| 0x63 | FMOV     | RR     | F[dst] = F[src]; no PSW update |
| 0x64 | ZMOV     | RR     | Z[dst] = Z[src]; no PSW update |
| 0x65 | QXCH     | Q2     | Swap Q-file handles between Q[qa] and Q[qb]; zero-cost, no gates emitted; self-swap (qa==qb) encodes as NOP |

Reserved: 0x2F (interrupt), 0x40 (formerly QSAMPLE, removed), 0x50 (reserved), 0x5D-0x5F (reserved), 0x66-0xFF (future).

## 4. Distribution IDs (QPREP / QPREPR / QPREPN dist field)

| ID | Name    | Description |
|----|---------|-------------|
| 0  | uniform | Equal probability over all basis states |
| 1  | zero    | Delta distribution at \|0\> |
| 2  | bell    | Correlated pair: P(00) = P(11) = 0.5 |
| 3  | ghz     | GHZ state: P(0000) = P(1111) = 0.5 |

## 5. Kernel IDs (QKERNEL / QKERNELF / QKERNELZ kernel field)

In source text, the kernel is identified by its four-letter mnemonic as the
first operand (e.g., `QKERNEL ENTG, Q1, Q0, R0, R1`). The assembler encodes
the mnemonic to the 5-bit numeric ID in the `kern` field.

| ID | Name             | Mnemonic | Description |
|----|------------------|----------|-------------|
| 0  | init             | UNIT | Re-initialize to uniform superposition |
| 1  | entangle         | ENTG | Create inter-qubit correlations (CNOT cascade) |
| 2  | fourier          | QFFT | Quantum Fourier Transform |
| 3  | diffuse          | DIFF | Grover diffusion operator (inversion about the mean) |
| 4  | grover_iter      | GROV | Oracle phase-flip + diffusion step |
| 5  | rotate           | DROT | Diagonal rotation: U[k][k] = exp(i * theta * k) |
| 6  | phase_shift      | PHSH | Phase shift: U[k][k] = exp(i * |z| * k) |
| 7  | fourier_inv      | QIFT | Inverse Quantum Fourier Transform |
| 8  | controlled_u     | CTLU | Controlled-U: apply sub-kernel conditioned on control qubit |
| 9  | diagonal_unitary | DIAG | Arbitrary diagonal unitary from CMEM complex pairs |
| 10 | permutation      | PERM | Basis-state permutation from CMEM table |

## 6. PSW Flag IDs (JMPF flag field)

`JMPF` uses flag name syntax in source text: `JMPF EF, label`. The assembler
maps each name to its numeric ID for the binary `pred` field. SF, EF, and IF
are intent-based flags set by kernel identity, not dynamic state inspection.

| ID | Name | Description |
|----|------|-------------|
| 0  | ZF   | Zero flag |
| 1  | NF   | Negative flag |
| 2  | OF   | Overflow flag |
| 3  | PF   | Predicate flag |
| 4  | QF   | Quantum active |
| 5  | SF   | Superposition created by last kernel |
| 6  | EF   | Entanglement created by last kernel |
| 7  | HF   | Hybrid mode active |
| 12 | IF   | Interference exploited by last kernel |
| 13 | AF   | Atomic section active (set on elected leader between HATMS and HATME) |

## 7. Reduction Function IDs (HREDUCE func field)

`HREDUCE` uses mnemonic-first syntax: `HREDUCE MNEM, H_src, R/F/Z_dst`
(e.g., `HREDUCE ARGMX, H0, R2`). The assembler maps each five-letter mnemonic
to its numeric ID for the binary `func` field.

| ID | Name      | Mnemonic | Output File | Description |
|----|-----------|----------|-------------|-------------|
| 0  | round     | ROUND | R (i64) | Round to nearest integer |
| 1  | floor     | FLOOR | R (i64) | Floor toward negative infinity |
| 2  | ceil      | CEILI | R (i64) | Ceiling toward positive infinity |
| 3  | trunc     | TRUNC | R (i64) | Truncate toward zero |
| 4  | abs       | ABSOL | R (i64) | Absolute value |
| 5  | negate    | NEGAT | R (i64) | Negate |
| 6  | magnitude | MAGNI | F (f64) | Complex magnitude: sqrt(re^2 + im^2) |
| 7  | phase     | PHASE | F (f64) | Complex phase: atan2(im, re) |
| 8  | real      | REALP | F (f64) | Real part of complex |
| 9  | imag      | IMAGP | F (f64) | Imaginary part of complex |
| 10 | mean      | MEANT | F (f64) | Distribution mean |
| 11 | mode      | MODEV | R (i64) | Distribution mode (most probable value) |
| 12 | argmax    | ARGMX | R (i64) | Index of most probable state |
| 13 | variance  | VARNC | F (f64) | Distribution variance |
| 14 | conj_z    | CONJZ | Z (f64,f64) | Complex conjugate: (re, -im) |
| 15 | negate_z  | NEGTZ | Z (f64,f64) | Complex negation: (-re, -im) |
| 16 | expect    | EXPCT | F (f64) | Expectation value: sum_k eigenvalue_k * p_k; eigenvalues from CMEM[R[ctx]..+n] |

Output register file depends on the function ID:
- IDs 0-5: result written to integer register file (R).
- IDs 6-10, 13, 16: result written to float register file (F).
- IDs 11-12: result written to integer register file (R).
- IDs 14-15: result written to complex register file (Z).

## 8. Observation Mode IDs (QOBSERVE mode field)

| ID | Name | Output Type | Description |
|----|------|-------------|-------------|
| 0  | DIST | Dist(Vec<(u16,f64)>) | Full diagonal distribution |
| 1  | PROB | Float(f64) | Single probability at R[ctx0] |
| 2  | AMP  | Complex(f64,f64) | Density matrix element dm[R[ctx0]][R[ctx1]] |

## 9. File Selector IDs (QENCODE file_sel field)

| ID | Name   | Register File | Conversion |
|----|--------|---------------|------------|
| 0  | R_FILE | R (integer)   | (val as f64, 0.0) |
| 1  | F_FILE | F (float)     | (val, 0.0) |
| 2  | Z_FILE | Z (complex)   | (re, im) used directly |

## 10. Rotation Axis IDs (QROT axis field)

| ID | Name | Description |
|----|------|-------------|
| 0  | X    | Rotation about X axis: Rx(theta) |
| 1  | Y    | Rotation about Y axis: Ry(theta) |
| 2  | Z    | Rotation about Z axis: Rz(theta) |

## 11. Trap IDs (SETIV trap_id field)

| ID | Name          | Description |
|----|---------------|-------------|
| 0  | arithmetic    | Arithmetic fault (division by zero) |
| 1  | quantum_error | Quantum fidelity violation |
| 2  | sync_failure  | Hybrid branch synchronization failure |

## 12. QMIXED CMEM Layout

`QMIXED` reads statevector/weight entries from CMEM starting at `R[base_addr_reg]`.
Each entry occupies `2 + 2*dim` consecutive CMEM cells:

```
CMEM[base + 0]        : weight (f64 bits, interpreted as float)
CMEM[base + 1]        : dim (u64 bits, statevector length = 2^num_qubits)
CMEM[base + 2]        : re(amplitude[0]) (f64 bits)
CMEM[base + 3]        : im(amplitude[0]) (f64 bits)
...
CMEM[base + 2 + 2*k]  : re(amplitude[k])
CMEM[base + 3 + 2*k]  : im(amplitude[k])
```

Entries are read consecutively for `R[count_reg]` statevectors. The resulting
density matrix is rho = sum_i w_i |psi_i><psi_i|, renormalized so Tr(rho) = 1.
