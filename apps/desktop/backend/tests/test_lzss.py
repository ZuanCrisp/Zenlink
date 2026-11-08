"""Issue #2: pure-Python lzss/lzfse stand-ins for the native .pyd files that
Windows Smart App Control blocked."""

import importlib.machinery
import importlib.util
import os
import sys
import unittest

from zenlink import _lzss


def _native_lzss():
    # The real pylzss .pyd from site-packages, loaded under another name (the
    # "lzss" name in sys.modules is ours).
    for d in sys.path:
        if not os.path.isdir(d):
            continue
        for f in os.listdir(d):
            if f.startswith("lzss.") and f.endswith(".pyd"):
                loader = importlib.machinery.ExtensionFileLoader("lzss", os.path.join(d, f))
                spec = importlib.util.spec_from_loader("lzss", loader)
                ours = sys.modules.get("lzss")
                try:
                    mod = importlib.util.module_from_spec(spec)
                    loader.exec_module(mod)
                finally:  # loading the extension registers it; keep ours
                    if ours is not None:
                        sys.modules["lzss"] = ours
                return mod
    return None


SAMPLES = [
    b"",
    b"a",
    b"abcabcabcabcabcabcabcabcabc" * 50,
    b"     leading spaces match the pre-filled window",
    bytes(range(256)) * 40,
    os.urandom(5000),
    os.urandom(300) * 30,  # matches far back, near the 4078-byte limit
]


class LzssTests(unittest.TestCase):
    def test_round_trip(self):
        for data in SAMPLES:
            self.assertEqual(_lzss.decompress(_lzss.compress(data)), data)

    def test_compresses_repeats(self):
        data = b"zenlink " * 1000
        self.assertLess(len(_lzss.compress(data)), len(data) // 4)

    def test_byte_compatible_with_pylzss(self):
        native = _native_lzss()
        if native is None:
            self.skipTest("pylzss not installed")
        for data in SAMPLES:
            self.assertEqual(_lzss.decompress(native.compress(data)), data)
            self.assertEqual(native.decompress(_lzss.compress(data)), data)

    def test_pyimg4_gets_the_pure_modules(self):
        import zenlink  # noqa: F401 - installs the stand-ins

        import pyimg4.parser

        self.assertIsNone(getattr(sys.modules["lzss"], "__file__", None))
        self.assertIsNone(getattr(sys.modules["lzfse"], "__file__", None))
        self.assertIs(pyimg4.parser.lzss.decompress, _lzss.decompress)
        with self.assertRaises(RuntimeError):
            sys.modules["lzfse"].decompress(b"")


if __name__ == "__main__":
    unittest.main()
