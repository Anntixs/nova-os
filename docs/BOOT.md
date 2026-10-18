# Boot handoff

UEFI loads `EFI/BOOT/BOOTX64.EFI`. NovaBoot obtains the Simple File System from its own Loaded Image device handle and reads `EFI/NOVA/KERNEL.ELF` (maximum 8 MiB) from that ESP. This matters because Nova OS 0.4.0 attaches a second FAT32 disk. NovaBoot checks ELF64 little-endian x86_64 executable headers, allocates each `PT_LOAD` segment at its linked physical address, zeroes its memory, and copies file bytes. It rejects mismatched virtual/physical addresses and invalid ranges. It obtains GOP and the ACPI RSDP if present, allocates dedicated pages for `NovaBootInfo`, the memory map, and a 64 KiB stack, then retrieves the final map and calls `ExitBootServices` with its key. A stale key is retried. After success it switches stacks and jumps to the ELF entry using the System V x86_64 ABI with `rdi` pointing at `NovaBootInfo`.

`NovaBootInfo` is `repr(C)`, version 1, with a magic, byte size, GOP framebuffer geometry and format, raw UEFI memory map pointer/size/descriptor size/version, optional RSDP address, and kernel loaded range. GOP formats 0 and 1 mean RGBX and BGRX; firmware bitmask and blit-only formats are rejected. All pointers are physical addresses under the firmware's identity mapping. The three handoff buffers and stack use UEFI LoaderData pages. They remain allocated after exit and the kernel owns them; later memory management must reserve them before reclaiming loader pages. The map buffer is 256 KiB and the maximum ELF file is 8 MiB. No UEFI service may be called after the successful exit.

The boot protocol's layout is defined only in `libs/nova-core/src/lib.rs`. A kernel checks magic, version, and size before using fields. The RSDP may be zero.

## Verified QEMU boot

On 2026-10-17 the release UEFI and kernel binaries built with Rust 1.99.1. QEMU 10.0.13 with OVMF 2025.02 booted from the staged FAT EFI filesystem. The serial log confirmed UEFI entry, ELF loading, GOP discovery, kernel entry, BootInfo validation, memory map validation, framebuffer initialization, and safe halt. The real VGA screenshot is saved in `build/nova-boot.png`; the captured serial stream is in `build/serial.log`.

## Nova OS 0.3.0 QEMU regression

On 2026-10-18, the staged release image booted with QEMU 10.0.13 and OVMF. `scripts/test.sh` verified memory, paging, heap, address-space cleanup, ELF validation, deterministic round-robin kernel switching, and a deliberate kernel invalid-opcode report. `scripts/test-userspace.py` sent real PS/2 key events through QEMU, ran `cpu`, `mem`, `uptime`, multiple user ELF programs and fault tests, and checked that the shell prompt returned. The serial transcript is `build/test-userspace.log`; QEMU framebuffer captures are `build/nova-hello.png` and `build/nova-0.3.png` after conversion from QEMU's PPM output.

## Nova OS 0.4.0 QEMU regression

`scripts/test-files.py` boots with the dedicated VirtIO FAT32 disk, sends PS/2 commands, reads directories and files, runs `/bin/hello` and `/bin/filetest` in Ring 3, tests user faults, writes a file, creates/removes an empty directory and checks the prompt returns. `--no-build --verify-persistence` starts another QEMU process with the same disk and reads the written file. The serial transcript is `build/test-files.log`; a framebuffer capture is `build/nova-0.4.png`.
