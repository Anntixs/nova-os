#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

NOVA_PROFILE=release NOVA_MONITOR=0 NOVA_SMPTEST=0 ./scripts/build.sh
VERSION="$(python3 -c 'import tomllib; print(tomllib.load(open("kernel/Cargo.toml", "rb"))["package"]["version"])')"
NAME="nova-os-${VERSION}-qemu-x86_64"
RELEASE_DIR="$ROOT/build/release/$NAME"
ARCHIVE="$ROOT/build/$NAME.tar.gz"
mkdir -p "$RELEASE_DIR"
cp build/nova-efi.img "$RELEASE_DIR/nova-efi.img"
NOVA_FRESH_IMAGE=1 NOVA_SYSTEM_IMAGE_OUTPUT="$RELEASE_DIR/nova-system.img" python3 scripts/make-system-image.py

cat > "$RELEASE_DIR/README.txt" <<'EOF'
Nova OS x86_64 / UEFI QEMU development image

This archive contains two raw FAT32 disks:
  nova-efi.img     UEFI NovaBoot and Nova Kernel
  nova-system.img  Nova system files and userspace programs

Requirements: qemu-system-x86_64 and OVMF_CODE_4M.fd / OVMF_VARS_4M.fd.
Run ./run-qemu.sh from this directory. The script detects common OVMF paths;
set OVMF_CODE and OVMF_VARS if yours differ. Set NOVA_SMP=4 or NOVA_RAM=1G
to change the VM. Set NOVA_DISPLAY=none for headless serial output.

The system image is a fresh development volume. Nova 0.6.0 starts additional
APs, but its user scheduler still runs on the bootstrap CPU. This package
does not claim full SMP or support for physical PCs.
EOF

cat > "$RELEASE_DIR/run-qemu.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
QEMU_BIN="${NOVA_QEMU:-qemu-system-x86_64}"
command -v "$QEMU_BIN" >/dev/null || { echo "Missing QEMU: $QEMU_BIN" >&2; exit 1; }
OVMF_CODE="${OVMF_CODE:-}"
if [[ -z "$OVMF_CODE" ]]; then
  for candidate in /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd /usr/share/edk2/ovmf/OVMF_CODE.fd /usr/share/edk2/x64/OVMF_CODE.4m.fd; do
    if [[ -f "$candidate" ]]; then OVMF_CODE="$candidate"; break; fi
  done
fi
[[ -f "$OVMF_CODE" ]] || { echo 'Set OVMF_CODE to an OVMF firmware image' >&2; exit 1; }
OVMF_VARS="${OVMF_VARS:-${OVMF_CODE/CODE/VARS}}"
[[ -f "$OVMF_VARS" ]] || { echo 'Set OVMF_VARS to the matching variables template' >&2; exit 1; }
cp "$OVMF_VARS" "$ROOT/OVMF_VARS.runtime.fd"
DATA_ARGS=()
[[ -z "${NOVA_QEMU_DATADIR:-}" ]] || DATA_ARGS=(-L "$NOVA_QEMU_DATADIR")
DISPLAY_ARGS=()
[[ -z "${NOVA_DISPLAY:-}" ]] || DISPLAY_ARGS=(-display "$NOVA_DISPLAY")
exec "$QEMU_BIN" "${DATA_ARGS[@]}" -machine q35 -m "${NOVA_RAM:-256M}" -cpu qemu64 -smp "${NOVA_SMP:-1}" \
  -drive "if=pflash,format=raw,readonly=on,file=$OVMF_CODE" \
  -drive "if=pflash,format=raw,file=$ROOT/OVMF_VARS.runtime.fd" \
  -drive "if=ide,index=0,format=raw,file=$ROOT/nova-efi.img" \
  -drive "if=none,id=nova-system,format=raw,file=$ROOT/nova-system.img" \
  -device virtio-blk-pci,drive=nova-system,disable-modern=on \
  -device VGA "${DISPLAY_ARGS[@]}" -serial stdio -no-reboot "$@"
EOF
chmod +x "$RELEASE_DIR/run-qemu.sh"
(
  cd "$RELEASE_DIR"
  sha256sum nova-efi.img nova-system.img > SHA256SUMS
)
tar -C "$ROOT/build/release" -czf "$ARCHIVE" "$NAME"
(
  cd "$ROOT/build"
  sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256"
)
echo "Nova release archive: $ARCHIVE"
