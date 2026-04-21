" Vim indent file for CQAM Assembly
" Language:    CQAM
" Maintainer:  Santiago Nunez-Corrales

if exists("b:did_indent")
  finish
endif
let b:did_indent = 1

setlocal indentexpr=GetCqamIndent()
setlocal indentkeys=o,O,0=LABEL:,0=.,0=HALT

function! GetCqamIndent()
  let lnum = prevnonblank(v:lnum - 1)
  if lnum == 0
    return 0
  endif

  let line = getline(v:lnum)
  let prev = getline(lnum)

  " Section headers and labels at column 0
  if line =~ '^\.\(data\|code\|shared\|private\)\>'
    return 0
  endif
  if line =~ '^LABEL:' || line =~ '^\w\+:'
    return 0
  endif
  if line =~ '^#!'
    return 0
  endif

  " Directives and instructions indented by 4
  return 4
endfunction
