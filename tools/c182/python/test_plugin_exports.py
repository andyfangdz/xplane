"""Synthetic PE fixtures for the production ABI guard."""
import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location('plugin_exports', Path(__file__).with_name('plugin_exports.py'))
abi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(abi)


def fixture(names=abi.EXPECTED):
    names = sorted(names)
    data = bytearray(0xa00)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 0x80)
    data[0x80:0x84] = b'PE\0\0'
    struct.pack_into('<HHIIIHH', data, 0x84, 0x8664, 1, 0, 0, 0, 240, 0)
    struct.pack_into('<H', data, 0x98, 0x20b)
    struct.pack_into('<III', data, 0x98 + 108, 16, 0x1000, 0x180)
    struct.pack_into('<8sIIII', data, 0x188, b'.rdata\0\0', 0x800, 0x1000, 0x800, 0x200)
    struct.pack_into('<IIHHIIIIIII', data, 0x200, 0, 0, 0, 0, 0, 1,
                     len(names), len(names), 0x1040, 0x1080, 0x10c0)
    at = 0x300
    for i, name in enumerate(names):
        struct.pack_into('<I', data, 0x240 + i * 4, 0x1500 + i)
        struct.pack_into('<I', data, 0x280 + i * 4, at + 0xe00)
        struct.pack_into('<H', data, 0x2c0 + i * 2, i)
        raw = name.encode('ascii') + b'\0'
        data[at:at + len(raw)] = raw
        at += len(raw)
    return data


class PluginExports(unittest.TestCase):
    def test_exact_sdk_contract(self):
        self.assertEqual(set(abi.exports(fixture())), abi.EXPECTED)

    def test_obsolete_entry_point_is_visible(self):
        names = set(abi.exports(fixture(abi.EXPECTED | {'c182_model_internal'})))
        self.assertEqual(names - abi.EXPECTED, {'c182_model_internal'})

    def test_missing_lifecycle_entry_is_visible(self):
        self.assertEqual(abi.EXPECTED - set(abi.exports(fixture(abi.EXPECTED - {'XPluginEnable'}))),
                         {'XPluginEnable'})

    def test_ordinal_only_export_rejected(self):
        data = fixture()
        struct.pack_into('<I', data, 0x200 + 24, 6)
        with self.assertRaises(ValueError):
            abi.exports(data)

    def test_forwarder_rejected(self):
        data = fixture()
        struct.pack_into('<I', data, 0x240, 0x1100)
        with self.assertRaisesRegex(ValueError, 'Forwarded'):
            abi.exports(data)

    def test_unmapped_names_rejected(self):
        data = fixture()
        struct.pack_into('<I', data, 0x280, 0xffffff00)
        with self.assertRaisesRegex(ValueError, 'Unmapped'):
            abi.exports(data)

    def test_truncation_rejected(self):
        with self.assertRaises(ValueError):
            abi.exports(fixture()[:0x220])


if __name__ == '__main__':
    unittest.main()
