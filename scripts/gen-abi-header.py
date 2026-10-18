#!/usr/bin/env python3
"""Generate C ABI numbers from the Rust wire-value definitions."""
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
source = (root / 'libs/nova-core/src/abi.rs').read_text()
target = root / 'build/sysroot/include/nova/abi_numbers.h'
matches = re.findall(r'^pub const ([A-Z][A-Z0-9_]*):(u64|i64)=(-?\d+);$', source, re.M)
if len(matches) < 20:
    raise SystemExit('Nova ABI definitions could not be parsed')
lines = ['/* Generated from libs/nova-core/src/abi.rs. Do not edit. */',
         '#ifndef NOVA_ABI_NUMBERS_H', '#define NOVA_ABI_NUMBERS_H']
for name, kind, value in matches:
    prefix = 'NOVA_' if kind == 'i64' else 'NOVA_SYS_'
    lines.append(f'#define {prefix}{name} {value}')
lines.append('#endif')
target.parent.mkdir(parents=True, exist_ok=True)
target.write_text('\n'.join(lines) + '\n')
