"""What Hybrid (iOS 17-26) shares with ZenLink's server: the loopback port of its control
server (/touch, /key, /button, proxied under /cast/) and the userspace-tunnel reset.
Apple Driver mode for iOS 27+ runs in the ZenLink window (Rust, zenlink-apple).
"""

from __future__ import annotations

import asyncio
import contextlib
import logging

log = logging.getLogger(__name__)

STREAM_HOST = "127.0.0.1"
STREAM_PORT = 47422  # beta (stable v2.2.31 uses 47322)


async def _reset_userspace_tunnel() -> None:
    """Close a userspace tunnel a previous session left behind."""
    try:
        from pymobiledevice3.remote import tunnel_service, userspace_tunnel

        active = getattr(userspace_tunnel, "_active_tunnel", None)
        if active is not None and hasattr(active, "aclose"):
            with contextlib.suppress(Exception):
                await asyncio.wait_for(active.aclose(), timeout=1.5)
        userspace_tunnel._active_tunnel = None
        userspace_tunnel.USERSPACE_ACTIVE = False
        tunnel_service.USE_USERSPACE_TUNNEL = False
        lock = getattr(userspace_tunnel, "_lifecycle_locks", {}).get(asyncio.get_running_loop())
        if lock is not None and lock.locked():
            with contextlib.suppress(Exception):
                lock.release()
    except Exception as error:
        log.debug("tunnel reset: %s", error)
