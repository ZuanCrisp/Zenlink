"""Local HTTP + WebSocket server the UI talks to (127.0.0.1 only).

/cast/* is the shared media API for both capture backends:
  Apple mode      relayed to pymobiledevice3's ScreenStreamServer: /cast/codec,
                  /cast/stream.bin (HEVC), /cast/audio.bin (PCM), POST
                  /cast/touch, /cast/key, /cast/button (input to the device);
  USB Direct mode served from the session's MediaHub: /cast/codec,
                  /cast/stream.bin, /cast/audio.bin (same framing). Input
                  isn't available on that path.
/api/connection and /api/drivers drive the Connection & Drivers settings.
"""

from __future__ import annotations

import asyncio
import logging
import os
import re

import aiohttp
from aiohttp import WSMsgType, web

from zenlink.apple import session
from zenlink.apple.models import set_custom_name
from zenlink.castmanager import CastManager
from zenlink.drivers import manager as driver_manager

SUPPORT_URL = "https://sociabuzz.com/zenvy/tribe"
CLIENT_KEY: web.AppKey["aiohttp.ClientSession"] = web.AppKey("client")
OCR_KEY: web.AppKey["OcrSession"] = web.AppKey("ocr")
log = logging.getLogger(__name__)

HOST = "127.0.0.1"
PORT = int(os.environ.get("ZENLINK_PORT") or 47421)  # beta port (stable uses 47321); same override as the Rust app
UPSTREAM = f"http://{session.STREAM_HOST}:{session.STREAM_PORT}"
_HOP = {"connection", "keep-alive", "transfer-encoding", "content-length", "content-encoding"}


@web.middleware
def build_app(manager: CastManager) -> web.Application:
    app = web.Application(client_max_size=4 * 1024 * 1024)
    routes = web.RouteTableDef()

    @routes.get("/")
    async def index(_request: web.Request) -> web.Response:
        return web.Response(text="ZenLink backend. Open ZenLink.exe.", content_type="text/plain")

    @routes.get("/api/state")
    async def state(_request: web.Request) -> web.Response:
        return web.json_response(manager.snapshot())

    @routes.post("/api/device/name")
    async def device_name(request: web.Request) -> web.Response:
        body = await request.json()
        set_custom_name(str(body.get("id", "")).replace("-", ""), str(body.get("name", "")))
        await asyncio.to_thread(manager.refresh_devices)
        return web.json_response(manager.snapshot())

    @routes.post("/api/device/select")
    async def device_select(request: web.Request) -> web.Response:
        body = await request.json()
        udid = str(body.get("id") or "")
        if udid and not re.fullmatch(r"[0-9A-Fa-f-]{8,64}", udid):
            return web.json_response({"error": "bad id"}, status=400)
        if body.get("forget"):
            return web.json_response(await asyncio.to_thread(manager.forget_device, udid))
        return web.json_response(await asyncio.to_thread(manager.select_device, udid or None))

    @routes.post("/api/cast/start")
    async def cast_start(request: web.Request) -> web.Response:
        body = await request.json() if request.can_read_body else {}
        return web.json_response(await asyncio.to_thread(manager.start, body.get("options") or {}))

    @routes.post("/api/cast/stop")
    async def cast_stop(_request: web.Request) -> web.Response:
        return web.json_response(await asyncio.to_thread(manager.stop))

    @routes.get("/api/connection")
    async def connection_get(_request: web.Request) -> web.Response:
        return web.json_response(manager.connection.snapshot())

    @routes.post("/api/connection")
    async def connection_set(request: web.Request) -> web.Response:
        body = await request.json()
        try:
            if "link" in body and "mode" not in body:
                return web.json_response(await asyncio.to_thread(manager.set_link, str(body["link"])))
            snap = await asyncio.to_thread(manager.choose_mode, str(body.get("mode")), bool(body.get("remember")))
        except ValueError as error:
            return web.json_response({"error": str(error)}, status=400)
        return web.json_response(snap)

    @routes.get("/api/drivers")
    async def drivers_get(_request: web.Request) -> web.Response:
        return web.json_response(await asyncio.to_thread(manager.refresh_drivers, True))

    @routes.post("/api/drivers/action")
    async def drivers_action(request: web.Request) -> web.Response:
        body = await request.json()
        if manager.state.get("phase") in ("connecting", "activating", "streaming"):
            return web.json_response({"ok": False, "message": "Stop casting before changing drivers."}, status=409)
        if not body.get("confirmed"):
            return web.json_response({"ok": False, "message": "The change was not confirmed."}, status=400)
        if manager.driver_busy:
            return web.json_response({"ok": False, "message": "Another driver change is running."}, status=409)
        manager.driver_busy = True
        try:
            result = await asyncio.to_thread(driver_manager.run_action, str(body.get("action")), str(body.get("instance")))
        except driver_manager.DriverActionError as error:
            result = {"ok": False, "message": str(error)}
        finally:
            manager.driver_busy = False
        await asyncio.to_thread(manager.refresh_drivers, True)
        return web.json_response(result)

    def keyboard_passcode(raw: str) -> str:
        from zenlink.apple import keyboard

        return keyboard.clean_passcode(raw)

    @routes.get("/api/passcode")
    async def passcode_get(_request: web.Request) -> web.Response:
        from zenlink import secret

        return web.json_response({"set": secret.has_secret("passcode")})

    @routes.post("/api/passcode")
    async def passcode_set(request: web.Request) -> web.Response:
        from zenlink import secret

        body = await request.json()
        code = keyboard_passcode(str(body.get("passcode") or ""))
        secret.set_secret("passcode", code)
        return web.json_response({"set": bool(code)})

    @routes.post("/api/ocr")
    async def ocr_capture(request: web.Request) -> web.Response:
        body = await request.json()
        try:
            result = await asyncio.to_thread(request.app[OCR_KEY].add_capture, str(body.get("image") or ""))
        except Exception as error:  # noqa: BLE001
            return web.json_response({"ok": False, "message": f"Text recognition failed: {error}"}, status=500)
        return web.json_response(result)

    @routes.post("/api/ocr/clear")
    async def ocr_clear(request: web.Request) -> web.Response:
        return web.json_response(request.app[OCR_KEY].clear())

    @routes.post("/api/keyboard/action")
    async def keyboard_action(request: web.Request) -> web.Response:
        from zenlink.apple import keyboard

        body = await request.json()
        if manager.state.get("phase") != "streaming" or manager.state.get("backend") not in ("apple", "hybrid"):
            return web.json_response({"ok": False, "message": "Shortcuts need Apple Driver mode while casting."}, status=409)
        try:
            from zenlink import secret

            passcode = secret.get_secret("passcode") if body.get("action") == "wake" else ""
            ok, message = await keyboard.run(request.app[CLIENT_KEY], UPSTREAM, str(body.get("action")),
                                             str(body.get("text") or ""), passcode,
                                             str((manager.state.get("device") or {}).get("family") or ""))
        except (aiohttp.ClientError, RuntimeError) as error:
            ok, message = False, f"The device didn't take the shortcut ({error})."
        return web.json_response({"ok": ok, "message": message}, status=200 if ok else 502)

    @routes.get("/api/audio")
    async def audio_get(_request: web.Request) -> web.Response:
        return web.json_response(manager.audio.stats())

    @routes.post("/api/audio")
    async def audio_set(request: web.Request) -> web.Response:
        body = await request.json()
        stats = await asyncio.to_thread(manager.set_audio, body.get("enabled"), body.get("volume"), body.get("muted"),
                                        body.get("route"))
        return web.json_response(stats)

    # Files to an iPhone/iPad (v2.5.1): Apple Driver mode only (usbmux).
    def files_device() -> dict | None:
        if manager.connection.active != "apple":
            return None
        dev = manager.state.get("device")
        if manager.state.get("backend") in ("apple", "hybrid") and dev and dev.get("id") != "airplay":
            return dev
        snap = manager.snapshot()
        return next((d for d in snap.get("devices", []) if d.get("ready") and d.get("id")), None)

    @routes.get("/api/files/apps")
    async def files_apps(_request: web.Request) -> web.Response:
        from zenlink.apple import files

        dev = files_device()
        if dev is None:
            return web.json_response({"ok": False, "message": "Needs Apple Driver mode and a connected device."},
                                     status=409)
        try:
            apps = await files.list_apps(dev["id"])
        except Exception as error:
            log.warning("files: app list failed: %s", error)
            return web.json_response({"ok": False, "message": files._short(error)}, status=502)
        return web.json_response({"ok": True, "device": dev.get("deviceName") or dev.get("name"), "apps": apps})

    @routes.post("/api/files/send")
    async def files_send(request: web.Request) -> web.Response:
        from pathlib import Path

        from zenlink.apple import files

        # A JSON content type keeps web pages from posting here (CORS preflight).
        if request.content_type != "application/json":
            return web.json_response({"ok": False, "message": "JSON only."}, status=415)
        body = await request.json()
        dev = files_device()
        local = Path(str(body.get("path") or ""))
        bundle = str(body.get("bundle") or "")
        if dev is None:
            return web.json_response({"ok": False, "message": "Needs Apple Driver mode and a connected device."},
                                     status=409)
        if not local.is_absolute() or not local.is_file() or not re.fullmatch(r"[A-Za-z0-9.\-]+", bundle):
            return web.json_response({"ok": False, "message": "Bad file or app."}, status=400)
        if files.job.get("state") == "running":
            return web.json_response({"ok": False, "message": "Another file is being sent."}, status=409)
        files.start(asyncio.get_running_loop(), dev["id"], bundle, str(body.get("app") or bundle), local)
        return web.json_response({"ok": True})

    @routes.get("/api/files/job")
    async def files_job(_request: web.Request) -> web.Response:
        from zenlink.apple import files

        return web.json_response(files.job)

    @routes.post("/api/client-log")
    async def client_log(request: web.Request) -> web.Response:
        try:
            body = await request.json()
        except ValueError:
            body = {}
        log.info("player: %s", {k: body.get(k) for k in (
            "codec", "decodedFps", "paintedFps", "decodeErrors", "rebuilds", "maxDecodeQueue",
            "latencyMs", "audioMs", "audio", "hardware", "renderer", "lastError")})
        return web.json_response({"ok": True})

    @routes.post("/api/app/show")
    async def app_show(_request: web.Request) -> web.Response:
        """A second launch asks the running instance to come to the front."""
        hook = getattr(manager, "show_hook", None)
        if hook is not None:
            await asyncio.to_thread(hook)
        return web.json_response({"ok": hook is not None})

    @routes.post("/api/app/restart")
    async def app_restart(_request: web.Request) -> web.Response:
        """Stop casting and restart the app. With a window, the window closes
        normally first (so WebView2 releases its data folder) and __main__
        starts the new instance; headless, the process restarts itself."""
        await asyncio.to_thread(manager.stop)
        hook = getattr(manager, "restart_hook", None)
        if hook is not None:
            asyncio.get_running_loop().call_later(0.3, hook)
        else:
            from zenlink import restart

            restart.spawn_new_instance()
            asyncio.get_running_loop().call_later(0.5, os._exit, 0)
        return web.json_response({"ok": True})

    @routes.get("/api/firewall")
    async def firewall_get(_request: web.Request) -> web.Response:
        from zenlink import firewall

        return web.json_response({"ok": await asyncio.to_thread(firewall.has_rule)})

    @routes.post("/api/firewall/allow")
    async def firewall_allow(_request: web.Request) -> web.Response:
        from zenlink import firewall

        if not await asyncio.to_thread(firewall.request_rule):
            return web.json_response({"ok": False, "message": "Windows didn't allow it (the permission prompt was declined)."})
        for _ in range(20):
            if await asyncio.to_thread(firewall.has_rule):
                return web.json_response({"ok": True})
            await asyncio.sleep(0.5)
        return web.json_response({"ok": False, "message": "The firewall rule wasn't added. Try again."})

    @routes.post("/api/wifi/prepare")
    async def wifi_prepare(_request: web.Request) -> web.Response:
        return web.json_response(await asyncio.to_thread(manager.prepare_wifi))

    @routes.post("/api/support")
    async def open_support(_request: web.Request) -> web.Response:
        """Open the author's support page (Sociabuzz) in the default browser."""
        import webbrowser

        webbrowser.open(SUPPORT_URL)
        return web.json_response({"ok": True})

    @routes.post("/api/drivers/zadig")
    async def drivers_zadig(request: web.Request) -> web.Response:
        """Open the official Zadig download page (not bundled: ZenLink only
        ships unmodified, redistributable driver files)."""
        body = await request.json()
        if not body.get("confirmed"):
            return web.json_response({"ok": False, "message": "Not confirmed."}, status=400)
        import webbrowser

        from zenlink.drivers import status as driver_status

        webbrowser.open(driver_status.ZADIG_URL)
        return web.json_response({"ok": True})

    @routes.route("*", "/cast/{tail:.*}")
    async def cast_route(request: web.Request) -> web.StreamResponse:
        hub = manager.hub
        if hub is not None:
            return await hub_route(request, hub)
        return await cast_proxy(request)

    async def hub_route(request: web.Request, hub) -> web.StreamResponse:
        tail = request.match_info["tail"]
        if tail == "codec":
            info = hub.codec()
            if info is None:
                return web.json_response({"error": "Waiting for the first video frame."}, status=503)
            return web.json_response(info, headers={"Cache-Control": "no-store"})
        if tail in ("stream.bin", "audio.bin"):
            loop = asyncio.get_running_loop()
            viewer = hub.open_video(loop) if tail == "stream.bin" else hub.open_audio(loop)
            resp = web.StreamResponse(headers={"Cache-Control": "no-store"})
            resp.content_type = "application/octet-stream"
            await resp.prepare(request)
            try:
                while True:
                    item = await viewer.queue.get()
                    if item is None:
                        break
                    await resp.write(item)
            except (ConnectionError, asyncio.CancelledError):
                pass
            finally:
                hub.close_viewer(viewer)
            return resp
        if tail == "pli":
            return web.json_response({"ok": True})
        if tail in ("touch", "key", "button") and manager.state.get("backend") == "hybrid":
            # Hybrid: the picture is AirPlay (hub), input goes to the CoreDevice
            # HID server on the same loopback port Apple mode uses.
            return await cast_proxy(request)
        return web.json_response({"error": "Touch, keyboard and buttons are not available in USB Direct mode."}, status=409)

    async def cast_proxy(request: web.Request) -> web.StreamResponse:
        url = f"{UPSTREAM}/{request.match_info['tail']}"
        if request.query_string:
            url += f"?{request.query_string}"
        body = await request.read() if request.can_read_body else None
        headers = {k: v for k, v in request.headers.items() if k.lower() in ("content-type", "accept")}
        client: aiohttp.ClientSession = request.app[CLIENT_KEY]
        try:
            upstream = await client.request(request.method, url, data=body, headers=headers)
        except aiohttp.ClientError as error:
            return web.json_response({"error": f"Screen stream is not running ({error})."}, status=503)
        async with upstream:
            if request.path.endswith(".bin"):
                # Live stream: relay chunks as they arrive, never buffer.
                resp = web.StreamResponse(status=upstream.status, headers={"Cache-Control": "no-store"})
                resp.content_type = upstream.content_type or "application/octet-stream"
                await resp.prepare(request)
                try:
                    async for chunk in upstream.content.iter_any():
                        await resp.write(chunk)
                except (ConnectionError, aiohttp.ClientError, asyncio.CancelledError):
                    pass
                return resp
            data = await upstream.read()
            out = {k: v for k, v in upstream.headers.items() if k.lower() not in _HOP}
            return web.Response(status=upstream.status, body=data, headers=out)

    @routes.get("/ws")
    async def events(request: web.Request) -> web.WebSocketResponse:
        ws = web.WebSocketResponse(compress=False)
        await ws.prepare(request)
        loop = asyncio.get_running_loop()
        queue: asyncio.Queue = asyncio.Queue()

        def push(message: str) -> None:
            loop.call_soon_threadsafe(queue.put_nowait, message)

        manager.add_listener(push)

        async def sender() -> None:
            while True:
                await ws.send_str(await queue.get())

        task = asyncio.create_task(sender())
        try:
            async for msg in ws:
                if msg.type in (WSMsgType.ERROR, WSMsgType.CLOSE):
                    break
        finally:
            manager.remove_listener(push)
            task.cancel()
        return ws

    async def client_ctx(app_: web.Application):
        timeout = aiohttp.ClientTimeout(total=None, sock_connect=5, sock_read=None)
        app_[CLIENT_KEY] = aiohttp.ClientSession(timeout=timeout, auto_decompress=False)
        yield
        await app_[CLIENT_KEY].close()

    from zenlink.ocr import OcrSession

    app[OCR_KEY] = OcrSession()
    app.cleanup_ctx.append(client_ctx)
    app.add_routes(routes)
    return app


def quiet_resets(loop: asyncio.AbstractEventLoop) -> None:
    """Windows' proactor logs a full traceback whenever a peer closes a socket
    abruptly (a window reloading its stream, a phone leaving Wi-Fi). Harmless:
    drop those, log everything else."""
    def handler(loop, context) -> None:
        if isinstance(context.get("exception"), (ConnectionResetError, ConnectionAbortedError, BrokenPipeError)):
            return
        loop.default_exception_handler(context)

    loop.set_exception_handler(handler)


def serve_forever(manager: CastManager, ready=None) -> None:
    async def main() -> None:
        quiet_resets(asyncio.get_running_loop())
        runner = web.AppRunner(build_app(manager), access_log=None)
        await runner.setup()
        await web.TCPSite(runner, HOST, PORT).start()
        if ready is not None:
            ready.set()
        await asyncio.Event().wait()

    asyncio.run(main())
