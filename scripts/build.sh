#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v cargo >/dev/null || { echo 'Rust cargo is required. Install rustup and the x86_64-unknown-uefi and x86_64-unknown-none targets.' >&2; exit 1; }
command -v rustup >/dev/null || { echo 'rustup is required to manage Nova target toolchains.' >&2; exit 1; }
for target in x86_64-unknown-uefi x86_64-unknown-none; do
  rustup target list --installed | grep -qx "$target" || { echo "Missing Rust target: $target (rustup target add $target)" >&2; exit 1; }
done
PROFILE="${NOVA_PROFILE:-release}"
case "$PROFILE" in release) PROFILE_ARG=(--release);; debug) PROFILE_ARG=();; *) echo "NOVA_PROFILE must be release or debug" >&2; exit 1;; esac
cargo build "${PROFILE_ARG[@]}" -p nova-boot --target x86_64-unknown-uefi
KERNEL_FEATURES=()
if [[ "${NOVA_SELFTEST:-0}" == "1" ]]; then KERNEL_FEATURES+=(--features selftest); fi
if [[ "${NOVA_EXCEPTIONTEST:-0}" == "1" ]]; then KERNEL_FEATURES+=(--features exceptiontest); fi
if [[ "${NOVA_MONITOR:-0}" == "1" ]]; then KERNEL_FEATURES+=(--features monitor); fi
RUSTFLAGS="${RUSTFLAGS:-} -C relocation-model=static -C code-model=large -C link-arg=-T$ROOT/userspace/hello/linker.ld" cargo build "${PROFILE_ARG[@]}" -p nova-user-programs --target x86_64-unknown-none
mkdir -p build/userspace
for program in sleeptest maptest spawntest filetest hello exit42 badmem badopcode yieldtest invalidptr invalidsys badpriv badcodewrite badstackexec badkernel; do
  cp "target/x86_64-unknown-none/$PROFILE/$program" "build/userspace/$program.elf"
  if [[ "$PROFILE" == "debug" ]]; then
    command -v strip >/dev/null || { echo 'GNU strip is required for compact debug user ELF embedding' >&2; exit 1; }
    strip --strip-debug "build/userspace/$program.elf"
  fi
done
RUSTFLAGS="${RUSTFLAGS:-} -C relocation-model=static -C link-arg=-T$ROOT/kernel/linker.ld" cargo build "${PROFILE_ARG[@]}" -p nova-kernel --target x86_64-unknown-none "${KERNEL_FEATURES[@]}"
./scripts/build-c.sh
mkdir -p build/esp/EFI/BOOT build/esp/EFI/NOVA
python3 scripts/make-system-image.py
cp target/x86_64-unknown-uefi/$PROFILE/nova-boot.efi build/esp/EFI/BOOT/BOOTX64.EFI
cp target/x86_64-unknown-none/$PROFILE/nova-kernel build/esp/EFI/NOVA/KERNEL.ELF
echo 'Nova ESP staged at build/esp'
