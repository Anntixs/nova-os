# Roadmap

- Milestone 0 — IMPLEMENTED: UEFI NovaBoot, kernel ELF, BootInfo, COM1, GOP.
- Milestone 1 — IMPLEMENTED: GDT, TSS/IST, IDT, exceptions, panic reporting, CPUID.
- Milestone 2 — IMPLEMENTED with bootstrap limits: UEFI-map frame allocation, four-level paging, NX heap, memory statistics.
- Milestone 3 — IMPLEMENTED for QEMU/OVMF: APIC/IOAPIC, PIT, PS/2 event queue, terminal, cpu/mem/uptime.
- Nova OS 0.3.0 — IMPLEMENTED/EXPERIMENTAL: process/thread objects, cooperative scheduling, user address spaces, ELF64 ET_EXEC, Ring 3, syscalls, fault isolation.
- Nova OS 0.4.0 — IMPLEMENTED/EXPERIMENTAL: PCI scan, legacy VirtIO Block, raw FAT32 system volume, VFS, file descriptors, file syscalls, disk-backed ELF execution, shell file commands, synchronous FAT32 mutation and reboot persistence.
- Nova OS 0.5.0 — IMPLEMENTED/EXPERIMENTAL: kernel starts FAT32 `/system/init` as PID 1; init spawns Ring 3 `/bin/nsh`; C CRT, initial libc, process spawn/wait, per-process cwd, anonymous mappings, blocking TTY input and sleep, independent ELF commands, sysroot and GCC wrapper. The logical `x86_64-unknown-nova` target is not an upstream compiler target.
- Nova OS 0.6.0 Foundation — IMPLEMENTED/EXPERIMENTAL: timer preemption on one CPU, userspace threads, orphan reaping, pipes, message channels, `/dev`, `/proc`, keyboard event records, controlled framebuffer drawing, kernel log and diagnostic C programs. SMP is still PLANNED.
- Multicore/platform development — EXPERIMENTAL: MADT CPU discovery, NovaBoot low trampoline reservation, INIT/SIPI AP entry and AP-local GDT/TSS/IST/IDT are implemented and tested in QEMU up to eight CPUs. APs now run pinned diagnostic PID 0 kernel workers through the shared scheduler; normal kernel and all user threads remain on the BSP. Full user-thread SMP, TLB shootdown, cross-CPU IPC/process stress and all Stage B drivers are PLANNED. Version remains 0.6.0.
- Nova UI — PLANNED after a stable platform milestone. The framebuffer lease now suppresses normal kernel-console rendering and restores it on release or owner death. The remaining platform gates are recorded in `UI_READINESS.md`.

NovaFS, networking, accelerated graphics, audio and Nova WinBridge remain later work.
