# CQAM Instruction Reference Examples

This document provides examples and explanations for each instruction type in the CQAM architecture.

## Classical Instructions

| Instruction     | Format                          | Description                                      |
|-----------------|----------------------------------|--------------------------------------------------|
| Label           | `LABEL: NAME`                   | Label for jump/branch targets                   |
| Load            | `CL:LOAD R1, 42`                | Load literal into classical register             |
| Store           | `CL:STORE result, R1`           | Store register value to memory address           |
| Add             | `CL:ADD R3, R1, R2`              | Add values from two registers                    |
| Sub             | `CL:SUB R4, R3, R1`              | Subtract values from two registers               |
| Jump            | `CL:JMP LOOP`                   | Unconditional jump to label                      |
| Conditional Jump| `CL:IF pred, THEN`              | Jump to label if predicate register is true      |

## Hybrid Instructions

| Instruction       | Format                             | Description                                      |
|------------------|-------------------------------------|--------------------------------------------------|
| Hybrid Fork       | `HYB:FORK`                         | Fork quantum-classical control paths             |
| Hybrid Merge      | `HYB:MERGE`                        | Merge hybrid control paths                       |
| Hybrid Cond Exec  | `HYB:COND_EXEC QF, THEN`           | Conditional hybrid jump based on flag            |
| Hybrid Reduce     | `HYB:REDUCE src, dst, round`       | Reduction of value using specified function      |

## Quantum Instructions

| Instruction   | Format                                 | Description                                      |
|---------------|-----------------------------------------|--------------------------------------------------|
| Quantum Prep  | `QPREP q1, dist_uniform`               | Prepare quantum distribution in register         |
| Quantum Kernel| `QKERNEL q2, q1, modexp`               | Apply named kernel function to quantum data      |
| Quantum Meas  | `QMEAS m1, q2`                         | Measure quantum register and write classical data|
| Quantum Observe| `QOBSERVE m2, q3`                     | Non-destructive observation of quantum state     |

## System-Level Instructions

| Instruction | Format      | Description                             |
|-------------|-------------|-----------------------------------------|
| Halt        | `HALT`      | Terminates program execution explicitly |

---
For more details, see: `cqam-core/src/instruction.rs`
