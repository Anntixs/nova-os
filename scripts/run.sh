#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$ROOT/scripts/build.sh"
QEMU_BIN="${NOVA_QEMU:-qemu-system-x86_64}"
command -v "$QEMU_BIN" >/dev/null || { echo 'qemu-system-x86_64 is required' >&2; exit 1; }
OVMF_CODE="${OVMF_CODE:-}"
if [[ -z "$OVMF_CODE" ]]; then
  for candidate in /usr/share/OVMF/OVMF_CODE.fd /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/edk2/ovmf/OVMF_CODE.fd /usr/share/edk2/x64/OVMF_CODE.4m.fd; do
    if [[ -f "$candidate" ]]; then OVMF_CODE="$candidate"; break; fi
  done
fi
[[ -f "$OVMF_CODE" ]] || { echo 'OVMF firmware missing; set OVMF_CODE=/path/to/OVMF_CODE.fd' >&2; exit 1; }
OVMF_VARS="${OVMF_VARS:-${OVMF_CODE/CODE/VARS}}"
[[ -f "$OVMF_VARS" ]] || { echo 'OVMF variable template missing; set OVMF_VARS=/path/to/OVMF_VARS.fd' >&2; exit 1; }
cp "$OVMF_VARS" "$ROOT/build/OVMF_VARS.fd"
export TMPDIR="$ROOT/build"
QEMU_DATA_ARGS=()
if [[ -n "${NOVA_QEMU_DATADIR:-}" ]]; then QEMU_DATA_ARGS=(-L "$NOVA_QEMU_DATADIR"); fi
exec "$QEMU_BIN" "${QEMU_DATA_ARGS[@]}" -machine q35 -m "${NOVA_RAM:-256M}" -cpu qemu64 -smp 1 \
  -drive "if=pflash,format=raw,readonly=on,file=$OVMF_CODE" \
  -drive "if=pflash,format=raw,file=$ROOT/build/OVMF_VARS.fd" \
  -drive "format=raw,file=fat:rw:$ROOT/build/esp" \
  -drive "if=none,id=nova-system,format=raw,file=$ROOT/build/nova-system.img" \
  -device virtio-blk-pci,drive=nova-system,disable-modern=on \
  -device VGA -display "${NOVA_DISPLAY:-gtk}" -serial stdio -no-reboot "${@}"
