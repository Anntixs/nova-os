# Scheduler

## IMPLEMENTED — Foundation extensions

The legacy ready queue now receives a timer-driven preemption request every ten PIT ticks. Context switching retains each thread's kernel stack and CPU context. `yield` remains available. Blocking waits for stdin, sleep, child exit, pipe and channel activity remove the thread from the runnable queue; producer/IRQ paths wake it. A user process can hold more than one thread. `/bin/schedtest` runs two CPU-bound children without explicit yield and has been observed to interleave `A1 B1 A2 B2 A3 B3` under QEMU.

## EXPERIMENTAL / PLANNED

Preemption is tested on a single active CPU. Queue and IPC critical sections mask local interrupts and require redesign before SMP. There is no priority policy, CPU affinity or real-time guarantee.

## IMPLEMENTED

Nova uses a FIFO ready queue and round-robin scheduling. x86_64 assembly saves RSP, RBX, RBP and R12–R15, then restores those registers from the next context and returns on its kernel stack. Rust owns run-queue and state transitions. Each thread has a dedicated kernel stack. `SYS_YIELD` queues the current thread at the tail and switches to the scheduler's host context. `SYS_EXIT` removes the thread from runnable execution permanently.

A QEMU self-test runs two kernel threads and asserts A1, B1, A2, B2, A3, B3. A userspace `yieldtest` resumes after SYS_YIELD. A thread may block on child wait, TTY input or sleep and is woken by child exit, a keyboard line or timer ticks. The scheduler restores each process PML4 before resuming a user thread and the kernel PML4 after returning to its host loop. TSS.RSP0 and the SYSCALL kernel-stack slot are updated for every scheduled thread.

## PLANNED

Priorities, SMP run queues, context switching of extended CPU state, and kernel-stack guard pages. Interrupts are disabled during kernel context transfer and SYSCALL handling to avoid an IRQ overwriting an active syscall frame via TSS.RSP0.
