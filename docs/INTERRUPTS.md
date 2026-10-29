# Interrupts and time

## IMPLEMENTED

IDT vectors 0–31 are CPU exceptions. Vector 32 is PIT IRQ0, 33 is PS/2 IRQ1, 240 is the reserved remote TLB flush IPI, 241 wakes an AP's scheduler host loop, and 255 is Local APIC spurious. Other vectors point to a fatal unknown-vector stub. CPU exceptions from CPL 3 (except NMI, double fault and machine check) terminate the current user process and return to the scheduler and waiting parent; kernel-mode exceptions remain fatal. NMI remains fatal because Nova has no platform NMI recovery handler. Fatal diagnostics report the Local APIC ID when the APIC is initialized. Assembly IRQ entry saves general registers and floating-point/SSE state, calls a small Rust dispatcher, restores state, and returns with `iretq`.

Nova validates the ACPI RSDP and SDTs, reads MADT IOAPIC and interrupt-source overrides, enables the Local APIC, routes IRQ0/IRQ1 through IOAPIC, and masks the legacy PIC. The PIT is programmed near 1000 Hz. A monotonic tick counter provides milliseconds to `uptime`. The timer IRQ also requests round-robin preemption every ten ticks. The PS/2 driver tracks key press/release, Shift/Ctrl/Alt/Caps Lock, and set-1 ASCII conversion. IRQ1 appends events to bounded TTY and raw-event queues; userspace consumes them outside IRQ context. Queue overflow is counted.

## TEMPORARY

PIT is the initial timer source. APIC timer and HPET calibration are planned. The 1000 Hz uptime assumes the PIT divisor and may drift or lose ticks under heavy VM load. The implementation is single-core; IOAPIC destination is the bootstrap CPU. PS/2 is the only input driver. There is no USB HID or SMP. See SCHEDULER.md for preemption limits.
