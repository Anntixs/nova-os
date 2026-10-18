# Device namespace

## IMPLEMENTED

The VFS mounts a generated device filesystem at `/dev` over the FAT32 root. `/dev/null` discards writes and returns EOF. `/dev/zero` returns zero bytes. `/dev/console` writes through the kernel TTY. `/dev/fb0` reports framebuffer metadata; drawing is controlled by framebuffer syscalls. `/dev/kbd0` exposes timestamped keyboard records. Descriptors use the same per-process FD table as regular files and pipes. Shell `ls /dev` shows entries from the mounted device filesystem.

## PLANNED

There are no stable hardware device handles, access permissions, mouse, GPU, audio or network devices. Device operations currently live in the descriptor dispatch layer; a richer driver registry can replace this without changing path resolution.
