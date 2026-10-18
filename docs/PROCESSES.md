# Processes and threads

## IMPLEMENTED

PID 0 is reserved for kernel threads; kernel boot loads `/system/init` from VFS as PID 1. Init spawns `/bin/nsh`, and nsh spawns command ELF files. Every user process has a distinct address space, PID, parent PID, cwd, environment, file table, exit status and at least one thread with a dedicated 32 KiB kernel stack. `spawn` copies user arguments, reads the child ELF through VFS, creates PT_LOAD/stack mappings and places argc/argv/envp on the child stack. `wait` blocks the parent, then reaps the exited child and releases its address space and kernel stack. User faults mark only the offending process as faulted and wake its waiting parent. Child cwd and environment are inherited; standard FDs 0–2 refer to the shared terminal.

The single CPU scheduler has a ready queue and blocked states for child wait, TTY read and sleep. `yield` rotates the queue; timer IRQs also preempt after a time slice. The scheduler host loop wakes sleepers and services TTY input. Kernel init and the emergency monitor run on the host kernel stack. Process globals currently mask interrupts on the bootstrap CPU for synchronization.

## IMPLEMENTED — Foundation extensions

Processes can have additional userspace threads, each with a TID, independent 32 KiB kernel stack, caller-supplied mapped userspace stack and scheduler state. `THREAD_CREATE`, `THREAD_EXIT`, and `THREAD_JOIN` are implemented; `/bin/threadtest` exercises them. Spawn inherits open descriptors, including pipes/channels. A dying parent transfers its children to PID 1; init runs a reaper thread that blocks in `WAIT(0)` for adopted orphans. The direct parent can use blocking `WAIT` or `WAIT_TRY`. `KILL` currently targets only a direct child. User stacks have an unmapped lower guard page; the kernel stack has no guard page yet. `/bin/memtest` tests a user stack-guard fault and recovery.

## EXPERIMENTAL / PLANNED

PID/TID values monotonically increase until reboot. Thread entry/stack ownership is the user's responsibility; there is no pthread ABI. No SMP, signals, process groups, permissions, per-process memory quotas or general capability rights yet. The kernel scheduler still uses the bootstrap CPU only.
