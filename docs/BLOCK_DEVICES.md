# Block devices

## IMPLEMENTED

`kernel/src/block/mod.rs` defines `BlockDevice` with `block_size`, `block_count`, `read_blocks`, and `write_blocks`. Callers receive explicit errors for unavailable devices, invalid buffers, range errors, timeout and I/O failures. FAT32 only uses this interface and does not know the PCI BAR or QEMU configuration.

`kernel/src/block/virtio.rs` implements one legacy VirtIO Block device. It negotiates no optional features, sets up the device's advertised queue (up to 1024 descriptors), submits a three-descriptor request, polls completion, and copies through a pinned 4 KiB DMA buffer. It supports read and write requests one 512-byte sector at a time. The static queue is 32 KiB and is 4 KiB aligned. The device reports its real sector count at boot.

## EXPERIMENTAL

The current registry has one device named internally `vda`. A single BSP executes I/O with interrupts masked. The request poll has a finite timeout but no reset/recovery path. DMA addresses rely on Nova's current physical identity mapping and legacy VirtIO's 32-bit queue PFN. The driver intentionally does not use transport interrupts or modern VirtIO PCI capabilities.

## PLANNED

Multiple block devices, DMA allocation independent of identity mappings, request queues, retries and modern VirtIO, NVMe and AHCI.
