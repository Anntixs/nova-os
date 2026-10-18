# Input events

## IMPLEMENTED

The PS/2 IRQ handler converts scan codes to `KeyEvent`: ASCII key when available, press/release, modifier bits, hardware code, and monotonic timestamp in milliseconds. Modifiers are Shift=1, Ctrl=2, Alt=4, Caps=8. A separate bounded raw queue feeds `/dev/kbd0`; the TTY character queue continues to serve stdin. Reading `/dev/kbd0` yields one 16-byte little-endian record: byte 0 kind (1 down, 2 up), byte 1 modifiers, bytes 2–3 code, byte 4 ASCII, bytes 5–7 reserved, bytes 8–15 timestamp. Reads block until an event and wake through the scheduler. Overflow drops the oldest raw event. `/bin/devtest` exercises this path in QEMU.

## PLANNED

`PointerEvent` is only a type; no mouse/touch driver or userspace pointer-event device exists. A display server will need a unified event stream and focus policy.
