"""Find iPhones/iPads on the local network without Bonjour.

Windows often never delivers mDNS replies (no Bonjour Service, a "Public"
network profile, or another program owning UDP 5353), so Apple Mobile
Device Service never lists the device on Wi-Fi. A device with Wi-Fi
connections enabled still listens on its lockdown port (TCP 62078), so:

1. probe the PC's own /24 subnets for TCP 62078 (a short connect each);
2. open lockdown with this PC's pair record and read the device's UDID,
   name and model (the device only answers a PC it trusts).

Results are cached; `scan_async()` refreshes them in the background so the
device list never waits on the network.
"""

from __future__ import annotations

import asyncio
import ipaddress
import logging
import os
import plistlib
import threading
import time
from pathlib import Path

log = logging.getLogger(__name__)

LOCKDOWN_PORT = 62078
_found: dict[str, dict] = {}   # udid -> {"ip", "name", "productType", "version", "deviceClass", "seen"}
_lock = threading.Lock()
_scanning = threading.Event()
_last_scan = 0.0
RESCAN_S = 20.0
FORGET_S = 90.0


def _lockdown_dir() -> Path:
    return Path(os.environ.get("ALLUSERSPROFILE") or r"C:\ProgramData") / "Apple" / "Lockdown"


def pair_record(udid: str) -> dict | None:
    path = _lockdown_dir() / f"{udid}.plist"
    try:
        with open(path, "rb") as fh:
            return plistlib.load(fh)
    except OSError:
        return None


def _any_pair_records() -> list[tuple[str, dict]]:
    out = []
    try:
        for path in _lockdown_dir().glob("*.plist"):
            if path.stem == "SystemConfiguration":
                continue
            try:
                with open(path, "rb") as fh:
                    out.append((path.stem, plistlib.load(fh)))
            except (OSError, ValueError):
                continue
    except OSError:
        pass
    return out


def _subnets() -> list[ipaddress.IPv4Network]:
    import ifaddr

    nets = []
    for adapter in ifaddr.get_adapters():
        name = f"{adapter.nice_name} {adapter.name}".lower()
        if any(skip in name for skip in ("vethernet", "virtual", "vmware", "hyper-v", "loopback", "wsl", "vpn", "tap")):
            continue
        for ip in adapter.ips:
            if not isinstance(ip.ip, str):
                continue
            addr = ipaddress.IPv4Address(ip.ip)
            if not addr.is_private or addr.is_loopback or addr.is_link_local:
                continue
            net = ipaddress.IPv4Network(f"{addr}/{max(ip.network_prefix, 24)}", strict=False)
            if net not in nets:
                nets.append(net)
    return nets


# An iPhone whose Wi-Fi is in power save answers the first SYN after 2-4 s
# (measured); with a shorter wait only a charging iPad was ever found.
PROBE_TIMEOUT_S = 4.0


async def _port_open(ip: str, timeout: float = PROBE_TIMEOUT_S) -> bool:
    try:
        _reader, writer = await asyncio.wait_for(asyncio.open_connection(ip, LOCKDOWN_PORT), timeout)
    except (OSError, asyncio.TimeoutError):
        return False
    writer.close()
    return True


async def _identify(ip: str) -> dict | None:
    from pymobiledevice3.lockdown import create_using_tcp

    for udid, record in _any_pair_records():
        try:
            client = await asyncio.wait_for(
                create_using_tcp(ip, identifier=udid, autopair=False, pair_record=record), 6)
        except Exception:  # noqa: BLE001 - not this device's record, or not trusted
            continue
        try:
            values = client.all_values or {}
            real = str(values.get("UniqueDeviceID") or "")
            if not real:
                continue
            return {
                "udid": real,
                "ip": ip,
                "name": values.get("DeviceName"),
                "productType": values.get("ProductType"),
                "version": values.get("ProductVersion"),
                "deviceClass": values.get("DeviceClass"),
                "paired": pair_record(real) is not None,
            }
        finally:
            try:
                await client.close()
            except Exception:  # noqa: BLE001
                pass
    return None


def _match_remotepairing(answers: list[tuple[str, str, str, int]], alt_irks: dict[str, bytes]) -> list[dict]:
    """(identifier, authTag, ip, port) Bonjour answers -> paired devices. The device proves who it
    is through authTag, derived from the altIRK stored in its RemotePairing record."""
    from pymobiledevice3.remote.tunnel_service import validate_auth_tag

    out = []
    for identifier, tag, ip, port in answers:
        for udid, irk in alt_irks.items():
            if validate_auth_tag(irk, identifier, tag):
                out.append({"udid": udid, "ip": ip, "port": port})
                break
    return out


def _remotepairing_devices(timeout: float = 3.0) -> list[dict]:
    """Paired devices advertising _remotepairing._tcp. iOS 27 closes lockdown on TCP 62078 over
    Wi-Fi, so `_identify` never recognises it; RemotePairing still works."""
    from pymobiledevice3.pair_records import iter_remote_pair_records_by_identifier
    from pymobiledevice3.remote.tunnel_service import PEER_ALT_IRK_KEY
    from zeroconf import ServiceBrowser, Zeroconf

    irks = {u: r[PEER_ALT_IRK_KEY] for u, _p, r in iter_remote_pair_records_by_identifier() if r.get(PEER_ALT_IRK_KEY)}
    if not irks:
        return []
    service = "_remotepairing._tcp.local."
    zc = Zeroconf()
    names: list[str] = []

    class Listener:
        def add_service(self, _zc, _type, name):
            names.append(name)

        def update_service(self, *_a):
            pass

        def remove_service(self, *_a):
            pass

    try:
        ServiceBrowser(zc, service, Listener())
        time.sleep(timeout)
        answers = []
        for name in names:
            info = zc.get_service_info(service, name, timeout=1500)
            if not info or not info.port:
                continue
            props = {k.decode(): (v or b"").decode(errors="replace") for k, v in info.properties.items()}
            ipv4 = [a for a in info.parsed_addresses() if ":" not in a]
            if props.get("identifier") and props.get("authTag") and ipv4:
                answers.append((props["identifier"], props["authTag"], ipv4[0], info.port))
        return _match_remotepairing(answers, irks)
    except Exception:  # noqa: BLE001 - no Bonjour on this network: the 62078 scan still runs
        log.debug("lan: remotepairing browse failed", exc_info=True)
        return []
    finally:
        zc.close()


def _family(udid: str) -> str:
    from . import models

    learned = models.model_name(udid.replace("-", ""), "iOS device")
    return "iPad" if learned.startswith("iPad") else "iPhone" if learned.startswith("iPhone") else ""


async def _scan() -> None:
    hosts = []
    for net in _subnets():
        hosts += [str(h) for h in net.hosts()][:1022]
    sem = asyncio.Semaphore(256)  # a /24 in one round: the scan takes about PROBE_TIMEOUT_S

    async def probe(ip: str) -> str | None:
        async with sem:
            return ip if await _port_open(ip) else None

    open_ips = [ip for ip in await asyncio.gather(*(probe(h) for h in hosts)) if ip]
    now = time.monotonic()
    known = {v["ip"]: k for k, v in _found.items()}
    for ip in open_ips:
        info = None
        if ip in known:
            with _lock:
                _found[known[ip]]["seen"] = now
            continue
        info = await _identify(ip)
        if info:
            info["seen"] = now
            with _lock:
                _found[info["udid"]] = info
            log.info("lan: %s (%s) at %s", info.get("name"), info["udid"], ip)
    from . import wifi

    for hit in await asyncio.to_thread(_remotepairing_devices):
        udid = hit["udid"]
        wifi._port_cache[udid] = hit["port"]  # the tunnel skips its port scan
        with _lock:
            old = _found.get(udid, {})
            _found[udid] = {
                **old,
                "udid": udid,
                "ip": hit["ip"],
                "deviceClass": old.get("deviceClass") or _family(udid),
                "paired": pair_record(udid) is not None,
                "seen": now,
            }
        if not old:
            log.info("lan: %s at %s:%s (RemotePairing)", udid, hit["ip"], hit["port"])
    with _lock:
        for udid in [u for u, v in _found.items() if now - v["seen"] > FORGET_S]:
            _found.pop(udid, None)


def _run_scan() -> None:
    global _last_scan
    try:
        asyncio.run(_scan())
    except Exception:  # noqa: BLE001
        log.warning("lan scan failed", exc_info=True)
    finally:
        _last_scan = time.monotonic()
        _scanning.clear()


def scan_async(force: bool = False) -> None:
    """Refresh the cache in the background (at most every RESCAN_S)."""
    if _scanning.is_set() or (not force and time.monotonic() - _last_scan < RESCAN_S):
        return
    _scanning.set()
    threading.Thread(target=_run_scan, name="lan-scan", daemon=True).start()


def devices() -> list[dict]:
    with _lock:
        return [dict(v) for v in _found.values()]


def host_for(udid: str) -> str | None:
    with _lock:
        hit = _found.get(udid)
    return hit["ip"] if hit else None
