"""Audit 2.5.11 regressions in the sound settings."""

import unittest


class AudioRouteTests(unittest.TestCase):
    """Audit 2.5.11: `enabled` alone (classic window) must still stop and
    start the PC sound; only the device volume needs a route."""

    def test_enabled_without_route_stops_and_starts_sound(self):
        from unittest import mock

        from zenlink.castmanager import CastManager

        m = CastManager()
        with mock.patch.object(m, "_stop_sound") as stop, mock.patch.object(m, "_start_sound") as start, \
                mock.patch.object(m, "_apply_device_volume") as volume:
            m.set_audio(enabled=False)
            stop.assert_called_once()
            m.state["phase"] = "streaming"
            m.set_audio(enabled=True)
            start.assert_called_once()
            volume.assert_not_called()
            m.set_audio(route="device")
            self.assertFalse(m.audio_enabled)
            volume.assert_called_once()


if __name__ == "__main__":
    unittest.main()
