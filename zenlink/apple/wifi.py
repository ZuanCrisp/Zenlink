"""Wireless (Wi-Fi) connection for Apple Driver mode.

Same pipeline as the cable (CoreDevice DisplayService: HEVC, sound, touch,
keyboard); only the tunnel to the device goes over the network:

1. Apple Mobile Device Service lists the device on Wi-Fi (after "Wi-Fi
   connections" was enabled once over the cable) -> CoreDeviceProxy over
   that network lockdown (iOS 17.4+).
2. Otherwise RemotePairing over Bonjour (_remotepairing._tcp), using the
   RemotePairing record ZenLink writes during the one-time cable setup.

Setup (once, with the cable, device unlocked and trusted): `prepare(udid)`
turns on Wi-Fi connections and writes the RemotePairing record. Afterwards
the cable can be unplugged; PC and device must share the same network.
Nothing here touches Windows drivers.
"""

from __future__ import annotations

import logging

log = logging.getLogger(__name__)


async def _prepare(udid: str) -> dict:
    from pymobiledevice3.exceptions import RemotePairingCompletedError
    from pymobiledevice3.lockdown import create_using_usbmux
    from pymobiledevice3.remote.tunnel_service import RemotePairingLockdownService

    lockdown = await create_using_usbmux(serial=udid, autopair=True, connection_type="USB")
    try:
        await lockdown.set_enable_wifi_connections(True)
        paired = False
        try:
            service = await RemotePairingLockdownService.create(lockdown)
            try:
                await service.connect(autopair=True)
            except RemotePairingCompletedError:
                pass
            finally:
                await service.close()
            paired = True
        except Exception as error:  # noqa: BLE001 - iOS < 17 has no RemotePairing; Wi-Fi sync still works
            log.info("wifi: RemotePairing record not written: %s", error)
        return {"ok": True, "wifiConnections": True, "remotePairing": paired}
    finally:
        await lockdown.close()


def prepare(udid: str) -> dict:
    import asyncio

    return asyncio.run(_prepare(udid))


async def _wifi_provider(serial, autopair, remotepairing_fallback=True):
    """Drop-in for userspace_tunnel._create_no_root_tunnel_provider, network only."""
    from pymobiledevice3.exceptions import UserspaceTunnelUnavailableError
    from pymobiledevice3.lockdown import create_using_usbmux
    from pymobiledevice3.remote import tunnel_service

    try:
        lockdown = await create_using_usbmux(serial=serial, autopair=False, connection_type="Network")
    except Exception as error:  # noqa: BLE001
        log.info("wifi: not listed by Apple Mobile Device Service: %s", error)
    else:
        try:
            return await tunnel_service.CoreDeviceTunnelProxy.create(lockdown), lockdown
        except Exception as error:  # noqa: BLE001
            log.info("wifi: CoreDeviceProxy over Wi-Fi unavailable: %s", error)
            await lockdown.close()

    services = await tunnel_service.get_remote_pairing_tunnel_services(udid=serial)
    if not services:
        raise UserspaceTunnelUnavailableError(
            "The device wasn't found on Wi-Fi. Keep it unlocked on the same network as this PC, "
            "and run \"Set up Wi-Fi\" once with the cable connected."
        )
    for extra in services[1:]:
        await extra.close()
    return services[0], None


class WifiRsdTunnel:
    """Async context manager like PreferredRsdTunnel, but over the network."""

    def __init__(self, serial: str) -> None:
        self.serial = serial
        self._tunnel = None

    async def __aenter__(self):
        from pymobiledevice3.remote import userspace_tunnel

        original = userspace_tunnel._create_no_root_tunnel_provider
        userspace_tunnel._create_no_root_tunnel_provider = _wifi_provider
        try:
            self._tunnel = userspace_tunnel.UserspaceRsdTunnel(serial=self.serial, autopair=False)
            return await self._tunnel.aopen()
        finally:
            userspace_tunnel._create_no_root_tunnel_provider = original

    async def __aexit__(self, *exc) -> None:
        if self._tunnel is not None:
            await self._tunnel.aclose()
            self._tunnel = None
