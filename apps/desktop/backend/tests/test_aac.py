"""zlaac.dll (minimal FFmpeg AAC decoder) replaces PyAV for AAC-ELD sound.

Fixtures (tests/fixtures/eld_*.zlau) are raw AAC-ELD access units, 480-sample
frames, stereo, made by ZenshiroDev/resources/windows/native/zlaac/tools/make_eld_fixture.cpp from a
synthetic signal. The 44.1 kHz config is f8e85000, the same cookie the iPhone
uses for AirPlay sound (receiver.AAC_ELD_44K).
"""

from __future__ import annotations

import array
import hashlib
import math
import sys
import unittest
from pathlib import Path

FIX = Path(__file__).resolve().parent / "fixtures"

# SHA-256 of zlaac's s16 stereo 48 kHz output for each fixture.
EXPECTED = {
    "eld_44100.zlau": (386544, "262bc5efca00de7056a91b70ee11a5f51986bfe487e8e3f28442013922748ca8"),
    "eld_48000.zlau": (385920, "8803a77ac8924523b6d5072a85dd61817100f65ac44ddeb6284f8964110a8282"),
}


def _cookie(asc: bytes) -> bytes:
    c = bytearray(asc)
    c[2] |= 0x10
    return bytes(c)


@unittest.skipUnless(sys.platform == "win32", "zlaac.dll is a Windows build")
class ZlaacTests(unittest.TestCase):
    def decode(self, name):
        from zenlink.aac import AacDecoder, read_zlau

        asc, rate, aus = read_zlau((FIX / name).read_bytes())
        d = AacDecoder(_cookie(asc), rate)
        return asc, rate, [d.decode(au) for au in aus]

    def test_fixture_configs_match_apple(self):
        from zenlink.aac import read_zlau
        from zenlink.airplay.receiver import AAC_ELD_44K

        asc, rate, aus = read_zlau((FIX / "eld_44100.zlau").read_bytes())
        self.assertEqual((asc, rate), (AAC_ELD_44K, 44100))
        self.assertGreater(len(aus), 150)

    def test_output_is_pinned(self):
        for name, (size, sha) in EXPECTED.items():
            with self.subTest(name):
                _, _, chunks = self.decode(name)
                pcm = b"".join(chunks)
                self.assertEqual(len(pcm), size)
                self.assertEqual(hashlib.sha256(pcm).hexdigest(), sha)

    def test_480_sample_frames_at_48k(self):
        _, _, chunks = self.decode("eld_48000.zlau")
        # 480 stereo s16 frames per access unit (480 * 4 bytes), no resampling at 48 kHz.
        self.assertTrue(all(len(c) == 1920 for c in chunks))
        _, _, chunks = self.decode("eld_44100.zlau")
        # 44.1 -> 48 kHz: 480 * 48000 / 44100 = 522.4 frames per AU on average.
        avg = sum(len(c) for c in chunks[10:]) / 4 / len(chunks[10:])
        self.assertAlmostEqual(avg, 480 * 48000 / 44100, delta=1.0)

    def test_signal_is_the_synthetic_tones(self):
        """Left is mostly a 440 Hz sine, right 660 Hz (see make_eld_fixture.cpp)."""
        _, _, chunks = self.decode("eld_48000.zlau")
        pcm = array.array("h", b"".join(chunks))
        left, right = pcm[0::2][9600:57600], pcm[1::2][9600:57600]

        def power(x, hz):
            c = sum(v * math.cos(2 * math.pi * hz * i / 48000) for i, v in enumerate(x))
            s = sum(v * math.sin(2 * math.pi * hz * i / 48000) for i, v in enumerate(x))
            return (c * c + s * s) / len(x) ** 2

        self.assertGreater(power(left, 440), 20 * power(left, 660))
        self.assertGreater(power(right, 660), 20 * power(right, 440))
        rms = math.sqrt(sum(v * v for v in left) / len(left))
        self.assertGreater(rms, 5000)

    def test_equivalent_to_pyav(self):
        """Same decoder as the old PyAV path: same sizes, at most 1 LSB apart."""
        try:
            import av
        except ImportError:
            self.skipTest("PyAV not installed (only needed for this comparison)")
        from zenlink.aac import read_zlau

        for name in EXPECTED:
            with self.subTest(name):
                asc, rate, aus = read_zlau((FIX / name).read_bytes())
                ctx = av.codec.CodecContext.create("aac", "r")
                ctx.extradata = _cookie(asc)
                ctx.sample_rate = rate
                ctx.layout = "stereo"
                ctx.open()
                rs = av.AudioResampler(format="s16", layout="stereo", rate=48000)
                old = []
                for au in aus:
                    b = bytearray()
                    for frame in ctx.decode(av.Packet(au)):
                        for p in rs.resample(frame):
                            b += bytes(p.planes[0])[: p.samples * 4]
                    old.append(bytes(b))
                _, _, new = self.decode(name)
                self.assertEqual([len(c) for c in old], [len(c) for c in new])
                x, y = array.array("h", b"".join(old)), array.array("h", b"".join(new))
                self.assertLessEqual(max(abs(i - j) for i, j in zip(x, y)), 1)

    def test_bad_input(self):
        from zenlink.aac import AacDecoder, AacError

        with self.assertRaises(AacError):
            AacDecoder(b"", 48000, channels=7)
        d = AacDecoder(_cookie(bytes.fromhex("f8e65000")), 48000)
        self.assertEqual(d.decode(b""), b"")
        for junk in (b"\xff" * 3, bytes(range(200))):
            try:
                d.decode(junk)
            except AacError:
                pass
        d.close()
        d.close()

    def test_wrappers_use_zlaac(self):
        from zenlink.airplay.receiver import _AudioDecoder
        self.assertTrue(_AudioDecoder(bytes(16), bytes(16), 8)._ok)


if __name__ == "__main__":
    unittest.main()
