# IPC (Foundation)

## IMPLEMENTED

`PIPE` (syscall 19) writes two little-endian `u64` file descriptors to a validated userspace buffer: read end then write end. A pipe is a 4096-byte FIFO. Empty reads block until data or all writers close; full writes block until space or all readers close. A read after the final writer closes returns zero. A write after the final reader closes returns `EPIPE=-32`. Descriptors inherited through spawn reference the same pipe. `read` and `write` may return a partial count.

`CHANNEL_CREATE` (20) creates one bidirectional message endpoint. Flag 0 blocks, flag 1 returns `EAGAIN=-11` when empty/full. `CHANNEL_SEND` (21) and `CHANNEL_RECEIVE` (22) preserve each message boundary; max message 4096 bytes, queue depth 32. A receive buffer too small returns `EMSGSIZE=-90` without discarding the message. Channel FDs inherit through spawn. User buffers are copied into kernel memory and validated before queue access.

The queue logic uses short atomic critical sections with local interrupts masked and scheduler wakeups; blocked processes do not poll. The scheduler records wakeups delivered while a waiter is still running, closing the wait registration to park gap on the BSP. Tested by `/bin/pipetest` and `/bin/msgtest` under QEMU.

## EXPERIMENTAL / PLANNED

Only the bootstrap CPU schedules threads. Atomic queue locking alone does not make blocking/wakeup safe across scheduler CPUs; the thread handoff and process lifetime rules remain BSP-only. Channels have no named endpoint discovery, rights transfer or peer-close indication. Those are needed before using them for a display server. Pipes have no descriptor duplication or shell redirection syntax yet.
