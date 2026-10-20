# Nova UI platform readiness

## IMPLEMENTED / TESTED UNDER QEMU

- A Ring 3 process can acquire the GOP framebuffer, query its format and dimensions, and draw validated rectangles and pixel buffers. The kernel console writes to serial instead of the framebuffer while the lease is held. Release, normal exit and user fault restore the console. A QEMU screenshot regression checks that stdout does not overwrite a leased framebuffer.
- The keyboard driver publishes key events with press/release state, modifiers and monotonic timestamps through `/dev/kbd0`. A process can acquire exclusive raw keyboard delivery; during the lease those keys are never queued for the TTY. The current userspace test consumes a key injected after acquisition, independently of the PS/2 driver.
- Anonymous pipes and message channels transfer data through Nova syscalls. Processes can spawn, wait, use file descriptors and read resources from FAT32.
- `/system/init` and `/bin/nsh` run in Ring 3. User faults are isolated in the current single scheduler CPU configuration.

## BLOCKING GAPS BEFORE A ROBUST DISPLAY SERVER

- The process scheduler still runs only on the BSP. APs enter Nova code and have private CPU state, but no user tasks are dispatched to them. The scheduler now has per-CPU host/current slots; cross-CPU thread handoff, per-CPU current process and TLB shootdown must be completed and stress-tested before enabling AP scheduling.
- Framebuffer ownership and kernel console handback are serialized only by the BSP execution model. A shared graphics lock or equivalent handoff protocol is required before multiple CPUs can render.
- The framebuffer interface copies pixels through syscalls. It has no page-backed surface, direct safe mapping, dirty-region batching or asynchronous present operation. A compositor can be prototyped with copies, but performance has not been measured.
- The raw keyboard lease prevents delivery to the TTY but has no focus policy, multi-seat support, pointer events or privileged revocation. Keyboard input ownership needs a broader device/session policy before general GUI clients can receive input safely.
- Pointer events are defined, but there is no working mouse driver. PS/2 keyboard is the only tested physical input source. USB/xHCI, HID, NVMe and AHCI remain unimplemented.
- Channels have no named discovery, peer-close reporting or capability transfer. A display server and clients need a safe endpoint discovery protocol and process lifetime handling.
- Graphics has no panic-safe shared lock, double buffer, cursor plane, hotplug or display mode management. Physical hardware remains untested.

The repository remains Nova OS 0.6.0. These gaps are not marked complete by a successful boot or a framebuffer screenshot. Do not label a release 0.7.0 or start the full desktop milestone until the relevant gates are implemented and exercised.
