# FAT32 driver

## IMPLEMENTED

Nova validates the BPB, 512-byte sectors, power-of-two cluster size, FAT32 cluster count, root cluster, volume size and block-device capacity. It follows cluster chains with bounds and traversal limits; invalid numbers, premature end markers and cycles return errors. Root and nested directory reading supports short 8.3 entries and VFAT long names. File reads honor the recorded size and cross sector/cluster boundaries. All on-disk integers are read as explicit little-endian values.

Writable operations support creation of 8.3 files and directories, overwrite/truncation, extension across clusters, removal of files and empty directories, and allocation/freeing of cluster chains. Both FAT copies are updated. New directory entries include `.` and `..`. Long existing names remain readable; creating or modifying a long-name target is not supported. The QEMU image builder emits long-name entries for programs such as `badopcode`.

## EXPERIMENTAL

No journaling, transactional rollback or media flush protocol exists. A failed multi-step write may leave allocated but unreachable clusters. Directory creation and file writes are synchronous. Timestamps and FAT32 FSInfo free counts are not updated. FAT names are compared case-insensitively, while new entries use short uppercase names on disk. A malformed disk returns an error where checked; the parser is still a first implementation and needs fuzzing before use with arbitrary removable media.

## PLANNED

Crash recovery, long-name mutation, free-space index, timestamp handling and broader interoperability tests.
