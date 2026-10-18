# ELF64 loader

## IMPLEMENTED

Nova's kernel validates ELF magic, class 64, little-endian encoding, x86_64 machine, ET_EXEC type, program-header bounds, PT_LOAD file/memory bounds, user-slot addresses and segment alignment. It rejects writable plus executable segments, zero-fills each mapped frame, copies file data and leaves BSS zeroed. It maps PF_W as writable and omits NX only for PF_X segments when NX is supported. The entry must lie in an executable LOAD segment. A two-page USER writable stack is mapped separately and marked NX when supported.

The build compiles each program to a separate ELF file and embeds those bytes. Parsing and mapping happen when the shell runs a program. No function pointer into kernel code stands in for the ELF. Each process gets its own page tables; cleanup walks only the owned user PML4 slot.

## LIMITATIONS

No PIE, relocation, PT_INTERP, dynamic linker, TLS, argv/environment, overlapping LOAD segments or shared libraries. The loader rejects overlapping mapped pages during mapping. The fixed 64 TiB user slot is an initial layout, not an ABI guarantee.
