# Advanced Non-Threaded Examples

19 programs exercising the full CQAM ISA without threaded features
(no HFORK/HMERGE, HATMS/HATME, or shared memory). All programs use
`QOBSERVE` for measurement (no `QSAMPLE`), the `IADD/ISUB` idiom for
register copies (no `IMOV`), and consistent `R15=1 / R14=0` conventions.

## Tier 1 — ISA Coverage Programs

Original programs designed to maximally cover all high-level ISA blocks,
quantum flags, and instruction categories. Each targets a specific
subsystem of the architecture.

| Program | Focus | Key Features |
|---------|-------|-------------|
| `isr_safe_division.cqam` | ISR, integer arithmetic, bitwise ops | SETIV/RETI, CALL/RET, JMPF ZF/NF, indirect memory (ISTRX/ILDX), all integer ALU ops |
| `complex_signal_analysis.cqam` | Complex/float math, type conversion | Full Z-register ops, FSIN/FCOS/FATAN2/FSQRT, CVTIF/CVTFI/CVTFZ/CVTZF, QENCODE Z_FILE, HREDUCE MAGNI/PHASE/REALP/IMAGP/CONJZ/NEGTZ |
| `multi_kernel_pipeline.cqam` | All 11 quantum kernels | UNIT/ENTG/QFFT/DIFF/GROV/DROT/PHSH/QIFT/CTLU/DIAG/PERM, QKERNELF/QKERNELZ, QPREPR, QSTORE/QLOAD, QHADM/QFLIP/QPHASE, HREDUCE EXPCT, JMPF SF/EF/QF |
| `quantum_state_engineering.cqam` | Gate-level ops, state manipulation | QPREPN, QCNOT/QCZ/QSWAP/QROT, QMEAS, QRESET, QENCODE F_FILE, QTENSOR, QPTRACE, all 6 float-to-int HREDUCE (ROUND/FLOOR/CEILI/TRUNC/ABSOL/NEGAT) |
| `grover_with_verification.cqam` | Grover search with subroutines | Iterative GROV kernel, FATAN2-computed iteration count, CALL/RET, JMPF EF, HREDUCE MEANT/VARNC/ARGMX/MODEV |
| `expectation_value_engine.cqam` | HREDUCE EXPCT under multiple distributions | HREDUCE EXPCT with eigenvalues from CMEM, QENCODE R_FILE, QMIXED, 4 distribution types |
| `quantum_error_recovery.cqam` | Quantum error ISR, decoherence handling | Trap 1 (quantum error), QPTRACE on Bell state, QCUSTOM, QPREPN, JMPF DF/CF |

## Tier 2 — Algorithm Reimplementations

Advanced rewrites of existing basic/intermediate programs. Same algorithms,
but with maximal use of subroutines (CALL/RET), `.data` section tables,
and concise register management.

| Program | Original | Reduction | Key Improvements |
|---------|----------|-----------|-----------------|
| `deutsch_jozsa.cqam` | `intermediate/` (124 lines) | 54% | Two-test constant/balanced discriminator, QPHASE as balanced oracle |
| `quantum_counting.cqam` | `intermediate/` (137 lines) | 38% | Doubling Grover depths (1,2,4,8) + QFT, factored inner loop |
| `simon.cqam` | `intermediate/` (156 lines) | 1% | pow2 + popcount subroutines via CALL/RET, parametric 4-round query |
| `phase_estimation.cqam` | `intermediate/` (126 lines) | 23% | pow2 subroutine, FATAN2 for exact 2*pi, rotation doubling |
| `quantum_walk.cqam` | `intermediate/` (123 lines) | 61% | Coin-shift model (QHADM + DIFF), single final QOBSERVE |
| `vqe_loop.cqam` | `intermediate/` (124 lines) | 36% | CALL/RET trial_state subroutine, step-search optimizer |
| `shor_period.cqam` | `intermediate/` (178 lines) | 48% | pow2 subroutine, DROT+PHSH phase encoding, classical continued-fraction post-processing |
| `reversible_adder.cqam` | `basic/` (486 lines) | 83% | `.i64` data tables for permutation (no runtime ISTR), 3-test harness with MODEV verification |
| `bitflip_repetition.cqam` | `intermediate/` (187 lines) | 66% | QCNOT encoding, QMEAS syndrome extraction, JIF syndrome decode + QFLIP correction |

## Tier 3 — Flag-Verified Algorithms

Programs designed to demonstrate credible use of quantum flags (SF, EF, CF)
in algorithmic contexts. Flags are intent-based: they reflect the last
quantum operation's purpose, not a measured state property.

| Program | Flags | Use Case |
|---------|-------|----------|
| `teleportation.cqam` | SF, EF, CF | Quantum teleportation of &#124;1> via Bell pair. SF verified after QHADM (superposition intent), EF after QCNOT (entanglement intent), CF after QMEAS (collapse confirmed before classical corrections). |
| `adaptive_grover.cqam` | SF, EF, CF | Grover search with QSTORE/QLOAD convergence probing. Non-destructive mode check at each iteration; SF confirms superposition intent after GROV kernel; terminates when mode matches target. |
| `state_classifier.cqam` | SF, EF, CF, QF | Comprehensive flag demonstration. Classifies 6 state types (ZERO, UNIFORM, BELL, GHZ, QHADM, QCNOT) by (SF, EF) signature. Shows CF lifecycle: set by QOBSERVE, cleared by HREDUCE. 12/12 automated checks. |

## Running

```bash
# Single program
cargo run --bin cqam-run -- examples/advanced_nothreads/teleportation.cqam

# With options
cargo run --bin cqam-run -- examples/advanced_nothreads/adaptive_grover.cqam --qubits 8
cargo run --bin cqam-run -- examples/advanced_nothreads/reversible_adder.cqam --print-final-state

# All programs (should all exit cleanly)
for f in examples/advanced_nothreads/*.cqam; do
  echo "--- $(basename $f) ---"
  cargo run --quiet --bin cqam-run -- "$f"
done
```

## Conventions

All programs follow these conventions:

- **R15 = 1, R14 = 0** — constant registers, never overwritten
- **Register copy idiom**: `IADD Rd, Rs, R15; ISUB Rd, Rd, R15` (no IMOV)
- **ECALL PRINT_STR**: R0=address, R1=length, R2..R15=%d args, F1..F15=%f args
- **CALL/RET** for reusable subroutines (pow2, popcount, read_flags, trial_state)
- **QOBSERVE** for all measurement (QSAMPLE not used)
- **HREDUCE MODEV** for basis-state identification (not ARGMX, which returns array index)
- **`.i64` / `.c64` / `.f64`** data sections for lookup tables and kernel parameters
