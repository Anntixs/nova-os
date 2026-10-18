# Nova libc foundation

## IMPLEMENTED

`userspace/libnova/` is a static, freestanding C library. Its public headers include `stddef.h`, `stdint.h`, `stdbool.h`, `limits.h`, `string.h`, `stdlib.h`, `stdio.h`, and `nova/{syscall,process,fs}.h`. The library implements `memcpy`, `memmove`, `memset`, `memcmp`, `strlen`, `strcmp`, `strncmp`, `strcpy`, `strncpy`, `strchr`, `malloc`, `free`, `calloc`, `realloc`, `atoi`, `strtol`, `abort`, `exit`, `putchar`, `puts`, `printf`, `fwrite`, plus Nova syscall wrappers. `printf` handles %s, %c, %d, %u, %x, %p and %%. Output uses FD 1; `stderr` is FD 2. `malloctest` executes repeated allocations, zero-fill, realloc preservation and frees in Ring 3.

The allocator obtains anonymous writable/non-executable pages through `MAP`, then reuses freed blocks and splits larger blocks. It does not return spare pages to the kernel. It is process local, single-threaded, and does not detect an invalid or double `free`.

## PLANNED

Full ISO C/POSIX coverage, buffered FILE streams, floating-point printf, locale, thread safety, standard errno, C++ runtime and dynamic libraries. Programs must only call functions present in Nova headers.
