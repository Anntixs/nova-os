# ACPI topology

## IMPLEMENTED / EXPERIMENTAL

NovaBoot passes the UEFI ACPI RSDP physical address. `kernel/src/platform/acpi.rs` validates the RSDP signature/checksum, extended RSDP checksum for revision 2+, XSDT/RSDT signature and checksum, and MADT checksum/record bounds. Before dereferencing a table, it checks that the full range fits a memory-backed UEFI descriptor and every page is identity mapped. It parses currently enabled (MADT flag bit 0) xAPIC processor records, one I/O APIC and IRQ0/IRQ1 source overrides. Bit 1, online-capable, does not authorize Nova to start a disabled CPU without an ACPI hotplug path. At most 64 processor records are retained; duplicate APIC IDs are ignored. The APIC driver receives the parsed topology instead of parsing ACPI itself. QEMU with `-smp 4` exposed APIC IDs 0–3, and `-smp 8` exposed 0–7.

Firmware physical addresses are still dereferenced through NovaBoot's retained identity map. The range check requires each table to fit one UEFI descriptor, which may reject unusual firmware layouts. x2APIC-only processor entries, multiple I/O APIC routing domains and ACPI AML are not supported.
