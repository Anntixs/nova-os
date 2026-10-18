#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v gcc >/dev/null || { echo 'GCC is required for the Nova C cross build' >&2; exit 1; }
command -v ar >/dev/null || { echo 'ar is required for Nova libc.a' >&2; exit 1; }
mkdir -p build/sysroot/include build/sysroot/lib build/c-obj build/userspace
cp -R userspace/libnova/include/. build/sysroot/include/
python3 scripts/gen-abi-header.py
FLAGS=(-ffreestanding -fno-builtin -fno-stack-protector -fno-pic -fno-pie -mno-red-zone -mcmodel=large -O2 -Wall -Wextra -Ibuild/sysroot/include)
for source in syscall string malloc stdio; do
  gcc "${FLAGS[@]}" -c "userspace/libnova/src/$source.c" -o "build/c-obj/$source.o"
done
gcc "${FLAGS[@]}" -c userspace/libnova/src/crt0.S -o build/sysroot/lib/crt0.o
ar rcs build/sysroot/lib/libc.a build/c-obj/syscall.o build/c-obj/string.o build/c-obj/malloc.o build/c-obj/stdio.o
for source in userspace/c/*.c; do
  name="$(basename "$source" .c)"
  gcc "${FLAGS[@]}" -c "$source" -o "build/c-obj/$name.o"
  ./scripts/nova-cc "build/c-obj/$name.o" -o "build/userspace/$name.elf"
done
