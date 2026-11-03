"""Keyboard shortcuts sent to the device through the Apple stream server's
HID endpoints (/key with HID keyboard usages, /button with named buttons).

Actions:
  copy, paste, cut, select-all, undo   Cmd + C / V / X / A / Z on the device
  paste-pc                             type the PC clipboard text (US layout)
  toggle-keyboard                      show / hide the on-screen keyboard (Eject)
  lock                                 restore brightness if dimmed, then sleep
  wake                                 wake, swipe up and type the saved passcode
  stealth                              dim the device's own screen to 0% / restore
                                       (the mirrored picture stays full quality)
"""

from __future__ import annotations

import asyncio

import aiohttp

CMD = 0xE3
SHIFT = 0xE1
_CMD_KEYS = {"copy": 0x06, "paste": 0x19, "cut": 0x1B, "select-all": 0x04, "undo": 0x1D}
_BUTTONS = {"toggle-keyboard": "eject"}
_state = {"dimmed": False}
_DIGITS = {str(d): 0x1E + d - 1 for d in range(1, 10)} | {"0": 0x27}
MAX_TEXT = 4000

_PLAIN = "1234567890-=[]\\;',./`"
_SHIFTED = "!@#$%^&*()_+{}|:\"<>?~"
_PLAIN_CODES = [0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
                0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x33, 0x34, 0x36, 0x37, 0x38, 0x35]


def char_usage(ch: str) -> tuple[int, bool] | None:
    """(HID usage, needs shift) for one character on a US layout."""
    if "a" <= ch <= "z":
        return 0x04 + ord(ch) - ord("a"), False
    if "A" <= ch <= "Z":
        return 0x04 + ord(ch) - ord("A"), True
    if ch in _PLAIN:
        return _PLAIN_CODES[_PLAIN.index(ch)], False
    if ch in _SHIFTED:
        return _PLAIN_CODES[_SHIFTED.index(ch)], True
    return {" ": (0x2C, False), "\n": (0x28, False), "\t": (0x2B, False)}.get(ch)


def install_buttons() -> None:
    """Add Eject (on-screen keyboard toggle) to the stream server's buttons."""
    from pymobiledevice3.remote.core_device import screen_stream

    screen_stream._NAMED_BUTTONS.setdefault("eject", (0x0C, 0xB8, 0.05))
    screen_stream._NAMED_BUTTONS.setdefault("brightness-down", (0x0C, 0x70, 0.025))
    screen_stream._NAMED_BUTTONS.setdefault("brightness-up", (0x0C, 0x6F, 0.025))
    _state["dimmed"] = False


async def run(client: aiohttp.ClientSession, base: str, action: str, text: str = "", passcode: str = "") -> tuple[bool, str]:
    async def key(usages):
        async with client.post(f"{base}/key", json={"usages": usages}) as r:
            if r.status >= 400:
                raise RuntimeError(f"key {r.status}")

    async def button(name):
        async with client.post(f"{base}/button", json={"name": name, "state": "press"}) as r:
            if r.status >= 400:
                raise RuntimeError(f"button {r.status}")

    async def touch(kind, x, y):
        async with client.post(f"{base}/touch", json={"type": kind, "x": x, "y": y}) as r:
            if r.status >= 400:
                raise RuntimeError(f"touch {r.status}")

    if action == "stealth":
        # 16 steps down reach 0%; restoring sends 4 steps up (~25%), and only
        # when this app dimmed it, so the screen never jumps to full.
        if not _state["dimmed"]:
            for _ in range(16):
                await button("brightness-down")
                await asyncio.sleep(0.03)
            _state["dimmed"] = True
            return True, "dimmed"
        for _ in range(4):
            await button("brightness-up")
            await asyncio.sleep(0.03)
        _state["dimmed"] = False
        return True, "restored"
    if action == "lock":
        if _state["dimmed"]:
            await run(client, base, "stealth")
        await button("lock")
        return True, ""
    if action == "wake":
        await button("home")
        await asyncio.sleep(0.45)
        await touch("contact", 32768, 58000)
        for i in range(1, 9):
            await asyncio.sleep(0.025)
            await touch("contact", 32768, int(58000 - 46000 * i / 8))
        await touch("release", 32768, 12000)
        pin = "".join(ch for ch in passcode if ch.isdigit())
        if pin:
            await asyncio.sleep(0.5)
            for digit in pin:
                await key([_DIGITS[digit]])
                await asyncio.sleep(0.04)
                await key([])
                await asyncio.sleep(0.06)
        return True, "unlocked" if pin else "woken"

    if action in _CMD_KEYS:
        await key([])
        await key([CMD])
        await asyncio.sleep(0.03)
        await key([CMD, _CMD_KEYS[action]])
        await asyncio.sleep(0.05)
        await key([CMD])
        await key([])
        return True, ""
    if action in _BUTTONS:
        async with client.post(f"{base}/button", json={"name": _BUTTONS[action], "state": "press"}) as r:
            return r.status < 400, "" if r.status < 400 else f"button {r.status}"
    if action == "paste-pc":
        text = text.replace("\r\n", "\n")[:MAX_TEXT]
        skipped = 0
        for ch in text:
            mapped = char_usage(ch)
            if mapped is None:
                skipped += 1
                continue
            usage, shift = mapped
            await key([SHIFT, usage] if shift else [usage])
            await asyncio.sleep(0.008)
            await key([])
            await asyncio.sleep(0.006)
        note = f"{skipped} characters can't be typed on a US keyboard and were skipped." if skipped else ""
        return True, note
    return False, f"Unknown action: {action}"
