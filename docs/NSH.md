# NovaShell in Ring 3

## IMPLEMENTED

`/bin/nsh` is an independent static C ELF. It reads canonical input from FD 0 and writes its prompt and output to FD 1. It splits whitespace, supports single and double quoted arguments and simple backslash escaping, then spawns external commands and waits for foreground completion. `cd`, `pwd`, `help` and `exit` are builtins. Bare names search the fixed `/bin` prefix; paths with slashes are passed to Nova VFS resolution relative to the process cwd. `ls`, `cat`, `echo`, `pwd`, `hello-c` and the diagnostic programs are separate ELF executables.

## EXPERIMENTAL / PLANNED

A command line is at most 511 bytes and 32 arguments. There is no pipe, redirection, expansion, script language, job control or signal handling. The kernel monitor remains a fallback, not the normal shell.
