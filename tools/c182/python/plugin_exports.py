"""Check that a Windows x64 plugin exposes only the X-Plane lifecycle ABI.

Uses the PE export directory directly; no SDK, compiler or third-party parser
is required. Callback addresses passed to the SDK do not need named exports.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

EXPECTED = frozenset(('XPluginStart', 'XPluginStop', 'XPluginEnable',
                      'XPluginDisable', 'XPluginReceiveMessage'))


def exports(data):
    def unpack(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(data):
            raise ValueError('Truncated PE data')
        return struct.unpack_from(fmt, data, offset)

    if data[:2] != b'MZ':
        raise ValueError('Missing DOS header')
    pe, = unpack('<I', 0x3c)
    if data[pe:pe + 4] != b'PE\0\0':
        raise ValueError('Missing PE signature')
    machine, count, _, _, _, optional_size, _ = unpack('<HHIIIHH', pe + 4)
    optional = pe + 24
    magic, = unpack('<H', optional)
    if machine != 0x8664 or magic != 0x20b or optional_size < 120:
        raise ValueError('Expected a Windows x64 PE32+ plugin')
    directory_count, = unpack('<I', optional + 108)
    if not directory_count:
        raise ValueError('Missing export directory')
    export_rva, export_size = unpack('<II', optional + 112)
    sections = []
    for i in range(count):
        at = optional + optional_size + i * 40
        unpack('<40s', at)
        size, rva, raw_size, raw = unpack('<IIII', at + 8)
        sections.append((rva, max(size, raw_size), raw, raw_size))

    def offset(rva, size):
        for base, virtual_size, raw, raw_size in sections:
            delta = rva - base
            if 0 <= delta < virtual_size and delta + size <= raw_size:
                at = raw + delta
                unpack(f'<{size}s', at)
                return at
        raise ValueError(f'Unmapped export RVA: {rva:#x}')

    if export_size < 40:
        raise ValueError('Missing export directory')
    directory = unpack('<IIHHIIIIIII', offset(export_rva, 40))
    functions, names, address_rva, name_rva, ordinal_rva = directory[6:]
    if functions != names or names > 65536:
        raise ValueError('Unexpected unnamed exports or export count')
    addresses = unpack(f'<{functions}I', offset(address_rva, functions * 4))
    pointers = unpack(f'<{names}I', offset(name_rva, names * 4))
    ordinals = unpack(f'<{names}H', offset(ordinal_rva, names * 2))
    result = []
    for pointer, ordinal in zip(pointers, ordinals):
        if ordinal >= functions or not addresses[ordinal]:
            raise ValueError('Invalid export ordinal/address')
        address = addresses[ordinal]
        if export_rva <= address < export_rva + export_size:
            raise ValueError('Forwarded plugin entry point')
        offset(address, 1)
        text = bytearray()
        for i in range(1024):
            byte = data[offset(pointer + i, 1)]
            if byte == 0:
                break
            text.append(byte)
        else:
            raise ValueError('Unterminated export name')
        result.append(text.decode('ascii'))
    if len(set(result)) != len(result) or len(set(ordinals)) != len(ordinals):
        raise ValueError('Duplicate export names or ordinals')
    return sorted(result)


def audit(path):
    data = Path(path).read_bytes()
    names = exports(data)
    return {'plugin': str(Path(path).resolve()),
            'sha256': hashlib.sha256(data).hexdigest(), 'exports': names,
            'missing': sorted(EXPECTED - set(names)),
            'unexpected': sorted(set(names) - EXPECTED),
            'passed': set(names) == EXPECTED}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('plugin', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    report = audit(args.plugin)
    text = json.dumps(report, indent=2) + '\n'
    if args.output:
        args.output.write_text(text, encoding='utf-8')
    print(text, end='')
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
