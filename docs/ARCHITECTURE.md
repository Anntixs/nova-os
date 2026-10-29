# Nova architecture

## IMPLEMENTED — Nova OS 0.5.0

UEFI runs NovaBoot, which loads the freestanding Rust kernel and supplies BootInfo. The kernel initializes CPU, memory, PCI/VirtIO Block, FAT32/VFS, scheduler, syscalls, APIC/timer, PS/2 and TTY. It reads `/system/init` from the mounted volume and starts it in Ring 3 as PID 1. Init spawns `/bin/nsh`; nsh spawns independent ELF applications. C programs link Nova CRT/libc and communicate only through Nova syscall ABI. Disk content is accessed through VFS and block layers. The framebuffer terminal remains in the kernel; userspace sees FDs 0–2, not device memory.

This separates bootloader, kernel, storage, process and userspace responsibilities. The kernel's old shell is retained as an emergency monitor. The development disk remains a raw FAT32 volume; a partitioned production layout is planned.

## IMPLEMENTED — Foundation work after 0.5.0

The BSP runs timer-driven preemptive round-robin scheduling for user and ordinary kernel threads. APs have scheduler host loops that can run pinned diagnostic PID 0 workers; general user SMP is not enabled. Processes can contain multiple userspace threads, block on TTY, child exit, IPC or sleep, and are reaped by their parent or PID 1 after orphan adoption. Kernel VFS mounts FAT32 at `/`, a device filesystem at `/dev`, and a generated information filesystem at `/proc`. Per-process descriptors carry files, devices, pipes and channels. `/dev/fb0` reports display metadata; drawing uses controlled framebuffer syscalls and a single-owner lease. The PS/2 driver publishes timestamped keyboard press and release events separately from TTY character input. Serial records also enter a bounded kernel log, readable through `/proc/kmsg`.

## EXPERIMENTAL / PLANNED

APs start through INIT/SIPI, load private GDT/TSS/IST/IDT state, and run kernel-only scheduler loops. Static ET_EXEC programs, one TTY and raw FAT32 storage remain. User preemption is BSP-only; IPC and process lifetime have not been validated under general SMP scheduling. User pages are eagerly mapped, and there is no copy-on-write, file mapping, swap, POSIX signals, permissions, dynamic linker, mouse driver, network, audio or graphics acceleration. Framebuffer drawing and the kernel terminal currently share the same surface; a future display server needs exclusive ownership and a separate console policy.
