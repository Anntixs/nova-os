# SMP bring-up status

## IMPLEMENTED / EXPERIMENTAL

NovaBoot protocol v2 reserves one UEFI LoaderData page below 1 MiB when available. The BSP copies a position-independent trampoline to that page, patches physical GDTR, jumps, CR3, AP stack, logical index and Rust entry, then sends INIT/deassert/SIPI (with one SIPI retry) through xAPIC ICR. The AP runs 16-bit real-mode code, enables protected mode, PAE and long mode, loads the BSP's identity-mapped kernel CR3, switches to its dedicated stack, and enters Rust. The AP loads its own GDT, TSS, IST stacks and IDT, enables NX and Local APIC, writes its GS-based CPU-local record, acknowledges, then idles with HLT. Startup is sequential and bounded by timer deadlines. QEMU `-smp 4` acknowledged APIC IDs 1–3; `-smp 8` acknowledged IDs 1–7.

If an IPI fails or an AP misses its acknowledgement deadline, BSP stops starting further APs. The trampoline page is shared; reusing its patched stack or logical ID while a late AP may still be starting could corrupt another CPU's execution state. A late AP remains outside the scheduler.

**Only the BSP is scheduler-online.** AP acknowledgement proves execution of Nova AP code, not multi-CPU task scheduling. `/proc/cpuinfo` separates discovered CPUs, APs that entered Nova, and CPUs admitted to the scheduler. The AP limit is eight preallocated slots. If NovaBoot cannot reserve a suitable low page, the kernel continues on the BSP.

The optional `NOVA_SMPTEST=1` build mode runs 256 physical-frame and heap allocation/verification cycles on each AP. QEMU with four CPUs completed this cross-CPU allocator probe and the normal userspace regression. The scheduler has a pinned host context, host CR3 and current-thread slot for each of eight CPU records; only the BSP slot is used to run threads. A yielding, blocking or exiting thread now stays Running and owned by its CPU until that CPU has switched to its host stack. The host then publishes the new state under the scheduler lock, so an AP cannot claim its live kernel stack. This handoff passed the existing BSP userspace regression and QEMU scheduler self-test; no AP task scheduling has been enabled. The process manager still has a single `current` process. IPC blocking/wakeup across active scheduler CPUs remains untested.

The syscall entry no longer uses global stack/RSP scratch variables: it obtains both from a trusted CPU-local GS record. First Ring 3 entry, syscall, IRQ and fatal exception paths pair their GS transitions. A userspace regression program loads a null GS selector, exercises syscalls and waits for a timer IRQ, then confirms that the kernel remains alive. The scheduler and process manager still need CPU-local running context before this can be used for AP user threads.

The QEMU startup and userspace regression matrix passed with `-smp 1`, `2`, `4`, and `8`. A separate `-smp 4 -m 4G` run with `NOVA_SMPTEST=1` passed. These tests establish AP entry and allocator concurrency only; they do not establish SMP scheduling, process-table concurrency, VFS concurrency, or hardware support outside QEMU.

## PLANNED before Stage B

CPU-local process ownership, cross-CPU testing of the new context-switch handoff, CPU-safe process and VFS tables, AP syscall entry stacks, TLB shootdown, interrupt-safe blocking synchronization, reschedule IPIs and concurrent stress tests. Existing locks that only clear local IF are not SMP-safe. No NVMe/AHCI/USB work should begin until this stage is stable.
