# Nova ABI 0.3

## Foundation extensions (experimental)

Syscall numbers 1–18 are retained. Implemented additions 19–33 cover pipes, message channels, userspace threads, controlled framebuffer drawing, TTY clear, direct-child termination and nonblocking child wait. All return signed 64-bit RAX values (nonnegative success, negative error). `EAGAIN=-11`, `EFAULT=-14`, `EPIPE=-32` and `EMSGSIZE=-90` are wire numbers; no Rust enum layout crosses the boundary. See [SYSCALLS.md](SYSCALLS.md), [IPC.md](IPC.md), [GRAPHICS.md](GRAPHICS.md) and [INPUT.md](INPUT.md). The ABI remains experimental and x86_64 specific.

## IMPLEMENTED (experimental ABI)

Ring 3 uses x86_64 `syscall`: RAX holds the call number, RDI/RSI/RDX hold the three supported arguments, and RAX returns a nonnegative result or a negative error. RCX and R11 are clobbered. The kernel switches to the current thread's trusted kernel stack before dispatch. Call numbers and negative error values have one authoritative Rust source, `libs/nova-core/src/abi.rs`; `scripts/gen-abi-header.py` generates the C header during the build. Numbers 1–10 remain compatible with Nova ABI 0.2. See [SYSCALLS.md](SYSCALLS.md).

The entry stack at `RSP % 16 == 8` contains `u64 argc`, `u64 argv_pointer`, `u64 envp_pointer`; the argv/envp pointer arrays are null terminated and point at null terminated UTF-8 strings on the user stack. CRT `_start` invokes `main(argc, argv)` and calls `exit` with its result. The initial environment contains `PATH=/bin`, `HOME=/home`, and `SHELL=/bin/nsh`; a child inherits its parent's environment when `spawn` receives null envp. Current limits are 32 pointers per array and 255 bytes per string.

File descriptors are per process. FD 0 is blocking canonical TTY input; 1 and 2 write the terminal. FDs 3–63 are FAT32 files. Children start with independent standard descriptors 0–2. General descriptor inheritance is planned. `STAT` is 16 bytes (`u32 kind`, reserved `u32`, `u64 size`); `READDIR` is 272 bytes (`u32 kind`, `u32 name_length`, 264 name bytes). User ranges are checked against USER mappings before kernel access. `MAP` accepts a length, protection bits 1=R, 2=W, 4=X, and zero flags. W+X is rejected. `UNMAP` currently requires the original address and length.

## PLANNED

Formal upstream compiler target `x86_64-unknown-nova`, dynamic linking, a compatibility policy, handle capabilities and additional syscall registers. The GCC wrapper is a logical cross-development path, not upstream target recognition.
