"""Send files to an iPhone/iPad (v2.5.1, Apple Driver mode only).

iOS has no shared Download folder that a PC can write to. What Apple's own
Finder/iTunes "File Sharing" uses is house_arrest: an app that declares
``UIFileSharingEnabled`` exposes its ``Documents`` folder over AFC, and the
file then shows up in that app and in the Files app under "On My iPhone".
So the user picks one of those apps (VLC, Documents, Pages …) and the file
is streamed into its Documents folder.

Needs Apple Mobile Device Service (usbmux) and a trusted device, i.e.
Apple Driver mode; ZenLink Studio replaces Apple's USB driver and has no
usbmux. Nothing here changes drivers or settings on the device.
"""

from __future__ import annotations

import asyncio
import logging
import os
import re
import time
from pathlib import Path

log = logging.getLogger(__name__)

DOCUMENTS = "/Documents"
CHUNK = 1 << 20  # 1 MB per AFC write: smooth progress, bounded memory

# The running (or last) transfer, read by GET /api/files/job.
job: dict = {"state": "idle", "name": "", "app": "", "sent": 0, "total": 0, "message": ""}


def safe_name(name: str) -> str | None:
    """A file name without folders or characters iOS/AFC would refuse."""
    clean = "".join("_" if (ord(c) < 32 or c in '"*/:<>?\\|') else c for c in name).strip().rstrip(".")
    if not clean or set(clean) <= {".", "_"}:
        return None
    if len(clean.encode()) > 200:
        stem, ext = os.path.splitext(clean)
        ext = ext if len(ext) <= 10 else ""
        while len((stem + ext).encode()) > 200:
            stem = stem[:-1]
        clean = stem + ext
    return clean


def numbered(name: str, n: int) -> str:
    stem, ext = os.path.splitext(name)
    if not stem:  # ".bashrc"
        stem, ext = name, ""
    return f"{stem} ({n}){ext}"


def sharing_apps(apps: dict) -> list[dict]:
    """Apps from installation_proxy whose Documents folder is shared."""
    out = []
    for bundle, info in apps.items():
        if not info.get("UIFileSharingEnabled"):
            continue
        name = info.get("CFBundleDisplayName") or info.get("CFBundleName") or bundle
        out.append({"bundle": bundle, "name": str(name)})
    return sorted(out, key=lambda a: a["name"].lower())


async def _lockdown(udid: str):
    from pymobiledevice3.lockdown import create_using_usbmux

    # Already trusted for Apple Driver mode: never start a new pairing here.
    return await create_using_usbmux(serial=udid, autopair=False)


async def list_apps(udid: str) -> list[dict]:
    from pymobiledevice3.services.installation_proxy import InstallationProxyService

    lockdown = await _lockdown(udid)
    try:
        proxy = InstallationProxyService(lockdown)
        try:
            apps = await proxy.get_apps(application_type="User")
        finally:
            await proxy.close()
    finally:
        await lockdown.close()
    return sharing_apps(apps)


async def send(udid: str, bundle: str, app_name: str, local: Path) -> str:
    """Streams `local` into the app's Documents folder; returns the name used."""
    from pymobiledevice3.services.house_arrest import HouseArrestService

    name = safe_name(local.name)
    if name is None:
        raise ValueError("unusable file name")
    total = local.stat().st_size
    job.update(state="running", name=name, app=app_name, sent=0, total=total, message="")
    lockdown = await _lockdown(udid)
    try:
        afc = await HouseArrestService.create(lockdown, bundle, documents_only=True)
        try:
            remote_name = name
            n = 0
            while await afc.exists(f"{DOCUMENTS}/{remote_name}"):
                n += 1
                if n > 999:
                    raise RuntimeError("no free file name")
                remote_name = numbered(name, n)
            handle = await afc.fopen(f"{DOCUMENTS}/{remote_name}", "w")
            try:
                with local.open("rb") as f:
                    while chunk := f.read(CHUNK):
                        await afc.fwrite(handle, chunk)
                        job["sent"] += len(chunk)
            finally:
                await afc.fclose(handle)
        finally:
            await afc.close()
    finally:
        await lockdown.close()
    job.update(state="done", name=remote_name)
    return remote_name


def start(loop: asyncio.AbstractEventLoop | None, udid: str, bundle: str, app_name: str, local: Path) -> None:
    """Runs `send` as a task; the result lands in `job`."""

    async def run() -> None:
        t = time.monotonic()
        try:
            saved = await send(udid, bundle, app_name, local)
            log.info("files: %s (%d bytes) to %s/Documents/%s in %.1f s", local, job["total"], bundle, saved,
                     time.monotonic() - t)
        except Exception as error:  # reported to the window, never fatal
            log.warning("files: sending %s to %s failed: %s", local, bundle, error)
            job.update(state="error", message=_short(error))

    job.update(state="running", name=local.name, app=app_name, sent=0, total=0, message="")
    asyncio.ensure_future(run(), loop=loop)


def _short(error: Exception) -> str:
    text = str(error) or type(error).__name__
    return re.sub(r"\s+", " ", text)[:200]
