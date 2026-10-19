# CPU-local state

## IMPLEMENTED / EXPERIMENTAL

`PerCpu` records are pinned static values (eight slots). IA32_GS_BASE points to the current record while in the kernel; IA32_KERNEL_GS_BASE holds the trusted record across Ring 3 entry. The record stores logical/APIC ID, startup state, kernel stack, a syscall-entry user RSP scratch slot, current TID placeholder, preemption depth and interrupt depth placeholders. BSP and APs install their own GS bases; AP startup verifies GS-relative logical ID before acknowledgement. APs also load independent GDT/TSS and dedicated IST stacks for double fault, NMI and machine check.

`SYSCALL` now uses the CPU-local trusted kernel stack and scratch RSP instead of global variables. `SWAPGS` runs on user entry/return through syscall, IRQ and fatal exception paths; the first IRETQ into userspace swaps too. Nova clears CR4.FSGSBASE on each CPU before admitting user code. NMI handling in the narrow syscall entry/exit SWAPGS window still needs a paranoid GS-base check before SMP user scheduling is enabled.

`current_tid` and nesting counters are not yet wired into the scheduler. The BSP still owns user scheduling. An AP in `Idle` has entered Nova and can HLT with its own IDT/TSS; it is not marked scheduler-online.
