" Vim syntax file for CQAM (Classical-Quantum Abstract Machine)
" Language:    CQAM Assembly
" Maintainer:  Santiago Nunez-Corrales
" Last Change: 2026-03-21

if exists("b:current_syntax")
  finish
endif

" Case-sensitive matching (CQAM mnemonics are uppercase)
syn case match

" =============================================================================
" Comments and pragmas
" =============================================================================
syn match   cqamComment   "#.*$" contains=cqamTodo
syn keyword cqamTodo      TODO FIXME XXX NOTE contained
syn match   cqamPragma    "^#!.*$"

" =============================================================================
" Sections and directives
" =============================================================================
syn match   cqamSection   "^\.\(data\|code\|shared\|private\)\>"
syn match   cqamDirective "\.\(ascii\|i64\|f64\|c64\|qstate\|org\)\>"

" =============================================================================
" Labels
" =============================================================================
syn match   cqamLabelDef  "^LABEL:\s*\w\+"
syn match   cqamLabelDef  "^\w\+:"
syn match   cqamLabelRef  "@\w\+\(\.\w\+\)\?"

" =============================================================================
" Integer arithmetic and logic (R-file)
" =============================================================================
syn keyword cqamIntOp     ILDI IADD ISUB IMUL IDIV IMOD
syn keyword cqamIntOp     IAND IOR IXOR INOT ISHL ISHR
syn keyword cqamIntOp     IEQ ILT IGT
syn keyword cqamIntOp     IINC IDEC IMOV

" =============================================================================
" Integer memory
" =============================================================================
syn keyword cqamMemOp     ILDM ISTR ILDX ISTRX

" =============================================================================
" Float arithmetic and memory (F-file)
" =============================================================================
syn keyword cqamFloatOp   FLDI FADD FSUB FMUL FDIV FABS
syn keyword cqamFloatOp   FEQ FLT FGT
syn keyword cqamFloatOp   FSIN FCOS FATAN2 FSQRT
syn keyword cqamFloatOp   FLDM FSTR FLDX FSTRX
syn keyword cqamFloatOp   FMOV

" =============================================================================
" Complex arithmetic and memory (Z-file)
" =============================================================================
syn keyword cqamCmpxOp    ZLDI ZADD ZSUB ZMUL ZDIV
syn keyword cqamCmpxOp    ZLDM ZSTR ZLDX ZSTRX
syn keyword cqamCmpxOp    ZMOV

" =============================================================================
" Type conversions
" =============================================================================
syn keyword cqamConvOp    CVTIF CVTFI CVTFZ CVTZF

" =============================================================================
" Control flow
" =============================================================================
syn keyword cqamControl   JMP JIF JMPF CALL RET
syn keyword cqamControl   HALT NOP
syn keyword cqamControl   SETIV RETI
syn keyword cqamControl   HFORK HMERGE HATMS HATME

" =============================================================================
" Quantum state preparation
" =============================================================================
syn keyword cqamQuantumOp QPREP QPREPN QPREPR
syn keyword cqamQuantumOp QPREPS QPREPSM
syn keyword cqamQuantumOp QENCODE QMIXED

" =============================================================================
" Quantum kernels
" =============================================================================
syn keyword cqamQuantumOp QKERNEL QKERNELF QKERNELZ

" =============================================================================
" Quantum gate-level operations
" =============================================================================
syn keyword cqamQuantumOp QHADM QFLIP QPHASE
syn keyword cqamQuantumOp QCNOT QCZ QSWAP QROT QCUSTOM

" =============================================================================
" Quantum observation, measurement, composite
" =============================================================================
syn keyword cqamQuantumOp QOBSERVE QMEAS QRESET
syn keyword cqamQuantumOp QTENSOR QPTRACE QSTORE QLOAD
syn keyword cqamQuantumOp QXCH

" =============================================================================
" Hybrid register operations
" =============================================================================
syn keyword cqamHybridOp  HREDUCE

" =============================================================================
" System calls
" =============================================================================
syn keyword cqamSysOp     ECALL IQCFG ICCFG ITID

" =============================================================================
" ECALL procedure names
" =============================================================================
syn keyword cqamEcallProc PRINT_STR PRINT_HIST PRINT_CMPX
syn keyword cqamEcallProc PRINT_INT PRINT_FLOAT PRINT_CHAR DUMP_REGS

" =============================================================================
" Distribution IDs (4-letter standard + backward compat)
" =============================================================================
syn keyword cqamDistId    ZERO UNIF BELL GHZS
syn keyword cqamDistId    UNIFORM GHZ

" =============================================================================
" Kernel IDs
" =============================================================================
syn keyword cqamKernelId  UNIT ENTG QFFT QIFT DIFF GROV
syn keyword cqamKernelId  DROT PHSH CTLU DIAG PERM

" =============================================================================
" Observe modes
" =============================================================================
syn keyword cqamMode      DIST PROB SAMPLE

" =============================================================================
" HREDUCE functions
" =============================================================================
syn keyword cqamReduceFn  MODEV ARGMX MEANT VARNC
syn keyword cqamReduceFn  ROUND FLOOR CEILI TRUNC ABSOL NEGAT
syn keyword cqamReduceFn  MAGNI PHASE REALP IMAGP CONJZ NEGTZ
syn keyword cqamReduceFn  EXPCT

" =============================================================================
" Rotation axes
" =============================================================================
syn match   cqamAxis      "\<[XYZ]\>" contained

" =============================================================================
" PSW flag IDs (for JMPF)
" =============================================================================
syn keyword cqamFlag      ZF SF CF DF EF QF INF OF NF
syn keyword cqamFlag      FK MG IF AF

" =============================================================================
" Registers
" =============================================================================
syn match   cqamIReg      "\<R\(1[0-5]\|[0-9]\)\>"
syn match   cqamFReg      "\<F\(1[0-5]\|[0-9]\)\>"
syn match   cqamZReg      "\<Z[0-7]\>"
syn match   cqamQReg      "\<Q[0-7]\>"
syn match   cqamHReg      "\<H[0-7]\>"

" =============================================================================
" Literals
" =============================================================================
syn match   cqamNumber    "\<-\?\d\+\>"
syn match   cqamFloat     "\<-\?\d\+\.\d*\>"
syn region  cqamString    start='"' end='"' contains=cqamEscape
syn match   cqamEscape    "\\[nrt\\\"0]" contained
syn match   cqamComplex   "\<-\?\d\+\(\.\d*\)\?J-\?\d\+\(\.\d*\)\?\>"

" =============================================================================
" Highlight linking
" =============================================================================
hi def link cqamComment   Comment
hi def link cqamTodo      Todo
hi def link cqamPragma    PreProc
hi def link cqamSection   Structure
hi def link cqamDirective PreProc
hi def link cqamLabelDef  Label
hi def link cqamLabelRef  Special

hi def link cqamIntOp     Statement
hi def link cqamMemOp     Statement
hi def link cqamFloatOp   Statement
hi def link cqamCmpxOp    Statement
hi def link cqamConvOp    Statement
hi def link cqamControl   Conditional
hi def link cqamQuantumOp Keyword
hi def link cqamHybridOp  Keyword
hi def link cqamSysOp     Function

hi def link cqamEcallProc Function
hi def link cqamDistId    Constant
hi def link cqamKernelId  Constant
hi def link cqamMode      Constant
hi def link cqamReduceFn  Constant
hi def link cqamAxis      Constant
hi def link cqamFlag      Constant

hi def link cqamIReg      Identifier
hi def link cqamFReg      Type
hi def link cqamZReg      Type
hi def link cqamQReg      Special
hi def link cqamHReg      Special

hi def link cqamNumber    Number
hi def link cqamFloat     Float
hi def link cqamString    String
hi def link cqamEscape    SpecialChar
hi def link cqamComplex   Number

let b:current_syntax = "cqam"
