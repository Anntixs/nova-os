# Nova syscalls

## IMPLEMENTED (ABI 0.3)

All calls return `i64` in RAX: nonnegative success, negative Nova error. See [ABI.md](ABI.md) for the register convention. The exact numbers come from `libs/nova-core/src/abi.rs`.

| Number | Call | RDI | RSI | RDX | Success |
|---:|---|---|---|---|---|
| 1 | WRITE | fd | user buffer | length | bytes written |
| 2 | EXIT | code | — | — | never returns |
| 3 | GETPID | — | — | — | PID |
| 4 | YIELD | — | — | — | 0 |
| 5 | OPEN | path | flags | — | FD |
| 6 | CLOSE | FD | — | — | 0 |
| 7 | READ | FD | user buffer | length | bytes read |
| 8 | SEEK | FD | signed offset | whence | new offset |
| 9 | STAT | FD | user stat buffer | — | 0 |
| 10 | READDIR | FD | user entry buffer | buffer size | 1 entry, 0 EOF |
| 11 | SPAWN | path | argv pointer array | envp pointer array | child PID |
| 12 | WAIT | child PID | optional `i32*` status | — | child PID |
| 13 | GETCWD | user buffer | capacity | — | pathname bytes |
| 14 | CHDIR | path | — | — | 0 |
| 15 | MAP | length | protection | flags (must be 0) | user address |
| 16 | UNMAP | original address | original length | — | 0 |
| 17 | CLOCK_GET | — | — | — | uptime milliseconds |
| 18 | SLEEP | milliseconds | — | — | 0 after wake |

`OPEN` flags: 0 read, 1 write, 2 read/write, plus `0x100` create or `0x200` truncate. `SEEK` whence is 0 start, 1 current, 2 end. `READ` and `WRITE` cap a single call at 65536 bytes. `WAIT`, `SLEEP` and empty TTY `READ` block their thread; timer and keyboard events wake it. `WAIT` reaps a direct child once. Unknown calls return `-38` (`ENOSYS`). Invalid user pointers return `-14` (`EFAULT`); other stable errors include `ENOENT=-2`, `EIO=-5`, `EBADF=-9`, `ENOMEM=-12`, `EINVAL=-22`, `ENOTSUP=-95`. These are numeric wire values, never Rust enum layouts.

| 19 | PIPE | pointer to two `u64` FDs | — | — | 0 |
| 20 | CHANNEL_CREATE | flags: 0 blocking, 1 nonblocking | — | — | FD |
| 21 | CHANNEL_SEND | FD | message pointer | byte count | bytes sent |
| 22 | CHANNEL_RECEIVE | FD | output pointer | capacity | bytes received |
| 23 | THREAD_CREATE | entry address | user stack top | — | TID |
| 24 | THREAD_EXIT | — | — | — | never returns |
| 25 | THREAD_JOIN | TID | — | — | 0 |
| 26 | FB_INFO | output pointer to 24 bytes | — | — | 0 |
| 27 | FB_ACQUIRE | — | — | — | 0 |
| 28 | FB_RELEASE | — | — | — | 0 |
| 29 | FB_FILL | pointer to 20-byte rectangle/color | — | — | 0 |
| 30 | FB_BLIT | pointer to 32-byte blit request | — | — | 0 |
| 31 | TTY_CLEAR | — | — | — | 0 |
| 32 | KILL | direct child PID | exit status | — | 0 |
| 33 | WAIT_TRY | direct child PID | optional `i32*` status | — | child PID or `EAGAIN` |

These are the implemented Foundation extensions to the original ABI 0.3 table above. `WAIT(0)` is reserved for init's adopted-orphan reaper. Pipes carry a bounded byte stream; channels preserve message boundaries. `THREAD_CREATE` starts a new thread in the caller's address space and requires an already mapped userspace stack. `FB_*` uses a single process lease. See [IPC.md](IPC.md) and [GRAPHICS.md](GRAPHICS.md).

## EXPERIMENTAL

The scheduler preempts at timer ticks on the sole active CPU. There is no SMP-safe synchronization, general process cancellation, descriptor duplication, file mapping or timeout variant of `WAIT`.
