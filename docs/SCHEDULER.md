# Scheduler

## IMPLEMENTED — Foundation extensions

The legacy ready queue now receives a timer-driven preemption request every ten PIT ticks. Context switching retains each thread's kernel stack and CPU context. `yield` remains available. Blocking waits for stdin, sleep, child exit, pipe and channel activity remove the thread from the runnable queue; producer/IRQ paths wake it. A user process can hold more than one thread. `/bin/schedtest` runs two CPU-bound children without explicit yield and has been observed to interleave `A1 B1 A2 B2 A3 B3` under QEMU.

When an event arrives after a consumer registers its wait but before it parks, `wake` now records a pending notification on the running thread. `block_current` consumes that notification and retries the condition instead of sleeping after the event. This closes the enqueue-to-park lost-wakeup window for the current BSP scheduler. A future multicore scheduler still needs an atomic park/switch protocol so another CPU cannot run a thread before its old CPU has finished switching away.

`thread_join` now checks the target state and registers the joining thread as blocked in one scheduler critical section. Previously the target could exit between separate critical sections and leave the joiner blocked forever. The userspace thread test now creates and joins 64 short-lived threads after its two-thread test; this passed under QEMU with one and four configured CPUs. This tests the BSP join path, not execution of ordinary threads on APs.

## EXPERIMENTAL / PLANNED

Timer preemption is tested for user threads on the BSP only. The shared ready queue now dispatches diagnostic PID 0 kernel workers to pinned APs; QEMU with four CPUs passed an atomic start-barrier and repeated cooperative-yield test on three APs. User and ordinary kernel threads remain pinned to CPU 0. There is no general CPU affinity API, AP timer preemption, priority policy or real-time guarantee.

## IMPLEMENTED

Nova uses a FIFO ready queue and round-robin scheduling. x86_64 assembly saves RSP, RBX, RBP and R12–R15, then restores those registers from the next context and returns on its kernel stack. Rust owns run-queue and state transitions. Each thread has a dedicated kernel stack. `SYS_YIELD` queues the current thread at the tail and switches to the scheduler's host context. `SYS_EXIT` removes the thread from runnable execution permanently.

A QEMU self-test runs two kernel threads and asserts A1, B1, A2, B2, A3, B3. A userspace `yieldtest` resumes after SYS_YIELD. A thread may block on child wait, TTY input or sleep and is woken by child exit, a keyboard line or timer ticks. The scheduler restores each process PML4 before resuming a user thread and the kernel PML4 after returning to its host loop. TSS.RSP0 and the SYSCALL kernel-stack slot are updated for every scheduled thread.

## PLANNED

General SMP placement for user processes, AP timer preemption, priorities, context switching of extended CPU state, and kernel-stack guard pages. Interrupts are disabled during kernel context transfer and SYSCALL handling to avoid an IRQ overwriting an active syscall frame via TSS.RSP0.
