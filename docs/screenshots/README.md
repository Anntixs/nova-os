# QEMU screenshots

Captured from a real QEMU/OVMF run on 2026-10-19 with `-machine q35 -cpu qemu64 -smp 4 -m 256M`, NovaBoot, the Nova kernel and the VirtIO Block development disk. The images are QEMU `screendump` output converted from PPM to PNG; no screen content was composited or redrawn afterward.

- `qemu-boot.png`: Nova 0.6.0 boot and Ring 3 shell prompt.
- `qemu-userspace.png`: shell output after userspace programs, faults isolated, and process kill.
- `qemu-nmi.png`: fatal NMI diagnostic after a QEMU monitor `nmi` command. This is an intentionally triggered diagnostic, not normal boot output.

The AP startup log reports three additional processors entering Nova code. The scheduler still runs user threads only on the BSP; these screenshots do not demonstrate SMP scheduling or hardware support outside QEMU.
