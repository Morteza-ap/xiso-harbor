#!/usr/bin/env python3
"""Create synthetic, redistributable XISO fixtures. Contains no game code.
Files inside XDVDFS use Windows-1252; host ISO/output paths may be Unicode.
The writer is independent of xdvdfs, so extraction tests do not reuse its packer.
"""
import argparse
import json
import math
from pathlib import Path
import struct

SECTOR = 2048
MAGIC = b'MICROSOFT*XBOX*MEDIA'

def make_fixture(destination, large=False, many=False, offset=0):
    files = {'DEFAULT.XEX': b'Synthetic fixture only. This is not executable Xbox code.\n',
             'README.txt': b'Public-domain synthetic extraction fixture.\n',
             'empty.dat': b'', 'Assets/sound file.bin': bytes(range(256)) * 17,
             'Assets/caf\xe9.txt': b'Windows-1252 filename; exact payload.\n'}
    if many:
        files.update({f'Files/group-{i//1000:02}/file-{i:05}.dat': f'fixture {i}\n'.encode() for i in range(10020)})
    if large:
        # Sparse source, real streamed output: more than 2 GiB in one entry.
        files['large-zeroes.bin'] = 2 * 1024**3 + 4 * 1024**2 + 17
    root = {}
    for path, payload in files.items():
        node = root
        components = path.split('/')
        for component in components[:-1]: node = node.setdefault(component, {})
        node[components[-1]] = payload
    cursor = 33
    writes = []
    def directory(node):
        nonlocal cursor
        names = sorted(node, key=str.upper)
        records = []
        length = 0
        for name in names:
            encoded = name.encode('cp1252')
            size = (14 + len(encoded) + 3) // 4 * 4
            records.append((name, encoded, length, size))
            length += size
        table_sector = cursor
        cursor += max(1, math.ceil(length / SECTOR))
        table = bytearray(max(SECTOR, math.ceil(length / SECTOR) * SECTOR))
        # Root record must be at offset zero; preorder balanced tree allocation.
        order = []
        def preorder(indices):
            if not indices: return
            middle = len(indices)//2
            order.append(indices[middle]); preorder(indices[:middle]); preorder(indices[middle+1:])
        preorder(list(range(len(records))))
        offsets = {}; physical = 0
        for index in order: offsets[index] = physical; physical += records[index][3]
        children = {}
        def links(indices):
            if not indices: return None
            middle = len(indices)//2; index = indices[middle]
            left = links(indices[:middle]); right = links(indices[middle+1:])
            children[index] = (0 if left is None else offsets[left]//4, 0 if right is None else offsets[right]//4)
            return index
        links(list(range(len(records))))
        for index, (name, encoded, _, _) in enumerate(records):
            payload = node[name]; is_dir = isinstance(payload, dict)
            if is_dir: sector, size = directory(payload)
            else:
                sector = cursor; size = payload if isinstance(payload, int) else len(payload)
                cursor += math.ceil(size / SECTOR)
                writes.append((sector * SECTOR, payload))
            left, right = children[index]
            record = struct.pack('<HHIIBB', left, right, sector, size, 0x10 if is_dir else 0x20, len(encoded)) + encoded
            start = offsets[index]; table[start:start+len(record)] = record
        writes.append((table_sector * SECTOR, table))
        return table_sector, len(table)
    table_sector, table_size = directory(root)
    descriptor = bytearray(SECTOR)
    descriptor[:20] = MAGIC; descriptor[20:28] = struct.pack('<II', table_sector, table_size); descriptor[2028:] = MAGIC
    destination = Path(destination); destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open('wb') as stream:
        stream.truncate(offset + cursor * SECTOR)
        stream.seek(offset + 32 * SECTOR); stream.write(descriptor)
        for location, payload in writes:
            if isinstance(payload, int): continue
            stream.seek(offset + location); stream.write(payload)
    manifest = {path: {'bytes': value if isinstance(value, int) else len(value), 'zeroes': isinstance(value, int)} for path, value in files.items()}
    destination.with_suffix('.manifest.json').write_text(json.dumps(manifest, indent=2))
    return files

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output'); parser.add_argument('--large', action='store_true'); parser.add_argument('--many', action='store_true')
    parser.add_argument('--offset', type=int, default=0, choices=[0, 405798912, 265879552, 34078720])
    args = parser.parse_args(); make_fixture(args.output, args.large, args.many, args.offset)
    print(f'Created synthetic fixture: {args.output}')
