"""Fake ZenLink backend for testing the Rust client without a device.

Serves the same protocol as apps/desktop/backend/zenlink/server.py (ZenshiroDev/docs/rust/AUDIT.md section 2)
from an H.264 MP4, looping, paced at the file's frame rate.

    python ZenshiroDev/tooling/dev/fake_backend.py VIDEO.mp4 [--port 47399] [--rotate-after SECONDS]

Then run the app with ZENLINK_PORT=47399.
"""

import argparse
import asyncio
import base64
import json
import struct
import random
import time

import av
from aiohttp import web


def load(path):
    c = av.open(path)
    s = c.streams.video[0]
    extradata = bytes(s.codec_context.extradata or b"")
    fps = float(s.average_rate or 60)
    packets = [(bytes(p), bool(p.is_keyframe)) for p in c.demux(s) if p.size]
    c.close()
    if s.codec_context.name == "hevc":
        codec = "hvc1.1.6.L153.B0"  # hvcC extradata, like Apple mode
    else:
        profile, compat, level = extradata[1], extradata[2], extradata[3]
        codec = f"avc1.{profile:02x}{compat:02x}{level:02x}"
    return {"codec": codec, "description": base64.b64encode(extradata).decode(), "keyframesOnRequest": True}, packets, fps


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("video")
    ap.add_argument("--video2", help="switch to this file every --rotate-after seconds (orientation change)")
    ap.add_argument("--port", type=int, default=47399)
    ap.add_argument("--rotate-after", type=float, default=0)
    ap.add_argument("--needs-choice", action="store_true", help="start with no connection mode (chooser shows)")
    ap.add_argument("--steps", type=float, default=0, help="seconds per connect phase (connecting, activating) before streaming")
    ap.add_argument("--fail", action="store_true", help="fail during activating (with --steps)")
    ap.add_argument("--jitter", type=float, default=0,
                    help="send each frame up to this many ms late (random, not cumulative), like a real link")
    ap.add_argument("--need", help="device needs this first (restore, bind, removefilter, trust, ...)")
    ap.add_argument("--no-device", action="store_true", help="no device plugged in (the Plug in screen)")
    ap.add_argument("--mode", default="apple", choices=["apple", "usbdirect"], help="active connection mode")
    a = ap.parse_args()
    clips = [load(a.video)] + ([load(a.video2)] if a.video2 else [])
    state = {"phase": "idle", "detail": "", "device": {"name": "iPhone (fake)"}, "error": None, "rotation": 0,
             "devices": [{"id": "fake", "name": "iPhone (fake)", "wifi": False}], "service": True,
             "connection": {"active": "apple", "link": "wired", "remember": True, "needsChoice": False}, "capture": None, "nativeAudio": False}
    log = {"touch": 0, "button": []}
    state["connection"]["active"] = a.mode
    if a.no_device:
        state.update(device=None, devices=[])
    if a.needs_choice:
        state["connection"].update(active=None, needsChoice=True)
    drv_mode = {"restore": "replaced", "bind": "apple"}.get(a.need or "", "apple")
    state["drivers"] = {"libusb0": {"available": True}, "devices": [{
        "instance": "USB\\VID_05AC&PID_12A8\\FAKE", "serial": "FAKE", "family": "iPhone",
        "mode": drv_mode, "unsafeFilter": a.need == "removefilter"}]}
    if a.no_device:
        pass
    elif a.need:
        dev = state["devices"][0]
        dev["ready"] = False
        if a.need != "trust":
            dev["need"] = a.need
    else:
        state["devices"][0]["ready"] = True

    async def drivers_action(request):
        body = await request.json()
        log["driver"] = body
        await asyncio.sleep(1.5)
        state["devices"][0].pop("need", None)
        state["devices"][0]["ready"] = True
        state["drivers"]["devices"][0].update(mode="replaced" if body.get("action") == "bind" else "apple", unsafeFilter=False)
        return web.json_response({"ok": True, "message": "Driver changed (fake)."})

    async def restart(_):
        log["restart"] = log.get("restart", 0) + 1
        return web.json_response({"ok": True})

    async def conn_get(_):
        return web.json_response(state["connection"])

    async def conn_set(request):
        body = await request.json()
        c = state["connection"]
        if "link" in body and "mode" not in body:
            c["link"] = body["link"]
        else:
            c.update(active=body.get("mode"), remember=bool(body.get("remember")), needsChoice=False)
            return web.json_response(state)  # like the real backend: the whole state ("error": null)
        return web.json_response(c)

    async def get_state(_):
        return web.json_response(state)

    async def run_steps():
        for ph in ("connecting", "activating"):
            state["phase"] = ph
            await asyncio.sleep(a.steps)
            if state["phase"] != ph:
                return
        if a.fail:
            state.update(phase="error", error="Unlock the device and tap Trust, then try again.", capture=None)
            return
        state["phase"] = "streaming"

    async def start(_):
        state.update(error=None, capture={"backend": "apple"})
        if a.steps > 0:
            asyncio.ensure_future(run_steps())
        else:
            state["phase"] = "streaming"
        return web.json_response({"ok": True})

    async def stop(_):
        state.update(phase="idle", capture=None)
        return web.json_response({"ok": True})

    async def codec(_):
        return web.json_response(clips[0][0])

    async def stream(request):
        resp = web.StreamResponse()
        resp.content_type = "application/octet-stream"
        await resp.prepare(request)
        t0 = time.perf_counter()
        n = 0
        clip = 0
        switched = time.perf_counter()
        while state["phase"] == "streaming":
            info, packets, fps = clips[clip]
            for data, key in packets:
                if state["phase"] != "streaming":
                    break
                if a.rotate_after and len(clips) > 1 and time.perf_counter() - switched > a.rotate_after and key:
                    clip = (clip + 1) % len(clips)
                    switched = time.perf_counter()
                    cfg = json.dumps(clips[clip][0]).encode()
                    await resp.write(struct.pack(">IB", len(cfg) + 1, 3) + cfg)
                    break
                await resp.write(struct.pack(">IB", len(data) + 1, 0 if key else 1) + data)
                n += 1
                wait = t0 + n / fps - time.perf_counter()
                if a.jitter:
                    wait += random.uniform(0, a.jitter / 1000)
                if wait > 0:
                    await asyncio.sleep(wait)
        return resp

    async def touch(request):
        log["touch"] += 1
        return web.json_response({"ok": True})

    async def button(request):
        log["button"].append((await request.json()).get("name"))
        print("button", log["button"][-1], flush=True)
        return web.json_response({"ok": True})

    async def ok(_):
        return web.json_response({"ok": True})

    async def debug(_):
        return web.json_response(log)

    app = web.Application()
    app.add_routes([
        web.get("/api/state", get_state), web.post("/api/cast/start", start), web.post("/api/cast/stop", stop),
        web.get("/cast/codec", codec), web.get("/cast/stream.bin", stream), web.post("/cast/touch", touch),
        web.post("/cast/button", button), web.post("/cast/pli", ok), web.post("/api/audio", ok),
        web.post("/api/keyboard/action", ok), web.get("/api/drivers", ok), web.post("/api/wifi/prepare", ok),
        web.get("/debug", debug), web.post("/api/drivers/action", drivers_action), web.post("/api/app/restart", restart), web.get("/api/connection", conn_get), web.post("/api/connection", conn_set),
    ])
    web.run_app(app, host="127.0.0.1", port=a.port, print=lambda *_: print(f"fake backend on {a.port}", flush=True))


if __name__ == "__main__":
    main()
