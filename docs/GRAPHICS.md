# Framebuffer interface

## IMPLEMENTED

The GOP framebuffer from NovaBoot remains mapped for the kernel terminal. `/dev/fb0` reads a 24-byte little-endian metadata record: six `u32` values for width, height, stride in pixels, GOP pixel format, bits per pixel (32), and pitch in bytes. `FB_INFO` (26) returns the same record. The kernel checks a process-owned framebuffer lease with `FB_ACQUIRE` (27) and `FB_RELEASE` (28). `FB_FILL` (29) takes five `u32` fields: x, y, width, height, RGB color. `FB_BLIT` (30) takes x, y, width, height, source stride in pixels, reserved word, and a `u64` userspace pixel-buffer address; pixels are 32-bit values. Rectangle and user range checks reject overflow and out-of-bounds access. The kernel reads user pixels through validated translations, never shares raw framebuffer memory with Ring 3. `/bin/fbtest` exercises fill/blit and releases the lease. While a process holds the lease, normal terminal and diagnostic text is sent to serial but cannot draw over its framebuffer. Releasing the lease or exiting/faulting clears the screen and restores terminal rendering; fatal kernel panic output can override the lease.

## EXPERIMENTAL / PLANNED

There is no double buffer, dirty-rectangle API, compositor, or framebuffer revocation protocol beyond owner exit. Terminal output produced while the lease is held is retained on serial only; it is not replayed on framebuffer handback. Lease release and terminal clear are serialized only by the current single scheduler CPU, so this handoff needs a shared graphics lock before concurrent CPUs render. `/dev/fb0` is metadata-only and cannot be mmap'ed.
