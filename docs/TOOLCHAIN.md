# Nova C cross-development path

## IMPLEMENTED

The logical target is `x86_64-unknown-nova`. The installed GCC does not recognize this as a formal target. `scripts/nova-cc` is an explicit temporary compiler driver: it invokes GCC with freestanding x86_64 flags, Nova headers, Nova `crt0.o`, Nova `libc.a`, Nova's userspace linker script, static linking and `-nostdlib`. It never requests host startup files or glibc. Run:

```sh
./scripts/build.sh
./scripts/nova-cc userspace/c/hello-c.c -o build/my-hello.elf
readelf -l build/my-hello.elf
```

The QEMU regression executes the packaged `/bin/hello-c`. `readelf -l` on the wrapper-built ELF shows two PT_LOAD segments (R E and RW), no PT_INTERP and no dynamic loader. `readelf -d` reports no dynamic section. The wrapper uses the host GCC as a build tool; Nova itself has no host dependency at runtime.

## PLANNED

An upstream Clang/LLVM target, Nova-specific compiler driver with complete options, sysroot-aware C++ toolchain, exception ABI, RTTI, libstdc++/libc++, and linker integration beyond static ET_EXEC.
