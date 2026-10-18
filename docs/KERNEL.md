# Nova Kernel 0.6.0

## IMPLEMENTED

The kernel is Rust `#![no_std]`, built separately from NovaBoot. Initialization reports serial diagnostics for CPU, memory, paging, heap, PCI/block, FAT32/VFS, scheduler, syscalls, interrupts, timer, keyboard and TTY. Required failures panic on serial. After mounting VFS the kernel reads `/system/init` and starts Ring 3 PID 1. If init fails it enters the emergency kernel monitor.

The emergency monitor's `cpu`, `mem`, and `uptime` use CPUID, physical allocator statistics and PIT ticks. Its `run` command loads ELF bytes from FAT32. Normal operation uses user init/nsh and `SPAWN`. User faults remain process-local. The kernel's panic path disables interrupts, avoids recursive reporting and halts the CPU safely.

## EXPERIMENTAL

One active BSP, timer preemption on that CPU, fixed-size heap and no crash-safe filesystem transactions. Scheduler and IPC critical sections use local interrupt masking, so SMP is not supported. See STORAGE.md and FAT32.md for storage limits.
