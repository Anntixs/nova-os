#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p build
for count in 1 2 4 8; do
  echo "[NOVA TEST] QEMU -smp $count"
  if [[ "$count" -eq 1 ]]; then stress=0; else stress=1; fi
  NOVA_SMP="$count" NOVA_SMPTEST="$stress" python3 scripts/test-init.py > "build/test-smp-${count}.log" 2>&1
  cp build/test-init.log "build/test-smp-${count}-serial.log"
  tail -n 1 "build/test-smp-${count}.log"
done
echo 'NOVA_TEST_SUCCESS AP startup matrix (scheduler remains BSP-only)'
