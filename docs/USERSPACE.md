# Userspace

## Foundation additions

The initial C runtime also builds `/bin/memtest`, `/bin/schedtest`, `/bin/threadtest`, `/bin/pipetest`, `/bin/msgtest`, `/bin/devtest`, `/bin/fbtest`, `/bin/uptime`, `/bin/sysinfo`, `/bin/ps` and `/bin/dmesg`. They are separate static ELF files on FAT32 and use Nova syscalls. The C allocator obtains anonymous pages from `MAP`; `free` reuses its blocks inside the process, while process teardown reclaims mapped pages. `/system/init` starts a reaper thread for adopted orphans. `/bin/nsh` understands a basic trailing `&`, PATH lookup and direct-child `kill`, plus the earlier file commands. This is not a full POSIX shell.

## IMPLEMENTED

NovaBoot enters the `no_std` kernel; after PCI/VirtIO/FAT32/VFS initialization, the kernel reads `/system/init` from disk, parses ELF64 PT_LOAD headers, maps a separate user address space and enters Ring 3 by IRETQ. PID 1 starts Ring 3 `/bin/nsh` with `SPAWN` and waits. `nsh` reads FD 0 through the TTY and spawns independent `/bin/ls`, `/bin/cat`, `/bin/echo`, `/bin/pwd` and C/Rust test executables. `hello-c` is an ordinary C `main` using Nova CRT and `printf`; no Linux libc or interpreter is present. `badmem` and `badopcode` terminate their own processes and leave nsh running.

The ELF link address remains fixed at `0x0000400000000000`; the ELF loader checks image bounds and rejects W+X segments. Text pages are user executable but not writable; data, anonymous mappings and the two-page user stack are non-executable when NX is present. `spawn` inherits cwd and environment. The stack ABI is in [ABI.md](ABI.md). `scripts/test-init.py` exercises keyboard-driven nsh, program execution, libc, argv, cwd and user fault isolation in QEMU.

## EXPERIMENTAL / PLANNED

Static ET_EXEC only. No dynamic loader, PIE, user threads beyond one per process, generic permissions or syscall ABI stability guarantee. The current system disk is a dedicated raw FAT32 development image. The former kernel shell is available as an emergency monitor (`NOVA_MONITOR=1`) and on init failure.
