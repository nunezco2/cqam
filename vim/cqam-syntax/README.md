# CQAM Syntax Highlighting for Vim/Neovim

Syntax highlighting, filetype detection, and auto-indentation for `.cqam` assembly files.

## Installation

### vim-plug

```vim
Plug 'nunezco2/cqam', { 'rtp': 'vim/cqam-syntax' }
```

### lazy.nvim (Neovim)

```lua
{ "nunezco2/cqam", config = false, rtp = "vim/cqam-syntax" }
```

### packer.nvim

```lua
use { 'nunezco2/cqam', rtp = 'vim/cqam-syntax' }
```

### Manual

Copy the directories into your Vim runtime path:

```bash
# Vim
cp -r ftdetect syntax indent ~/.vim/

# Neovim
cp -r ftdetect syntax indent ~/.config/nvim/
```

## Features

- Filetype detection for `.cqam` files
- Syntax highlighting for all 71 ISA instructions
- Quantum operations highlighted distinctly from classical
- Register files color-coded by type (R=integer, F=float, Z=complex, Q=quantum, H=hybrid)
- Named constants: distribution IDs (ZERO/UNIF/BELL/GHZS), kernel IDs, reduce functions, flags
- ECALL procedure names, observe modes, rotation axes
- String literals with escape sequences
- Complex number literals (aJb format)
- Label definitions and @label references
- Section headers and data directives
- Comment highlighting with TODO/FIXME/XXX support
- Auto-indentation (4 spaces for instructions, 0 for labels/sections)

## Highlight Groups

| Group | Color (typical) | Elements |
|-------|----------------|----------|
| Statement | Yellow | Integer, float, complex, conversion ops |
| Keyword | Purple | Quantum ops, HREDUCE |
| Conditional | Red | Control flow (JMP, JIF, HALT) |
| Function | Cyan | ECALL, system instructions |
| Constant | Orange | Dist IDs, kernel IDs, flags, reduce fns |
| Identifier | White | R-registers |
| Type | Green | F-registers, Z-registers |
| Special | Magenta | Q-registers, H-registers, label refs |
| String | Green | String literals |
| Number | Orange | Integer and float literals |
| Comment | Gray | Comments (#) |
| PreProc | Blue | Pragmas (#!), directives (.ascii, .org) |
| Label | Yellow bold | Label definitions |
