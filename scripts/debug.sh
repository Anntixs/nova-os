#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
echo 'QEMU will pause at startup. In GDB: target remote :1234; symbol-file target/x86_64-unknown-none/debug/nova-kernel'
export NOVA_PROFILE=debug
exec "$ROOT/scripts/run.sh" -S -gdb tcp::1234
