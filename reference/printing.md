# CQAM Printing Reference: ECALL PRINT\_STR, ECALL PRINT\_HIST, and ECALL PRINT\_CMPX

## Overview

The CQAM virtual machine provides three environment calls for producing human-readable
output: `ECALL PRINT_STR` for formatted text output from classical memory,
`ECALL PRINT_HIST` for structured display of hybrid register contents, and
`ECALL PRINT_CMPX` for printing a complex number held in a Z-register.  All three are
invoked through the `ECALL` instruction using their procedure ID names.  None of these
calls pushes the call stack — execution resumes at PC+1 upon return.

The full set of ECALL procedure IDs is:

| ID | Name          | Description                                          |
|----|---------------|------------------------------------------------------|
|  0 | `PRINT_INT`   | Print R0 as a signed decimal integer, with newline   |
|  1 | `PRINT_FLOAT` | Print F0 as a floating-point number, with newline    |
|  2 | `PRINT_STR`   | Print a formatted string from CMEM                  |
|  3 | `PRINT_CHAR`  | Print R0 as a single ASCII character (no newline)    |
|  4 | `DUMP_REGS`   | Dump all non-zero registers to stderr (debug)        |
|  5 | `PRINT_HIST`  | Display the contents of an H register               |
|  6 | `PRINT_CMPX`  | Print Z[R0] as a complex number in `a + ib` form    |

---

## ECALL PRINT\_STR (ProcId 2)

### Purpose

Reads a NUL-terminated format string from classical memory (CMEM) beginning at
the address in R0, processes format specifiers, and writes the result to stdout
without appending a newline unless `\n` appears in the string itself.  The format
string is read byte-by-byte as ASCII; the `len` field (R1) tells the executor how
many bytes to consume.

### Register Convention

| Register | Role                                                                           |
|----------|--------------------------------------------------------------------------------|
| R0       | CMEM base address of the format string                                         |
| R1       | Length in characters, **excluding** the NUL terminator                         |
| R2, R3, R4, ... | Integer arguments, consumed left-to-right for each `%d` specifier   |
| F1, F2, F3, ... | Float arguments, consumed left-to-right for each `%f` specifier     |
| Z0, Z1, Z2, ... | Complex arguments, consumed left-to-right for each `%c` specifier  |

Note that integer arguments begin at R2 (R0 and R1 are reserved for the address and
length), float arguments begin at F1 (F0 is not consumed), and complex arguments begin
at Z0.

### Format Specifiers

| Specifier | Source register  | Output                                           |
|-----------|------------------|--------------------------------------------------|
| `%d`      | Next Rn (n≥2)    | Signed decimal integer from the R-file           |
| `%f`      | Next Fn (n≥1)    | Floating-point number from the F-file            |
| `%c`      | Next Zn (n≥0)    | Complex number from the Z-file in `a + ib` / `a - ib` form |
| `%%`      | _(none)_         | Literal `%` character                            |
| Any other `%x` | _(none)_   | Passes through verbatim as the two-character sequence `%x` |

There is no width or precision field syntax: `%d` and `%f` always use Rust's default
`{}` format, which for `f64` produces the shortest exact representation.

### Argument Cursor Rules

Integer arguments use a cursor that starts at register index 2 and advances by 1
with each `%d` encountered.  Float arguments use a separate cursor that starts at
register index 1 and advances by 1 with each `%f` encountered.  Complex arguments
use a third independent cursor that starts at register index 0 and advances by 1
with each `%c` encountered.  All three cursors advance independently; mixing
specifiers in the same format string interleaves each register file correctly.

If a cursor advances beyond its register file boundary (index ≥ 16 for R and F
files, index ≥ 8 for the Z file), the corresponding `get()` call will return
`Err(RegisterOutOfBounds)` and that specifier will produce no output for that slot
(the error is swallowed silently by the `if let Ok(v)` guard in the executor).

### String Storage in CMEM

Strings are declared with `.ascii` in the `.data` section.  Each character occupies
one CMEM cell (one i64 word), stored as its ASCII code value.  The assembler appends a
NUL byte (value 0) as the final cell.  The `@label` pseudo-expression resolves to the
base CMEM address of the string.  The `@label.len` pseudo-expression resolves to the
total number of cells including the NUL terminator.

The correct length to pass in R1 is therefore `@label.len - 1`, which excludes the
NUL terminator from the scan.  In practice this is written as:

```
ILDI R1, @banner.len
ILDI R15, 1
ISUB R1, R1, R15
```

or, using the pattern common in the example programs, the subtraction can be done with
any scratch register before calling `ECALL PRINT_STR`.

### Output

Output goes to **stdout** via `print!()` (no trailing newline is added by the runtime).
Newlines must be embedded in the format string as `\n` characters in the `.ascii`
directive.

### Complete Example

```
.data
    .org 1000
banner:
    .ascii "=== Quantum RNG ===\n"
result_fmt:
    .ascii "Samples: %d, mean: %f\n"

.code
    ; Print banner (no arguments)
    ILDI R0, @banner
    ILDI R1, @banner.len
    ILDI R15, 1
    ISUB R1, R1, R15
    ECALL PRINT_STR

    ; Print formatted result: R2 = sample count, F1 = mean
    ILDI  R2, 100          ; integer argument -> %d
    FLDI  F1, 0            ; float argument   -> %f (placeholder; set by program logic)
    ILDI R0, @result_fmt
    ILDI R1, @result_fmt.len
    ILDI R15, 1
    ISUB R1, R1, R15
    ECALL PRINT_STR
```

**Output:**
```
=== Quantum RNG ===
Samples: 100, mean: 0
```

### Using `%c` to Print Complex Values Inline

`%c` lets a format string embed a complex number without a separate `ECALL PRINT_CMPX`
call.  The output format is identical to `ECALL PRINT_CMPX`: `a + ib` when the
imaginary part is non-negative, `a - ib` when negative.

```
#! qubits 2

.data
    .org 1000
amp_fmt:
    .ascii "|00> amplitude: %c\n"

.code
    QPREP  Q0, UNIFORM
    QAMP   Z0, Q0, 0           ; store amplitude of |00> in Z0

    ILDI R0, @amp_fmt
    ILDI R1, @amp_fmt.len
    ILDI R15, 1
    ISUB R1, R1, R15
    ECALL PRINT_STR

    HALT
```

**Output (uniform 2-qubit state):**
```
|00> amplitude: 0.5 + i0
```

Multiple `%c` specifiers consume Z0, Z1, Z2, ... in order:

```
.ascii "a0=%c  a1=%c\n"
```

| Specifier occurrence | Source |
|----------------------|--------|
| First `%c`           | Z0     |
| Second `%c`          | Z1     |

### Multiple Specifiers of the Same Type

When more than one `%d` or `%f` appears, arguments are drawn from consecutive
registers.  Given:

```
.ascii "Results: samples=%d, sum=%d, mean=%f, sq_deviation=%f\n"
```

The executor maps:

| Specifier occurrence | Source  |
|----------------------|---------|
| First `%d`           | R2      |
| Second `%d`          | R3      |
| First `%f`           | F1      |
| Second `%f`          | F2      |

So before calling `ECALL PRINT_STR`, load the arguments as:

```
    ILDM R2, 103      ; sample count
    ILDM R3, 104      ; sum
    FLDM F1, 100      ; mean
    FLDM F2, 102      ; squared deviation
```

### Edge Cases

**Zero-length string.** If R1 = 0, the format loop iterates zero times and nothing
is printed.

**Empty format string pointing to NUL.** The NUL byte is ASCII 0, which is not `%`,
so it is passed to `output.push(0 as char)` and then printed as a control character.
Best practice is to always use `@label.len - 1` rather than loading R1 by hand.

**More specifiers than available argument registers.** The `if let Ok(v)` guard
silently omits the substitution for any specifier whose cursor has overflowed the
register file.  The format string continues to be scanned; subsequent specifiers that
do not overflow will still be substituted.

**CMEM address out of range.** R0 is cast to u16 before indexing.  If the value in R0
exceeds 65535 due to sign, the wrapping cast produces a wrapped address; the resulting
output will be garbage characters.  Always use `.org` in the `.data` section together
with the `@label` mechanism to ensure a valid address.

**Register pressure.** `ECALL PRINT_STR` does not save or restore any registers.
Because R0 and R1 are clobbered by the address/length arguments, callers that need to
preserve their loop counters or results should spill them to CMEM with `ISTR` or `FSTR`
before the call and reload them with `ILDM` or `FLDM` afterward (as done in
`qrng.cqam`).

---

## ECALL PRINT\_HIST (ProcId 5)

### Purpose

Reads a hybrid register H[R0] and renders its contents to stdout as a formatted
table, bar chart, or summary, depending on the mode in R1.  The output is followed by
a newline.  This is the primary way to inspect measurement results produced by
`QOBSERVE` and the per-shot histograms produced by running with `--shots N`.

### Register Convention

| Register | Role                                                                          |
|----------|-------------------------------------------------------------------------------|
| R0       | Hybrid register index to display (0–7)                                        |
| R1       | Display mode (0 = table, 1 = bar chart, 2 = sorted-by-state, 3 = top-K)      |
| R2       | Top-K count (mode 3 only); if R2 ≤ 0 or unset, defaults to 5                 |

R0 must be in [0, 7].  Values outside this range return
`CqamError::TypeMismatch` and terminate execution.

### Display Modes

#### Mode 0 — Table (probability-descending order)

Prints all outcomes sorted from highest probability (or highest count, for `Hist`) to
lowest.  Each row shows the basis state as a zero-padded binary string of width
`num_qubits` (read from the runtime configuration), followed by the probability or
count.

```
H0 (exact, 8 outcomes):
  |000> : 0.125000
  |001> : 0.125000
  |010> : 0.125000
  ...
```

For shot histograms, each row shows the count and the percentage:

```
H0 (1024 shots, 8 outcomes):
  |000> :    132 ( 12.89%)
  |001> :    130 ( 12.70%)
  ...
```

#### Mode 1 — Bar Chart

Renders a visual horizontal bar for each outcome, scaled so the highest-probability
outcome fills exactly 50 block characters (`█`).  The percentage is printed at the
right.

```
H1 (exact, 2 outcomes):
  |000> ██████████████████████████████████████████████████  50.00%
  |111> ██████████████████████████████████████████████████  50.00%
```

For shot histograms the bar is scaled against the highest count and percentages are
computed against `total_shots`.

#### Mode 2 — Sorted by State Index

Identical output format to mode 0, but rows are sorted numerically by basis state
index rather than by probability.  Useful when scanning across the full Hilbert space
in a predictable order.

```
H0 (exact, 8 outcomes):
  |000> : 0.125000
  |001> : 0.125000
  |010> : 0.125000
  |011> : 0.125000
  |100> : 0.125000
  |101> : 0.125000
  |110> : 0.125000
  |111> : 0.125000
```

#### Mode 3 — Top-K

Sorts by probability (or count) descending, then shows only the K highest outcomes,
where K = R2 (defaulting to 5 if R2 ≤ 0).  A summary line reports how many outcomes
were omitted and their aggregate probability.

```
H3 (exact, top 2 of 65536):
  |0000000000000000> : 0.500000
  |1111111111111111> : 0.500000
```

```
H3 (exact, top 10 of 65536):
  |0000000000000000> : 0.500000
  |1111111111111111> : 0.500000
  ... 65534 more outcomes (0.00% total)
```

Any mode value not in {0, 1, 2, 3} falls through to the default branch, which behaves
identically to mode 0 (probability-descending table).

### HybridValue Handling

The display is determined by the concrete variant held in the register:

| HybridValue variant | Display produced                                                          |
|---------------------|---------------------------------------------------------------------------|
| `Empty`             | `H{n}: (empty)`                                                           |
| `Int(k)`            | `H{n}: \|{binary}\> (single sample)` — binary width = `num_qubits`       |
| `Float(f)`          | `H{n}: {:.6}` — six decimal places                                        |
| `Complex(re, im)`   | `H{n}: ({:.6} + {:.6}i)` or `H{n}: ({:.6} - {:.6}i)` depending on sign   |
| `Dist(entries)`     | Full distribution table/bar/top-K per mode (from `QOBSERVE DIST`)         |
| `Hist(histogram)`   | Shot-count table/bar/top-K per mode (from `--shots N` runs)               |

The mode and top-K parameters are only meaningful for `Dist` and `Hist` variants.
For scalar variants (`Empty`, `Int`, `Float`, `Complex`) the mode and top-K arguments
are read from registers but have no effect on formatting.

**`Int` variant.** This is the result of `QOBSERVE` with mode `SAMPLE`, which produces
a single projective measurement outcome.  It is displayed as a binary state label
rather than a number, because the value represents a computational basis state index.

**`Dist` variant.** This is the result of `QOBSERVE` with mode `DIST` (the default).
It holds the full diagonal of the density matrix as a vector of (state, probability)
pairs.  Probabilities are `f64` values in [0.0, 1.0] that should sum to 1.0.

**`Hist` variant.** This is produced when the program is run with `--shots N`.  In
this mode `QOBSERVE` performs N projective measurements and accumulates integer counts.
The `total_shots` field records N; the `counts` map records outcomes.  Percentages
printed by the formatter are computed as `count / total_shots * 100`.

### Binary State Labels

All state labels are zero-padded to `num_qubits` binary digits.  The qubit count is
taken from `ctx.config.default_qubits`, which is set by the `#! qubits N` pragma or
the `--qubits N` command-line flag.  A 3-qubit program labels states as `|000>`
through `|111>`; a 16-qubit program labels them as `|0000000000000000>` through
`|1111111111111111>`.

### Complete Example — Uniform Superposition

```
#! qubits 3

.code
    QPREP Q0, UNIFORM
    QOBSERVE H0, Q0, DIST, R0, R0

    ; Mode 0: table, probability-descending
    ILDI R0, 0
    ILDI R1, 0
    ECALL PRINT_HIST

    ; Mode 1: bar chart
    ILDI R0, 0
    ILDI R1, 1
    ECALL PRINT_HIST

    ; Mode 2: sorted by state index
    ILDI R0, 0
    ILDI R1, 2
    ECALL PRINT_HIST

    ; Mode 3: top 3 outcomes
    ILDI R0, 0
    ILDI R1, 3
    ILDI R2, 3
    ECALL PRINT_HIST

    HALT
```

### Complete Example — GHZ Top-K

From `ghz_verify.cqam`: after observing a 16-qubit GHZ state, only the two non-zero
entries are meaningful.  Mode 3 with K = 10 makes this explicit:

```
    QPREP Q0, GHZ
    QOBSERVE H3, Q0

    ILDI R0, 3    ; H register index
    ILDI R1, 3    ; Mode 3: top-K
    ILDI R2, 10   ; show at most 10
    ECALL PRINT_HIST
```

**Output (16 qubits):**
```
H3 (exact, top 2 of 65536):
  |0000000000000000> : 0.500000
  |1111111111111111> : 0.500000
```

### Complete Example — Single Sample

From `qrng.cqam`: `QOBSERVE` with mode `SAMPLE` stores an `Int` in H0, so
`PRINT_HIST` displays it as a single-sample state label:

```
    QOBSERVE H0, Q0, SAMPLE
    ILDI R0, 0
    ILDI R1, 0    ; mode has no effect on Int variant
    ECALL PRINT_HIST
```

**Output (8 qubits, example):**
```
H0: |10110011> (single sample)
```

### Edge Cases

**Empty register.** Printing an unwritten H register (e.g., H7 before any write)
produces `H7: (empty)`.  No error is raised.

**R2 ≤ 0 in mode 3.** The executor guards `v > 0`; any other value of R2 (including 0
or negative) causes the top-K default of 5 to be used.  If R2 is never written, its
zero-initialized value triggers the default.

**K larger than the number of outcomes.** If R2 exceeds the number of distinct
outcomes, all outcomes are shown and the "... N more" line is omitted.

**H register index out of range.** R0 ≥ 8 returns `CqamError::TypeMismatch` and
terminates execution.  Values are not wrapped.

---

## ECALL PRINT\_CMPX (ProcId 6)

### Purpose

Reads the complex number stored in Z-register Z[R0] and writes it to stdout in
human-readable algebraic notation.  The output has the form `a + ib` when the
imaginary part is non-negative and `a - ib` when it is negative, where `a` and `b`
are formatted using Rust's default `{}` display for `f64` (shortest exact
representation, no trailing zeros beyond what is needed for exactness).  No newline
is appended; callers that need a line break must follow the call with
`ECALL PRINT_CHAR` or embed the newline in a subsequent `ECALL PRINT_STR`.

### Register Convention

| Register | Role                                      |
|----------|-------------------------------------------|
| R0       | Z-register index to display (0–7)         |

No other registers are read.  R0 must be in [0, 7]; the Z-register file has eight
slots indexed from 0.

### Output Format Specification

The executor produces output according to the following rule, where `re` and `im`
are the real and imaginary components of the stored complex number:

| Condition   | Output template      | Example            |
|-------------|----------------------|--------------------|
| `im >= 0.0` | `{re} + i{im}`       | `3.14 + i2.72`     |
| `im < 0.0`  | `{re} - i{-im}`      | `3.14 - i2.72`     |

The sign of `re` is part of its default `f64` rendering and is printed as-is.
The letter `i` precedes the magnitude of the imaginary part in both cases; the sign
is conveyed by the operator (`+` or `-`) between the two terms.

When `im` is exactly `0.0` the condition `im >= 0.0` is true, so the output is
`{re} + i0`.  There is no special-case suppression of the imaginary term.

#### Selected examples

| Z-register value       | Output            |
|------------------------|-------------------|
| `3.14 + 2.72i`         | `3.14 + i2.72`    |
| `3.14 - 2.72i`         | `3.14 - i2.72`    |
| `3.14 + 0.0i`          | `3.14 + i0`       |
| `0.0 + 2.72i`          | `0 + i2.72`       |
| `0.0 + 0.0i`           | `0 + i0`          |
| `-3.14 + 2.72i`        | `-3.14 + i2.72`   |
| `-3.14 - 2.72i`        | `-3.14 - i2.72`   |
| `1e-10 - 1e-10i`       | `0.0000000001 - i0.0000000001` |

The last row illustrates that Rust's default `f64` display writes the shortest
decimal that round-trips, which for small values may be a long decimal string rather
than scientific notation.

### Complete Example

```
#! qubits 2

.data
    .org 1000
label:
    .ascii "amplitude = "
newline:
    .ascii "\n"

.code
    ; Prepare a 2-qubit state and extract an amplitude into Z0
    QPREP  Q0, UNIFORM
    QAMP   Z0, Q0, 0       ; store amplitude of |00> in Z0

    ; Print label
    ILDI R0, @label
    ILDI R1, @label.len
    ILDI R15, 1
    ISUB R1, R1, R15
    ECALL PRINT_STR

    ; Print the complex amplitude
    ILDI R0, 0             ; Z-register index
    ECALL PRINT_CMPX

    ; Print newline
    ILDI R0, @newline
    ILDI R1, @newline.len
    ILDI R15, 1
    ISUB R1, R1, R15
    ECALL PRINT_STR

    HALT
```

**Output (uniform 2-qubit state, amplitude of |00> = 0.5 + 0i):**
```
amplitude = 0.5 + i0
```

### Notes on Edge Cases

**Zero imaginary part.** When `im == 0.0`, the output is `{re} + i0`, not just
`{re}`.  The imaginary term is always present.  Programs that need to suppress it
must test and branch before calling `ECALL PRINT_CMPX`.

**Zero real part.** When `re == 0.0`, Rust's `{}` format renders it as `0`, so the
output is `0 + i{im}` or `0 - i{im}`.

**Both parts zero.** Output is `0 + i0`.

**Negative real part.** The minus sign for `re` is part of its `{}` rendering and
appears before `re` in the output, e.g. `-1 + i0`.  The `+` or `-` between the terms
still reflects the sign of `im` only.

**No newline appended.** `ECALL PRINT_CMPX` uses `print!`, not `println!`.  A
newline will not appear unless explicitly printed afterward.

**Register clobbering.** `ECALL PRINT_CMPX` reads only R0 and the Z-register it
names.  No registers are written.  R0 is consumed as an argument and remains
unchanged after the call.

**Z-register index out of range.** If R0 >= 8, the `zregs.get()` call returns
`CqamError::RegisterOutOfBounds` and execution terminates.

---

## Common Patterns

### Banner + Formatted Output

The idiomatic CQAM output pattern prints a static header followed by parameterized
results:

```
.data
    .org 1000
banner:
    .ascii "=== My Program ===\n"
out_fmt:
    .ascii "n=%d, p=%f\n"

.code
    ; Print banner (no format args)
    ILDI R0, @banner
    ILDI R1, @banner.len
    ILDI R15, 1
    ISUB R1, R1, R15
    ECALL PRINT_STR

    ; ... compute results into R2 and F1 ...

    ILDI R0, @out_fmt
    ILDI R1, @out_fmt.len
    ILDI R15, 1
    ISUB R1, R1, R15
    ECALL PRINT_STR
```

### Spilling Registers Around ECALL

`ECALL PRINT_STR` overwrites R0 and R1 with the address and length arguments.  If
those registers hold live loop state, spill them to CMEM first:

```
    ; Spill loop registers
    ISTR R0, 90       ; save loop counter
    ISTR R1, 91       ; save constant 1

    ; Set up print arguments
    ILDI R0, 0        ; H index
    ILDI R1, 0        ; mode
    ECALL PRINT_HIST

    ; Restore loop registers
    ILDM R0, 90
    ILDM R1, 91
```

This pattern appears verbatim in `examples/basic/qrng.cqam`.

### The `@label` / `@label.len` Mechanism

Every `.ascii` declaration creates two assembler pseudo-symbols:

| Symbol       | Value                                                           |
|--------------|-----------------------------------------------------------------|
| `@label`     | CMEM address of the first character cell                        |
| `@label.len` | Total number of cells including the NUL terminator              |

The value loaded by `ILDI R1, @banner.len` is the **total** length including NUL.
The PRINT_STR executor scans exactly R1 bytes, so passing the raw `.len` value would
include the NUL in the output (appearing as a control character).  The correct idiom
always subtracts 1 before the call.

Because `ILDI` accepts a 16-bit signed immediate, strings whose base address or length
exceeds 32767 must use `ILDM` with a pre-stored constant, though in practice `.data`
sections are placed well below this limit.

### Storing Results in CMEM Then Printing

When results from multiple loop iterations must be passed as format arguments, a common
approach is to compute all results first and write them to fixed CMEM slots, then load
them into the correct argument registers immediately before the `ECALL`:

```
    ; --- store results ---
    FSTR F5, 100      ; mean  -> CMEM[100]
    ISTR R2, 103      ; count -> CMEM[103]
    ISTR R3, 104      ; sum   -> CMEM[104]

    ; --- load arguments into calling-convention positions ---
    ILDM R2, 103      ; first  %d <- count
    ILDM R3, 104      ; second %d <- sum
    FLDM F1, 100      ; first  %f <- mean

    ILDI R0, @res_fmt
    ILDI R1, @res_fmt.len
    ILDI R15, 1
    ISUB R1, R1, R15
    ECALL PRINT_STR
```

This separates the computation phase from the presentation phase and avoids register
aliasing issues.
