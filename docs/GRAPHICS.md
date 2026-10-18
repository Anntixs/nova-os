# Framebuffer interface

## IMPLEMENTED

The GOP framebuffer from NovaBoot remains mapped for the kernel terminal. `/dev/fb0` reads a 24-byte little-endian metadata record: six `u32` values for width, height, stride in pixels, GOP pixel format, bits per pixel (32), and pitch in bytes. `FB_INFO` (26) returns the same record. The kernel checks a process-owned framebuffer lease with `FB_ACQUIRE` (27) and `FB_RELEASE` (28). `FB_FILL` (29) takes five `u32` fields: x, y, width, height, RGB color. `FB_BLIT` (30) takes x, y, width, height, source stride in pixels, reserved word, and a `u64` userspace pixel-buffer address; pixels are 32-bit values. Rectangle and user range checks reject overflow and out-of-bounds access. The kernel reads user pixels through validated translations, never shares raw framebuffer memory with Ring 3. `/bin/fbtest` exercises fill/blit and releases the lease.

## EXPERIMENTAL / PLANNED

There is no double buffer, dirty-rectangle API, compositor, or framebuffer revocation protocol beyond owner exit. The kernel terminal can render while a userspace process owns the framebuffer, so a future display server must coordinate console handoff. `/dev/fb0` is metadata-only and cannot be mmap'ed.
