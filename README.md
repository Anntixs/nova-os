# Nova OS 0.6.0 Foundation

Nova OS is an independent x86_64 UEFI operating system with NovaBoot and a freestanding Rust `no_std` kernel. The kernel mounts a real FAT32 disk and starts `/system/init` as Ring 3 PID 1. Init starts `/bin/nsh`, a Ring 3 command shell. Independent C and Rust ELF programs use Nova syscalls; they do not use a host OS at runtime.

## Build and run

Install Rust targets `x86_64-unknown-uefi` and `x86_64-unknown-none`, GCC, binutils, Python 3, QEMU x86_64 and OVMF. Run `./scripts/run.sh`; it builds NovaBoot, kernel, Nova libc/CRT, userspace ELFs and a 64 MiB raw FAT32 system disk. The ESP is in `build/esp`. `/home` and `/tmp` survive image rebuilds; `/system/init` and `/bin` are refreshed from source. The scripts accept `NOVA_QEMU`, `OVMF_CODE`, `OVMF_VARS`, `NOVA_RAM` and `NOVA_PROFILE=debug|release`.

Try `ls /dev`, `ls /proc`, `sysinfo`, `ps`, `uptime`, `dmesg`, `memtest`, `schedtest`, `pipetest`, `msgtest`, `hello-c` and `args one two "hello nova"` in `nsh`. `cd`, `pwd`, `clear`, `help`, `env`, `kill` and `exit` are builtins; filesystem, diagnostic and test commands are independent userspace ELF files. The former kernel shell is retained as an emergency monitor; build with `NOVA_MONITOR=1 ./scripts/build.sh` to enter it directly.

`gsattack` is a Ring 3 regression that loads a null GS selector, exercises Nova syscalls and remains active across timer interrupts. It checks the CPU-local syscall stack entry path; it does not imply that APs schedule user threads yet.

Run `python3 scripts/test-init.py` for the normal QEMU boot and userspace regression. The 0.4 monitor regression is `NOVA_MONITOR=1 python3 scripts/test-files.py`; rebuild normally afterward. `./scripts/nova-cc userspace/c/hello-c.c -o build/my-hello.elf` compiles an ordinary C `main` against Nova's sysroot and static libc. See [TOOLCHAIN.md](docs/TOOLCHAIN.md) for its exact limitations.

Run `./scripts/package-image.sh` to build a downloadable QEMU archive in `build/`. It contains separate raw FAT32 EFI and Nova system disks, a `run-qemu.sh` launcher and SHA-256 checksums. A clean packaged system disk is created without overwriting writable data in the normal development image. It is a QEMU/OVMF development image, not a single GPT disk or a tested physical-hardware installer.

The same packaging command also creates `build/nova-os-0.6.0-qemu-x86_64-windows.zip` with `run-qemu-windows.bat`. On Windows, extract the ZIP, install QEMU for Windows and double-click the BAT file. It searches for QEMU and matching OVMF firmware; if OVMF is missing, place `OVMF_CODE_4M.fd` and `OVMF_VARS_4M.fd` beside the BAT. The BAT is a launcher for QEMU; the operating system still boots from the two IMG files.

The [prebuilt Nova 0.6.0 QEMU archive](releases/nova-os-0.6.0-qemu-x86_64.tar.gz) and its [SHA-256 checksum](releases/nova-os-0.6.0-qemu-x86_64.tar.gz.sha256) are checked in for direct download. This release reached the Ring 3 `nsh` prompt when booted from both packaged images with QEMU/OVMF and four virtual CPUs.

For Windows, download the [ZIP with BAT launcher](releases/nova-os-0.6.0-qemu-x86_64-windows.zip) and its [SHA-256 checksum](releases/nova-os-0.6.0-qemu-x86_64-windows.zip.sha256). Extract it, then run `run-qemu-windows.bat`. QEMU and matching OVMF firmware must be installed separately. The BAT launcher has been inspected and packaged but has not been executed on Windows; the same ZIP's disk images booted to `nova:/$` under QEMU/OVMF on Linux.

Foundation adds timer preemption on one active CPU, user threads, orphan reaping, pipes, message channels, `/dev`, `/proc`, input event records, a controlled framebuffer drawing API and a bounded kernel log. See [ARCHITECTURE.md](docs/ARCHITECTURE.md), [IPC.md](docs/IPC.md), [GRAPHICS.md](docs/GRAPHICS.md), [INPUT.md](docs/INPUT.md) and [SYSCALLS.md](docs/SYSCALLS.md). SMP, a mouse driver, a display server and Nova UI are later work.

Experimental multicore bring-up can be observed with `NOVA_SMP=4 ./scripts/run.sh` and `cpuinfo` in nsh. NovaBoot reserves a low SIPI page; APs enter Nova long-mode code, load their own GDT/TSS/IDT and idle. **Only the BSP schedules tasks.** This is not yet SMP task scheduling. See [SMP.md](docs/SMP.md) and [HARDWARE_SUPPORT.md](docs/HARDWARE_SUPPORT.md).
