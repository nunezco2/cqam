# CQAM Syntax (VS Code)

TextMate-based syntax highlighting and starter snippets for `.cqam`
source files — the Classical-Quantum Assembly Language used by the
CQAM runtime.

This extension is **grammar-only**: no LSP, no TypeScript, no runtime
dependencies. It contributes a language, a grammar, and a handful of
snippets. Nothing activates beyond the language contribution itself.

## Features

- Highlighting for all CQAM opcode families (integer, float, complex,
  conversion, control flow, interrupts, system, quantum, hybrid).
- Distinct scopes for kernel mnemonics (`QFFT`, `GROV`, …), reduce
  functions (`REALP`, `MODEV`, `ARGMX`, …), ECALL procedures
  (`PRINT_STR`, …), PSW flag names, distributions, modes, traps.
- Register highlighting for `R/F/Z0..15` and `Q/H0..7`.
- Label definitions and `@label`/`@label.len` references.
- Integer, hex, binary, float, and complex (`aJb`) numeric literals.
- String literals with `\n \t \\ \" \0` escapes and `%d %f %s %c`
  placeholders.
- `#` and `//` line comments (with `#!` pragma lines preserved).
- Starter snippets: program skeleton, quantum pipeline, observe-|0⟩,
  HFORK/HMERGE block, PRINT_STR boilerplate, JMPF.

## Install

Choose one of:

### 1. Development symlink (fastest)

```sh
ln -s "$PWD" ~/.vscode/extensions/cqam-local.cqam-syntax-0.1.0
```

Then *Developer: Reload Window* (`Cmd+Shift+P`). Edits to the grammar
take effect on the next reload.

### 2. Package and install

Requires [`@vscode/vsce`](https://github.com/microsoft/vscode-vsce):

```sh
npm i -g @vscode/vsce      # one-time
vsce package               # produces cqam-syntax-0.1.0.vsix
code --install-extension cqam-syntax-0.1.0.vsix
```

## Verifying the grammar

Open any file under `../../examples/basic/` and run
*Developer: Inspect Editor Tokens and Scopes* (`Cmd+Shift+P`). Hover
tokens and confirm the scope chain contains `source.cqam`.

Suggested smoke-test files:

| File | Exercises |
|---|---|
| `deutsch_jozsa.cqam` | comments, pragmas, sections, `.org`/`.ascii`, labels, `@label.len`, `ECALL PRINT_STR`, quantum+classical opcodes, `HREDUCE REALP/MODEV` |
| `bernstein_vazirani.cqam` | `#! qubits N`, `%d` placeholders, `IQCFG` |
| `qft_16q.cqam` | 16-qubit `QKERNEL QFFT/QIFT/PHSH` pipeline |
| `ecall_hello.cqam` | minimal `.ascii` + `ECALL PRINT_STR` |
| `../intermediate/grover_16q.cqam` | `DIFF/GROV/CTLU` kernels, `HREDUCE ARGMX`, `JIF/JMP/JMPF` |

## Out of scope

No LSP, diagnostics, formatting, completion, hover info, DAP, or custom
themes. The authoritative assembler in `cqam-as` remains the single
source of syntactic validation — this extension just colors tokens.

## Ground truth

Grammar scopes are derived from the lexer/parser in
`cqam-core/src/parser/text.rs` and the `Instruction`, `KernelId`,
`ReduceFn`, and `ProcId` enums in `cqam-core/src/instruction.rs`.
Keep this extension in sync with those files when the ISA changes.
