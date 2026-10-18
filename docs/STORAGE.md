# Storage in Nova OS 0.4.0

## IMPLEMENTED

The QEMU configuration attaches two independent disks. NovaBoot runs from the firmware-visible ESP (`build/esp`, exposed by QEMU's FAT directory backend). A second 64 MiB raw FAT32 volume is `build/nova-system.img`. It has no MBR/GPT partition table: sector zero is the FAT32 volume boot record. The kernel mounts this volume at `/`. NovaBoot uses the UEFI Loaded Image device handle to locate its own ESP; adding a second FAT filesystem cannot redirect the kernel lookup.

The data path is `VirtIO PCI → BlockDevice → FAT32 → VFS → file descriptor/syscall or NovaShell`. Only the VirtIO Block backend is implemented. The driver polls one legacy virtqueue and uses a fixed DMA bounce buffer. PCI interrupts are disabled for this device. QEMU uses `virtio-blk-pci,disable-modern=on`. The driver checks the advertised queue size and capacity and uses only 512-byte sectors. All block operations are serialized on the bootstrap CPU. There is no block cache.

`scripts/build.sh` compiles the user ELF programs, then regenerates the FAT32 volume. The image builder carries existing `/home`, `/system`, and `/tmp` trees into the rebuilt image, while refreshing `/bin` and `/etc`. The latter are build-owned. The builder uses Python's standard library and requires no host FAT formatter. Direct disk changes persist across QEMU restarts; rebuilding also preserves the three writable trees. The image is for development, not a crash-consistent installer.

## EXPERIMENTAL

FAT32 writes are synchronous and update data, FAT copies and directory entries without journaling. A power loss during a multi-sector operation may leave a damaged volume. Filesystem mutation uses the current single-CPU serialization; SMP requires explicit locking. The raw FAT32 volume is capped at 64 MiB. VFS also mounts generated `/dev` and `/proc` filesystems; `fsinfo` free-space hints are not maintained.

## PLANNED

GPT partition parsing, storage discovery policy, NVMe/AHCI/USB drivers, asynchronous I/O, caching, journaling or a native NovaFS, and multiple mounts.
