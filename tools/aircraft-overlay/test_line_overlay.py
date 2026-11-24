from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from line_overlay import apply, record


class OverlayTest(unittest.TestCase):
    def test_exact_bytes(self):
        with TemporaryDirectory() as directory:
            base, edit, patch = (Path(directory) / p for p in ("base", "edited", "patch"))
            for old, new in [(b"", b""), (b"", b"first"), (b"gone\n", b""),
                             (b"a\r\nb\r\na\n", b"a\r\nb\na\na\n"),
                             (b"x\ny", b"x\r\ny\n"), (b"x\r\ny\r\n", b"x\ny"),
                             (b"a\nb\nc\n", b"new\na\nc\nend"),
                             (b"copy 1 1\n", b"copy 1 1\n")]:
                with self.subTest(old=old, new=new):
                    base.write_bytes(old)
                    edit.write_bytes(new)
                    patch.write_bytes(record(base, edit))
                    self.assertEqual(apply(base, patch), new)
                    base.write_bytes(old + b"modified")
                    with self.assertRaises(ValueError):
                        apply(base, patch)

    def test_damaged_records(self):
        with TemporaryDirectory() as directory:
            base, edit, patch = (Path(directory) / p for p in ("base", "edited", "patch"))
            base.write_bytes(b"line\n")
            edit.write_bytes(b"line\n")
            header = record(base, edit).splitlines(keepends=True)[0]
            for invalid in (b"copy 0 1\n", b"copy 1 2\n", b"add 5\nabc", b"add 2\nabc", b"?\n"):
                with self.subTest(invalid=invalid):
                    patch.write_bytes(header + invalid)
                    with self.assertRaises(ValueError):
                        apply(base, patch)


if __name__ == "__main__":
    unittest.main()
