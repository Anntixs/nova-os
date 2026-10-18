# PCI

## IMPLEMENTED

Nova scans bus/device/function tuples with PCI configuration ports `0xCF8/0xCFC`. It records vendor ID, device ID, class, subclass and six raw BAR registers. Multifunction devices are scanned when the header advertises them. The VirtIO driver enables I/O and bus mastering after identifying `1af4:1001`; no filesystem code accesses PCI directly. Detected functions are logged to COM1.

## EXPERIMENTAL

This is conventional PCI config mechanism 1. BAR sizes are not probed, bridge routing is not interpreted, and only one matching legacy VirtIO Block function is bound. The driver validates BAR0 is an I/O BAR before using it.

## PLANNED

ECAM/MCFG, bridge topology and resource allocation, capability parsing, MSI/MSI-X and a wider driver registry.
