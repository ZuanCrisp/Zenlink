"""The bundled scrcpy server must match the version and checksum pinned in Rust."""

import hashlib
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]  # apps/desktop/backend/tests -> repository root
RUST = ROOT / "ZenshiroDev" / "crates" / "zenlink-android" / "src" / "scrcpy.rs"
SCRCPY = ROOT / "ZenshiroDev" / "resources" / "third_party" / "scrcpy"


def _const(name: str) -> str:
    m = re.search(rf'pub const {name}: &str = "([^"]+)";', RUST.read_text(encoding="utf-8"))
    assert m, name
    return m.group(1)


class ScrcpyPinTests(unittest.TestCase):
    def test_server_checksum_matches_rust_pin(self):
        server = SCRCPY / _const("SERVER_FILE")
        digest = hashlib.sha256(server.read_bytes()).hexdigest()
        self.assertEqual(digest, _const("SERVER_SHA256"))

    def test_checksum_is_in_signed_sums_file(self):
        sums = (SCRCPY / "SHA256SUMS.txt").read_text(encoding="utf-8")
        self.assertIn(f"{_const('SERVER_SHA256')}  {_const('SERVER_FILE')}", sums)

    def test_file_name_matches_version(self):
        self.assertEqual(_const("SERVER_FILE"), f"scrcpy-server-v{_const('SCRCPY_VERSION')}")

    def test_licence_is_kept(self):
        self.assertIn("Apache License", (SCRCPY / "LICENSE").read_text(encoding="utf-8"))

    def test_third_party_notes_list_the_checksum(self):
        notes = (ROOT / "ZenshiroDev" / "docs" / "THIRD_PARTY.md").read_text(encoding="utf-8")
        self.assertIn(_const("SERVER_SHA256"), notes)


if __name__ == "__main__":
    unittest.main()
