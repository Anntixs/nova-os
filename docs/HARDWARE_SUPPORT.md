# Hardware support status

## TESTED UNDER QEMU/OVMF

NovaBoot UEFI, GOP framebuffer, xAPIC/IOAPIC/PIT, PS/2 keyboard, legacy VirtIO Block, raw FAT32, Ring 3 userspace and AP startup probe. QEMU `-smp 1`, `-smp 2`, `-smp 4` and `-smp 8` have passed the normal userspace regression and a diagnostic AP kernel-worker scheduler test. APs run pinned kernel workers and otherwise idle; user scheduling remains on the BSP.

## IMPLEMENTED BUT UNTESTED ON PHYSICAL HARDWARE

RSDP/XSDT/RSDT/MADT parsing and xAPIC AP startup have only been tested against QEMU's ACPI tables and firmware behavior. Physical x86_64 UEFI machines may have x2APIC-only records, multiple IOAPICs, unusual low memory maps or different interrupt routing.

## PLANNED

SMP-safe scheduling and process ownership, remote TLB shootdown, PCIe capability and MSI/MSI-X support, DMA abstraction, GPT, NVMe, AHCI, xHCI/USB, HID and platform power management. No support for these should be inferred from the existing PCI scan or VirtIO driver.
