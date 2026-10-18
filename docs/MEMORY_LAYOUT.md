# Virtual address layout

## IMPLEMENTED

| Range | Use |
| --- | --- |
| `0x0000000000200000` upward | Kernel ELF, identity mapped, supervisor-only in process PML4 |
| Other firmware lower mappings | Retained for kernel bootstrap; PML4 USER bit cleared in every process |
| `0x0000400000000000`–`0x0000407fffffffff` | Dedicated user PML4 slot (index 128) |
| `0x0000400000000000` upward | Fixed-address user ELF PT_LOAD segments |
| `0x000040000fffe000`–`0x000040000fffffff` | Two user stack pages, writable and NX when supported |
| `0xffff900000000000`–`0xffff9000000fffff` | 1 MiB kernel heap, supervisor-only |
| `0xffff920000000000` / `0xffff920000001000` | Local APIC / IOAPIC MMIO, supervisor-only |

The exact stack top is `0x0000400010000000`. Startup builds argc, argv and envp arrays/string data on the two mapped pages and sets `RSP mod 16 = 8`. The adjacent lower page is unmapped, but this is **not** a complete kernel-stack guard-page design. User segments must fit wholly in the dedicated PML4 slot. Anonymous `MAP` allocations occupy `0x0000400020000000` upward within a 256 MiB window; each mapping is private, USER and NX unless executable protection is requested. W+X is rejected. User page table entries are USER at all four levels. Kernel slots are cloned from the permanent supervisor PML4 with their root USER bit cleared. The initial kernel mapping is still identity-based, so this is not a complete higher-half kernel.

Only UEFI ConventionalMemory frames (type 7), excluding the first MiB and kernel image, enter the frame allocator. LoaderData, BootServices, ACPI, MMIO and framebuffer memory remain reserved. The allocator currently covers up to 64 GiB. The process address space owns all tables and leaf frames under its user PML4 slot and releases them after switching back to the kernel PML4.

## PLANNED

Kernel stack guard pages, ASLR, KASLR, removal of firmware identity mappings, SMEP/SMAP enforcement and a full W^X audit of kernel mappings. ELF user code is not writable; writable user data and stack are NX if CPUID supports NX.
