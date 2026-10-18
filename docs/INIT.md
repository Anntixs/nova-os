# Nova init

## IMPLEMENTED

After mounting FAT32, the kernel reads `/system/init` through VFS and starts its ELF in Ring 3 as PID 1. The C `main` in `userspace/c/init.c` starts `/bin/nsh`, waits for it, reports its exit and restarts it after a blocked one-second sleep. The image builder copies `init.elf` to `/system/init` on every build. If loading or running init fails, the kernel reports the error on serial and enters the emergency kernel monitor. `NOVA_MONITOR=1` builds directly into that monitor for development regressions.

## PLANNED

Service supervision policy, configuration, shutdown handling, orphan reaping and IPC with system services. None of this lives in the kernel today.
