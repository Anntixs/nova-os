# Virtual filesystem

## IMPLEMENTED

The VFS owns path normalization, the root mount, generic `Node` metadata, a `FileSystem` interface and open `File` objects. The current mounted implementation is FAT32. Shell commands and syscalls call VFS operations; neither contains a second directory model. Paths are normalized once: absolute and relative paths, repeated separators, `.` and `..` work, and `..` stops at `/`. NovaShell holds its cwd; each Process has a cwd field initialized to `/`. User `open` resolves relative to that process cwd.

Each open file has an independent offset and directory enumeration cursor. Process file descriptor slots 0, 1, 2 are reserved for stdin, stdout and stderr; file descriptors start at 3 and are reclaimed when the process exits. `SYS_WRITE` supports stdout/stderr and writable regular files. Directory entries returned to userspace use a Nova ABI structure, not FAT directory records.

## EXPERIMENTAL

Only `/` is mounted. Open regular files are loaded into kernel memory, capped by the FAT32 reader at 16 MiB. This simplifies lifetime and independent offsets but means an already-open descriptor does not observe writes through a different descriptor. The kernel shell uses VFS directly rather than syscalls. File descriptors are process-local but are not yet a general capability table. There is no `/dev` or mount namespace.

## PLANNED

Multiple mounts, symlinks, per-file streaming and cache coherence, device nodes and handle rights.
