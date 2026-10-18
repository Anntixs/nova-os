# Kernel locking status

## IMPLEMENTED / EXPERIMENTAL

The physical-frame allocator and kernel heap use atomic compare-exchange spinlocks with local interrupts disabled. The process table and scheduler now apply the same exclusion to their short state operations. Pipe and message-channel queues have per-object atomic locks. These operations restore the caller's previous IF state after releasing the lock. A diagnostic build (`NOVA_SMPTEST=1`) has APs concurrently allocate, verify and free frames and heap buffers; QEMU with four CPUs passed 256 iterations on each AP.

## NOT YET SMP-SAFE

The process table and scheduler still represent one current process/thread and one host context, owned by the BSP. Atomic exclusion alone does not make those identities CPU-local. VFS, FAT32, block drivers, TTY, input queues, logging and graphics need a complete cross-CPU audit before APs may run arbitrary threads. No blocking kernel mutex, generic semaphore, completion or wait-queue abstraction is implemented. Local IF masking does not stop another CPU. Stage B drivers must not be started on APs until these foundations are corrected and stress-tested.

Current observed lock order is scheduler → process manager for scheduling transitions; process exit drops descriptor references and wakes waiters after releasing the process lock. Code changes must preserve this ordering or replace it with explicit non-nested state transitions.
