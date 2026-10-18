#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
QEMU_BIN="${NOVA_QEMU:-qemu-system-x86_64}"
command -v "$QEMU_BIN" >/dev/null || { echo 'QEMU is required; set NOVA_QEMU if needed' >&2; exit 1; }
command -v timeout >/dev/null || { echo 'GNU timeout is required' >&2; exit 1; }
OVMF_CODE="${OVMF_CODE:-}"
if [[ -z "$OVMF_CODE" ]]; then
  for candidate in /usr/share/OVMF/OVMF_CODE.fd /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/edk2/ovmf/OVMF_CODE.fd; do
    if [[ -f "$candidate" ]]; then OVMF_CODE="$candidate"; break; fi
  done
fi
[[ -f "$OVMF_CODE" ]] || { echo 'Set OVMF_CODE to an OVMF code image' >&2; exit 1; }
OVMF_VARS="${OVMF_VARS:-${OVMF_CODE/CODE/VARS}}"
[[ -f "$OVMF_VARS" ]] || { echo 'Set OVMF_VARS to an OVMF variable template' >&2; exit 1; }
QEMU_DATA_ARGS=()
if [[ -n "${NOVA_QEMU_DATADIR:-}" ]]; then QEMU_DATA_ARGS=(-L "$NOVA_QEMU_DATADIR"); fi
run_case() {
  local log="$1"
  cp "$OVMF_VARS" build/OVMF_VARS.fd
  export TMPDIR="$ROOT/build"
  timeout 10s "$QEMU_BIN" "${QEMU_DATA_ARGS[@]}" -machine q35 -m "${NOVA_RAM:-256M}" -cpu qemu64 -smp 1 \
    -drive "if=pflash,format=raw,readonly=on,file=$OVMF_CODE" \
    -drive "if=pflash,format=raw,file=$ROOT/build/OVMF_VARS.fd" \
    -drive "format=raw,file=fat:rw:$ROOT/build/esp" \
    -drive "if=none,id=nova-system,format=raw,file=$ROOT/build/nova-system.img" \
    -device virtio-blk-pci,drive=nova-system,disable-modern=on -device VGA -display none \
    -serial "file:$log" -no-reboot || [[ "$?" -eq 124 ]]
}
NOVA_MONITOR=1 NOVA_SELFTEST=1 NOVA_EXCEPTIONTEST=0 ./scripts/build.sh
run_case "$ROOT/build/test-memory.log"
rg -q 'NOVA_TEST_SUCCESS memory paging heap' build/test-memory.log
rg -q 'user address space isolation and cleanup OK' build/test-memory.log
rg -q 'ELF validation and mapping OK' build/test-memory.log
rg -q 'scheduler round robin OK' build/test-memory.log
rg -q 'Kernel initialization complete' build/test-memory.log
if rg -q 'NOVA KERNEL PANIC|NOVA KERNEL EXCEPTION' build/test-memory.log; then echo 'Unexpected kernel failure in memory test' >&2; exit 1; fi
NOVA_MONITOR=1 NOVA_SELFTEST=0 NOVA_EXCEPTIONTEST=1 ./scripts/build.sh
run_case "$ROOT/build/test-exception.log"
rg -q 'Exception: INVALID OPCODE' build/test-exception.log
rg -q 'System halted.' build/test-exception.log
NOVA_SELFTEST=0 NOVA_EXCEPTIONTEST=0 ./scripts/build.sh
echo 'NOVA_TEST_SUCCESS QEMU memory and exception suites'
