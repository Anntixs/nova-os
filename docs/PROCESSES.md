# Processes and threads

## IMPLEMENTED

PID 0 is reserved for kernel threads; kernel boot loads `/system/init` from VFS as PID 1. Init spawns `/bin/nsh`, and nsh spawns command ELF files. Every user process has a distinct address space, PID, parent PID, cwd, environment, file table, exit status and at least one thread with a dedicated 32 KiB kernel stack. `spawn` copies user arguments, reads the child ELF through VFS, creates PT_LOAD/stack mappings and places argc/argv/envp on the child stack. `wait` blocks the parent, then reaps the exited child and releases its address space and kernel stack. User faults mark only the offending process as faulted and wake its waiting parent. Child cwd and environment are inherited; standard FDs 0–2 refer to the shared terminal.

The BSP scheduler has a ready queue and blocked states for child wait, TTY read and sleep. `yield` rotates the queue; timer IRQs also preempt after a time slice. The scheduler host loop wakes sleepers and services TTY input. Kernel init and the emergency monitor run on the host kernel stack. The process manager stores processes in a stable table and tracks the active PID per logical CPU. An atomic lock with local IRQ masking protects this table. Only the BSP schedules normal threads; CPU-local slots are preparation for AP scheduling, not evidence that it works.

## IMPLEMENTED — Foundation extensions

Processes can have additional userspace threads, each with a TID, independent 32 KiB kernel stack, caller-supplied mapped userspace stack and scheduler state. `THREAD_CREATE`, `THREAD_EXIT`, and `THREAD_JOIN` are implemented; `/bin/threadtest` exercises them. Spawn inherits open descriptors, including pipes/channels. A dying parent transfers its children to PID 1; init runs a reaper thread that blocks in `WAIT(0)` for adopted orphans. The direct parent can use blocking `WAIT` or `WAIT_TRY`. `KILL` currently targets only a direct child. User stacks have an unmapped lower guard page; the kernel stack has no guard page yet. `/bin/memtest` tests a user stack-guard fault and recovery.

`WAIT` reports a dead child only after all its threads are Exited and no CPU still owns one of their kernel stacks. The scheduler publishes thread exit after switching to its host stack, then wakes the parent. `KILL` preserves its chosen exit status if the target later attempts a normal exit. A running target receives a pending stop request; the next local timer interrupt or reschedule IPI switches it off its stack before the parent may reap it. An AP kernel-worker diagnostic verified the remote IPI and stack handoff. Ordinary Ring 3 threads still run only on the BSP, so cross-CPU user-process termination remains untested.

Kernel stacks are zero-initialized directly in heap storage. The previous `Box::new([0; 32768])` expression created a 32 KiB temporary on the caller's kernel stack in debug builds; creating init's reaper thread overflowed that stack and corrupted the process cwd. The heap-backed allocation was verified with the full QEMU init regression in debug mode. Kernel-stack guard pages remain planned.

## EXPERIMENTAL / PLANNED

PID/TID values monotonically increase until reboot. Thread entry/stack ownership is the user's responsibility; there is no pthread ABI. No SMP, signals, process groups, permissions, per-process memory quotas or general capability rights yet. The kernel scheduler still uses the bootstrap CPU only.
