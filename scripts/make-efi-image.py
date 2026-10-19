#!/usr/bin/env python3
"""Create a standalone FAT32 EFI volume containing NovaBoot and the kernel."""
from pathlib import Path
import struct

SECTOR = 512
SECTORS = 131072  # 64 MiB; enough clusters to be unambiguously FAT32.
RESERVED = 32
FAT_SECTORS = 1024
DATA_START = RESERVED + 2 * FAT_SECTORS
OUT = Path("build/nova-efi.img")
BOOTLOADER = Path("build/esp/EFI/BOOT/BOOTX64.EFI")
KERNEL = Path("build/esp/EFI/NOVA/KERNEL.ELF")

image = bytearray(SECTORS * SECTOR)
fat = [0] * (FAT_SECTORS * SECTOR // 4)
fat[:3] = [0x0FFFFFF8, 0x0FFFFFFF, 0x0FFFFFFF]
next_cluster = 3


def sector(index: int, data: bytes) -> None:
    assert 0 <= index < SECTORS and len(data) <= SECTOR
    offset = index * SECTOR
    image[offset:offset + len(data)] = data


def cluster(index: int, data: bytes) -> None:
    sector(DATA_START + index - 2, data)


def allocate(data: bytes) -> int:
    global next_cluster
    chunks = [data[i:i + SECTOR] for i in range(0, len(data), SECTOR)] or [b""]
    first = next_cluster
    for position, chunk in enumerate(chunks):
        index = next_cluster
        next_cluster += 1
        if index >= len(fat) or DATA_START + index - 2 >= SECTORS:
            raise ValueError("EFI image is too small")
        fat[index] = next_cluster if position + 1 < len(chunks) else 0x0FFFFFFF
        cluster(index, chunk)
    return first


def entry(name: str, start: int, size: int, directory: bool) -> bytes:
    if name in (".", ".."):
        short = name.ljust(11)
    else:
        stem, dot, extension = name.partition(".")
        if len(stem) > 8 or len(extension) > 3 or not stem.isascii() or not extension.isascii():
            raise ValueError(f"not an 8.3 EFI name: {name}")
        short = stem.upper().ljust(8) + extension.upper().ljust(3)
    record = bytearray(32)
    record[:11] = short.encode("ascii")
    record[11] = 0x10 if directory else 0x20
    struct.pack_into("<H", record, 20, start >> 16)
    struct.pack_into("<H", record, 26, start & 0xFFFF)
    struct.pack_into("<I", record, 28, size)
    return record


def directory(own: int, parent: int | None, items: list[tuple[str, int, int, bool]]) -> None:
    records = bytearray()
    if parent is not None:
        records += entry(".", own, 0, True)
        records += entry("..", parent, 0, True)
    for name, start, size, is_dir in items:
        records += entry(name, start, size, is_dir)
    if len(records) > SECTOR:
        raise ValueError("EFI directory exceeds one cluster")
    cluster(own, records)


def main() -> None:
    bootloader = BOOTLOADER.read_bytes()
    kernel = KERNEL.read_bytes()
    efi_dir = allocate(b"")
    boot_dir = allocate(b"")
    nova_dir = allocate(b"")
    boot_file = allocate(bootloader)
    kernel_file = allocate(kernel)
    directory(boot_dir, efi_dir, [("BOOTX64.EFI", boot_file, len(bootloader), False)])
    directory(nova_dir, efi_dir, [("KERNEL.ELF", kernel_file, len(kernel), False)])
    directory(efi_dir, 2, [("BOOT", boot_dir, 0, True), ("NOVA", nova_dir, 0, True)])
    directory(2, None, [("EFI", efi_dir, 0, True)])

    boot = bytearray(SECTOR)
    boot[:3] = b"\xeb\x58\x90"
    boot[3:11] = b"NOVAESP "
    struct.pack_into("<HBHBHHBHHHII", boot, 11, 512, 1, RESERVED, 2, 0, 0, 0xF8, 0, 63, 255, 0, SECTORS)
    struct.pack_into("<IHHIHH", boot, 36, FAT_SECTORS, 0, 0, 2, 1, 6)
    boot[64:68] = b"\x80\x00\x29\x00"
    boot[71:82] = b"NOVA EFI   "
    boot[82:90] = b"FAT32   "
    boot[510:512] = b"\x55\xaa"
    sector(0, boot)
    sector(6, boot)
    fsinfo = bytearray(SECTOR)
    struct.pack_into("<I", fsinfo, 0, 0x41615252)
    struct.pack_into("<I", fsinfo, 484, 0x61417272)
    struct.pack_into("<II", fsinfo, 488, 0xFFFFFFFF, 0xFFFFFFFF)
    fsinfo[510:512] = b"\x55\xaa"
    sector(1, fsinfo)
    sector(7, fsinfo)
    for index in range(FAT_SECTORS):
        data = struct.pack("<128I", *fat[index * 128:(index + 1) * 128])
        sector(RESERVED + index, data)
        sector(RESERVED + FAT_SECTORS + index, data)
    OUT.parent.mkdir(exist_ok=True)
    OUT.write_bytes(image)
    print(f"Nova EFI FAT32 image: {OUT}, {next_cluster - 2} allocated clusters")


if __name__ == "__main__":
    main()
