# Nova OS 0.6.0 Foundation

Nova OS is an independent x86_64 UEFI operating system with NovaBoot and a freestanding Rust `no_std` kernel. The kernel mounts a real FAT32 disk and starts `/system/init` as Ring 3 PID 1. Init starts `/bin/nsh`, a Ring 3 command shell. Independent C and Rust ELF programs use Nova syscalls; they do not use a host OS at runtime.

## Build and run

Install Rust targets `x86_64-unknown-uefi` and `x86_64-unknown-none`, GCC, binutils, Python 3, QEMU x86_64 and OVMF. Run `./scripts/run.sh`; it builds NovaBoot, kernel, Nova libc/CRT, userspace ELFs and a 64 MiB raw FAT32 system disk. The ESP is in `build/esp`. `/home` and `/tmp` survive image rebuilds; `/system/init` and `/bin` are refreshed from source. The scripts accept `NOVA_QEMU`, `OVMF_CODE`, `OVMF_VARS`, `NOVA_RAM` and `NOVA_PROFILE=debug|release`.

Try `ls /dev`, `ls /proc`, `sysinfo`, `ps`, `uptime`, `dmesg`, `memtest`, `schedtest`, `pipetest`, `msgtest`, `hello-c` and `args one two "hello nova"` in `nsh`. `cd`, `pwd`, `clear`, `help`, `env`, `kill` and `exit` are builtins; filesystem, diagnostic and test commands are independent userspace ELF files. The former kernel shell is retained as an emergency monitor; build with `NOVA_MONITOR=1 ./scripts/build.sh` to enter it directly.

Run `python3 scripts/test-init.py` for the normal QEMU boot and userspace regression. The 0.4 monitor regression is `NOVA_MONITOR=1 python3 scripts/test-files.py`; rebuild normally afterward. `./scripts/nova-cc userspace/c/hello-c.c -o build/my-hello.elf` compiles an ordinary C `main` against Nova's sysroot and static libc. See [TOOLCHAIN.md](docs/TOOLCHAIN.md) for its exact limitations.

Foundation adds timer preemption on one active CPU, user threads, orphan reaping, pipes, message channels, `/dev`, `/proc`, input event records, a controlled framebuffer drawing API and a bounded kernel log. See [ARCHITECTURE.md](docs/ARCHITECTURE.md), [IPC.md](docs/IPC.md), [GRAPHICS.md](docs/GRAPHICS.md), [INPUT.md](docs/INPUT.md) and [SYSCALLS.md](docs/SYSCALLS.md). SMP, a mouse driver, a display server and Nova UI are later work.
