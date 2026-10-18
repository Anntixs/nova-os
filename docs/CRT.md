# Nova C runtime startup

## IMPLEMENTED

`userspace/libnova/src/crt0.S` defines ELF `_start`. It receives Nova's user stack header (`argc`, `argv`, `envp` pointers), aligns the stack for System V x86_64 calls, invokes `main(argc, argv)`, and passes its return code to Nova `EXIT`. `crt0.o` is placed into `build/sysroot/lib`; Nova does not link Linux CRT objects. The linker script `userspace/hello/linker.ld` gives executable read-only text/rodata and writable non-executable data/BSS PT_LOAD segments.

## PLANNED

C++ constructors/destructors, TLS initialization, dynamic loader entry, auxiliary vectors and standard process startup extensions. Programs using this CRT are statically linked ET_EXEC files.
