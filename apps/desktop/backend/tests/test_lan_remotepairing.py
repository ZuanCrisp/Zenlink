"""Wi-Fi discovery of iOS 27 devices, which close lockdown on TCP 62078: the
_remotepairing._tcp authTag identifies a paired device by its stored altIRK."""

import base64
import unittest

from pymobiledevice3.remote.siphash import compute_auth_tag

from zenlink.apple import lan

IRK_PHONE = bytes(range(16))
IRK_PAD = bytes(range(16, 32))
IDENT = "D45C330B-B1B7-4E60-842C-C3ED8C81C91B"


def _tag(irk: bytes) -> str:
    return base64.b64encode(compute_auth_tag(irk, IDENT)).decode()


class MatchRemotePairing(unittest.TestCase):
    def test_tag_picks_the_right_record(self):
        irks = {"pad": IRK_PAD, "phone": IRK_PHONE}
        hits = lan._match_remotepairing([(IDENT, _tag(IRK_PHONE), "192.168.1.6", 49152)], irks)
        self.assertEqual(hits, [{"udid": "phone", "ip": "192.168.1.6", "port": 49152}])

    def test_unpaired_device_is_ignored(self):
        other = bytes(16)
        self.assertEqual(lan._match_remotepairing([(IDENT, _tag(other), "192.168.1.9", 49152)], {"phone": IRK_PHONE}), [])

    def test_garbage_tag_is_ignored(self):
        self.assertEqual(lan._match_remotepairing([(IDENT, "!!", "192.168.1.9", 1)], {"phone": IRK_PHONE}), [])


if __name__ == "__main__":
    unittest.main()
