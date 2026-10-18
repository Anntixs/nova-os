# Nova sysroot

## IMPLEMENTED

`scripts/build-c.sh` creates `build/sysroot/include` from `userspace/libnova/include` and `build/sysroot/lib/{crt0.o,libc.a}`. It generates `include/nova/abi_numbers.h` from the kernel's shared Rust ABI definitions. The same build compiles all C sources in `userspace/c` into static ELF files and installs them into the FAT32 image. `init.elf` goes to `/system/init`; other C ELFs go to `/bin`.

The sysroot is a build artifact and can be regenerated. There is no host libc header or library in its link path.

## PLANNED

Versioned target triples, pkg-config metadata, C++ headers/libraries, dynamic libraries and a package manager.
