# Kernel locking status

## IMPLEMENTED / EXPERIMENTAL

The physical-frame allocator and kernel heap use atomic compare-exchange spinlocks with local interrupts disabled. The process table and scheduler use the same exclusion for state operations and keep CPU-local current slots. Pipe and message-channel queues have per-object atomic locks. The VirtIO block registry serializes access to its queue and DMA buffers across CPUs. FAT32 filesystem operations hold a volume transaction lock, so a read cannot observe a concurrent partial FAT update. These operations restore the caller's previous IF state after releasing the lock. A diagnostic build (`NOVA_SMPTEST=1`) has APs concurrently allocate, verify and free frames and heap buffers; QEMU with four CPUs passed 256 iterations on each AP.

## NOT YET SMP-SAFE

The mount table is installed on the BSP and frozen before AP startup; each mounted filesystem must implement `Send + Sync`. Runtime mounting remains unsupported. FAT32 and block spinlocks currently span synchronous disk I/O, so callers on other CPUs may spin for an entire request. TTY, input queues, logging, graphics, paging and process lifetime still need a complete cross-CPU audit before APs may run arbitrary threads. No blocking kernel mutex, generic semaphore, completion or wait-queue abstraction is implemented. Local IF masking does not stop another CPU. Stage B drivers must not be started on APs until these foundations are corrected and stress-tested.

Current observed lock order is scheduler → process manager for scheduling transitions; process exit drops descriptor references and wakes waiters after releasing the process lock. Code changes must preserve this ordering or replace it with explicit non-nested state transitions.

`thread_join` performs the target-state check and waiter registration while holding the scheduler lock. The CPU now records a pending state transition and completes it only after the context switch has returned to its host stack. `yield`, `block`, thread exit and join use this handoff; the ready queue cannot expose a stack still active on the old CPU. This passed BSP regression tests, but cross-CPU scheduling is not enabled or validated. Other global subsystems remain blockers.
