#!/usr/bin/env python3
"""Build Nova's temporary raw FAT32 development volume without host mkfs tools."""
from pathlib import Path
import os
import struct
import tomllib

OUT = Path(os.environ.get('NOVA_SYSTEM_IMAGE_OUTPUT', 'build/nova-system.img'))
VERSION = tomllib.loads(Path('kernel/Cargo.toml').read_text())['package']['version']
SECTOR = 512
SECTORS = 131072
RESERVED = 32
FAT_SECTORS = 1024
DATA_START = RESERVED + 2 * FAT_SECTORS
ROOT_CLUSTER = 2

def saved_user_data():
    """Carry writable user trees through a rebuild of packaged /bin and /etc."""
    if not OUT.exists():
        return {}
    old = OUT.read_bytes()
    if len(old) != SECTORS * SECTOR or old[3:11] != b'NOVAFAT ':
        return {}
    spc = old[13]
    if spc == 0 or spc & (spc - 1):
        raise ValueError('existing Nova volume has invalid cluster geometry')
    reserved = struct.unpack_from('<H', old, 14)[0]
    fats = old[16]
    fat_sectors = struct.unpack_from('<I', old, 36)[0]
    data_start = reserved + fats * fat_sectors
    limit = (SECTORS - data_start) // spc + 2
    def chain(first):
        seen = set()
        while first >= 2 and first < limit:
            if first in seen: raise ValueError('cyclic FAT in existing Nova image')
            seen.add(first)
            yield first
            value = struct.unpack_from('<I', old, reserved * SECTOR + first * 4)[0] & 0x0fffffff
            if value >= 0x0ffffff8: return
            first = value
        raise ValueError('invalid FAT chain in existing Nova image')
    def content(first, size):
        if size == 0: return b''
        if size > 16 * 1024 * 1024: raise ValueError('user file too large to preserve')
        result = bytearray()
        for n in chain(first):
            pos = (data_start + (n - 2) * spc) * SECTOR
            result += old[pos:pos + spc * SECTOR]
            if len(result) >= size: return bytes(result[:size])
        raise ValueError('truncated user file in existing Nova image')
    def scan(first, depth=0):
        if depth > 12: raise ValueError('user directory depth exceeded')
        result = {}
        for n in chain(first):
            start = (data_start + (n - 2) * spc) * SECTOR
            for off in range(start, start + spc * SECTOR, 32):
                raw = old[off:off + 32]
                if raw[0] == 0: return result
                if raw[0] == 0xe5 or raw[11] == 15 or raw[11] & 8: continue
                name = raw[:8].decode('ascii').rstrip()
                ext = raw[8:11].decode('ascii').rstrip()
                if ext: name += '.' + ext
                if name in ('.', '..'): continue
                cluster = (struct.unpack_from('<H', raw, 20)[0] << 16) | struct.unpack_from('<H', raw, 26)[0]
                size = struct.unpack_from('<I', raw, 28)[0]
                result[name.lower()] = scan(cluster, depth + 1) if raw[11] & 16 else content(cluster, size)
        return result
    root = scan(ROOT_CLUSTER)
    return {k: root.get(k, {}) for k in ('home', 'system', 'tmp')}

PRESERVED = {} if os.environ.get('NOVA_FRESH_IMAGE') == '1' else saved_user_data()
NEXT = 3
fat = [0] * (FAT_SECTORS * SECTOR // 4)
fat[0:3] = [0x0FFFFFF8, 0x0FFFFFFF, 0x0FFFFFFF]
image = bytearray(SECTORS * SECTOR)

def sector(n, content):
    start = n * SECTOR
    image[start:start + len(content)] = content

def cluster(n, content):
    sector(DATA_START + n - 2, content)

def allocate(data):
    global NEXT
    chunks = [data[i:i + SECTOR] for i in range(0, len(data), SECTOR)] or [b'']
    first = NEXT
    for i, chunk in enumerate(chunks):
        n = NEXT
        NEXT += 1
        fat[n] = NEXT if i + 1 < len(chunks) else 0x0FFFFFFF
        cluster(n, chunk)
    return first

def short_name(name):
    if name in ('.', '..'):
        return name.ljust(11).encode('ascii'), False
    stem, dot, ext = name.partition('.')
    if len(stem) <= 8 and len(ext) <= 3:
        return (stem.upper().ljust(8) + ext.upper().ljust(3)).encode('ascii'), False
    return (stem[:6].upper().ljust(6, '_') + '~1' + ext[:3].upper().ljust(3)).encode('ascii'), True

def entries(name, first, size, directory=False):
    short, long = short_name(name)
    result = bytearray()
    if long:
        checksum = 0
        for byte in short:
            checksum = ((checksum & 1) << 7) + (checksum >> 1) + byte
            checksum &= 255
        chars = [ord(c) for c in name]
        chunks = [chars[i:i + 13] for i in range(0, len(chars), 13)]
        for index in range(len(chunks), 0, -1):
            chunk = chunks[index - 1]
            values = chunk + [0] + [0xffff] * (12 - len(chunk)) if len(chunk) < 13 else chunk
            record = bytearray(32)
            record[0] = index | (0x40 if index == len(chunks) else 0)
            record[11] = 15
            record[13] = checksum
            for off, val in zip([1,3,5,7,9,14,16,18,20,22,24,28,30], values):
                struct.pack_into('<H', record, off, val)
            result += record
    record = bytearray(32)
    record[:11] = short
    record[11] = 0x10 if directory else 0x20
    if not long and name not in ('.', '..'):
        # FAT NTRes lowercase flags preserve the developer image's names.
        record[12] = (0x08 if any(c.islower() for c in name.partition('.')[0]) else 0) | (0x10 if any(c.islower() for c in name.partition('.')[2]) else 0)
    struct.pack_into('<H', record, 20, first >> 16)
    struct.pack_into('<H', record, 26, first & 0xffff)
    struct.pack_into('<I', record, 28, size)
    result += record
    return result

def directory(items, parent, own):
    data = bytearray()
    if parent:
        data += entries('.', own, 0, True)
        data += entries('..', parent, 0, True)
    for name, first, size, is_dir in items:
        data += entries(name, first, size, is_dir)
    previous = own
    for start in range(0, len(data), SECTOR):
        if start:
            following = allocate(b'')
            fat[previous] = following
            previous = following
        cluster(previous, data[start:start + SECTOR])

boot = bytearray(SECTOR)
boot[:3] = b'\xeb\x58\x90'
boot[3:11] = b'NOVAFAT '
struct.pack_into('<HBHBHHBHHHII', boot, 11, 512, 1, RESERVED, 2, 0, 0, 0xf8, 0, 63, 255, 0, SECTORS)
struct.pack_into('<IHHIHH', boot, 36, FAT_SECTORS, 0, 0, ROOT_CLUSTER, 1, 6)
boot[64:68] = b'\x80\x00\x29\x00'
boot[71:82] = b'NOVASYSTEM '
boot[82:90] = b'FAT32   '
boot[510:512] = b'\x55\xaa'
sector(0, boot)
sector(6, boot)
fsinfo = bytearray(SECTOR)
struct.pack_into('<I', fsinfo, 0, 0x41615252)
struct.pack_into('<I', fsinfo, 484, 0x61417272)
struct.pack_into('<II', fsinfo, 488, 0xffffffff, 0xffffffff)
fsinfo[510:512] = b'\x55\xaa'
sector(1, fsinfo)
sector(7, fsinfo)

def build_tree(items, own, parent):
    entries_list = []
    for name, value in sorted(items.items()):
        if isinstance(value, dict):
            child = allocate(b'')
            build_tree(value, child, own)
            entries_list.append((name, child, 0, True))
        else:
            first = allocate(value) if value else 0
            entries_list.append((name, first, len(value), False))
    directory(entries_list, parent, own)

root_items = []
for name in ('bin', 'etc', 'home', 'system', 'tmp'):
    own = allocate(b'')
    if name == 'bin':
        items = {binary.stem: binary.read_bytes() for binary in Path('build/userspace').glob('*.elf') if binary.stem != 'init'}
    elif name == 'etc':
        items = {'version': f'Nova OS {VERSION}\n'.encode(), 'hostname': b'nova\n'}
    else:
        items = PRESERVED.get(name, {})
        if name == 'system' and Path('build/userspace/init.elf').exists():
            items['init'] = Path('build/userspace/init.elf').read_bytes()
    build_tree(items, own, ROOT_CLUSTER)
    root_items.append((name, own, 0, True))
directory(root_items, 0, ROOT_CLUSTER)
for i in range(FAT_SECTORS):
    chunk = fat[i * 128:(i + 1) * 128]
    data = struct.pack('<128I', *chunk)
    sector(RESERVED + i, data)
    sector(RESERVED + FAT_SECTORS + i, data)
OUT.parent.mkdir(exist_ok=True)
OUT.write_bytes(image)
print(f'Nova FAT32 image: {OUT}, {NEXT - 2} allocated clusters')
