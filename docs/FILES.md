# Files and commands

## IMPLEMENTED

The system disk contains `/bin` (freestanding ELF executables), `/etc/version`, `/etc/hostname`, and initially empty `/home`, `/system`, `/tmp`. `ls`, `pwd`, `cd`, `cat`, `touch`, `mkdir`, `rm`, and `write <file> <text>` use VFS. `write` truncates and writes text from the shell line buffer. `run <name>`, absolute executable paths such as `/bin/hello`, and `./hello` read bytes through VFS and pass them to the existing ELF64 loader. An unqualified command searches `/bin` after built-in commands. No executable is embedded in the kernel as a special case.

`filetest` is a separate Ring 3 ELF file. It opens `/etc/version`, reads it, checks STAT and SEEK, enumerates `/bin`, checks invalid descriptor handling, creates and writes `/home/filetest.txt`, reads it back, then exits. It uses Nova syscalls only.

## EXPERIMENTAL

Shell command lines are limited to 128 bytes. There is no quoting, environment, argv, file permission model or full PATH variable. The initial execution fallback is fixed to `/bin`. No hardlinks, rename or copy command is implemented.
