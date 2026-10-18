# CPU-local state

## IMPLEMENTED / EXPERIMENTAL

`PerCpu` records are pinned static values (eight slots). IA32_GS_BASE points to the current record. The record stores logical/APIC ID, startup state, kernel stack, current TID placeholder, preemption depth and interrupt depth placeholders. BSP and APs install their own GS_BASE; AP startup verifies GS-relative logical ID before acknowledgement. APs also load independent GDT/TSS and dedicated IST stacks for double fault, NMI and machine check.

`current_tid`, nesting counters and syscall-stack migration are not yet wired into the scheduler. The BSP still owns user scheduling. An AP in `Idle` has entered Nova and can HLT with its own IDT/TSS; it is not marked scheduler-online.
