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

import asyncio
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


def has_remote_record(udid: str) -> bool:
    from pymobiledevice3.pair_records import iter_remote_pair_records_by_identifier

    return any(identifier == udid for identifier, _path, _record in iter_remote_pair_records_by_identifier())


async def _open_ports(host: str, first: int = 49152, last: int = 65535) -> list[int]:
    """The RemotePairing port is dynamic; find the device's listening ports."""
    sem = asyncio.Semaphore(1500)
    found: list[int] = []

    async def probe(port: int) -> None:
        async with sem:
            try:
                _r, writer = await asyncio.wait_for(asyncio.open_connection(host, port), 0.8)
            except (OSError, asyncio.TimeoutError):
                return
            writer.close()
            found.append(port)

    await asyncio.gather(*(probe(port) for port in range(first, last + 1)))
    return sorted(found)


_port_cache: dict[str, int] = {}


async def _remotepairing_by_ip(serial: str, host: str):
    from pymobiledevice3.remote.tunnel_service import create_core_device_tunnel_service_using_remotepairing

    cached = _port_cache.get(serial)
    ports = [cached] if cached else await _open_ports(host)
    for attempt in range(2):
        for port in ports:
            try:
                service = await asyncio.wait_for(
                    create_core_device_tunnel_service_using_remotepairing(serial, host, port), 10)
            except Exception as error:  # noqa: BLE001 - not the RemotePairing port
                log.debug("wifi: %s:%s is not RemotePairing (%r)", host, port, error)
                continue
            _port_cache[serial] = port
            log.info("wifi: RemotePairing at %s:%s", host, port)
            return service
        if attempt == 0 and cached:  # the port changes when the device restarts
            _port_cache.pop(serial, None)
            ports = await _open_ports(host)
            cached = None
        else:
            break
    return None


async def _wifi_provider(serial, autopair, remotepairing_fallback=True):
    """Drop-in for userspace_tunnel._create_no_root_tunnel_provider, network only.

    Over Wi-Fi the device only offers the tunnel through RemotePairing
    (CoreDeviceProxy over a network lockdown connects, then the device drops
    the tunnel). Windows usually gets no Bonjour replies, so the device's IP
    comes from the LAN scan (lan.py) and its RemotePairing port from a scan.
    """
    from pymobiledevice3.exceptions import UserspaceTunnelUnavailableError
    from pymobiledevice3.remote import tunnel_service

    from . import lan

    if not has_remote_record(serial):
        raise UserspaceTunnelUnavailableError(
            "Wi-Fi isn't set up for this device yet. Connect the cable, unlock the device, press "
            "\"Set up Wi-Fi\" in Settings once, then unplug."
        )
    host = lan.host_for(serial)
    if host is None:
        await asyncio.to_thread(lan._run_scan)
        host = lan.host_for(serial)
    if host:
        service = await _remotepairing_by_ip(serial, host)
        if service is not None:
            return service, None
    services = await tunnel_service.get_remote_pairing_tunnel_services(udid=serial)
    if services:
        for extra in services[1:]:
            await extra.close()
        return services[0], None
    raise UserspaceTunnelUnavailableError(
        "The device wasn't found on Wi-Fi. Keep it unlocked and on the same network as this PC."
    )


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
