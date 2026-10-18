# Process/system information filesystem

## IMPLEMENTED

The VFS mounts a generated read-only filesystem at `/proc`. Its files are `/proc/meminfo`, `/proc/uptime`, `/proc/cpuinfo`, `/proc/system`, `/proc/kmsg`, and `/proc/<pid>/status` for process records that currently exist. Reads produce kernel information at the time the file is opened. `/proc/kmsg` snapshots the bounded 64 KiB serial log ring. C programs `ps`, `sysinfo`, `uptime`, and `dmesg` read these paths through ordinary file syscalls.

## EXPERIMENTAL

Snapshots may become stale while a descriptor is open. Process records include unreaped exited children. No credential policy limits reading `/proc` yet; it is a development interface and is not Linux-compatible.
