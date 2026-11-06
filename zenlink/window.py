"""Native window controls for the frameless ZenLink window (pywebview js_api).

pywebview's Window methods marshal onto the UI thread, so they are safe from
the js_api worker thread. Dragging uses pywebview's own drag regions
(.pywebview-drag-region); only edge resizing needs a Win32 message.
"""

from __future__ import annotations

import ctypes
from ctypes import wintypes

_user32 = ctypes.windll.user32
_user32.MonitorFromWindow.restype = ctypes.c_void_p
_user32.GetMonitorInfoW.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_WM_NCLBUTTONDOWN = 0x00A1
_HT = {"left": 10, "right": 11, "top": 12, "topleft": 13, "topright": 14,
       "bottom": 15, "bottomleft": 16, "bottomright": 17}

TITLE = "ZenLink Beta"


class _MonitorInfo(ctypes.Structure):
    _fields_ = [("cbSize", wintypes.DWORD), ("rcMonitor", wintypes.RECT),
                ("rcWork", wintypes.RECT), ("dwFlags", wintypes.DWORD)]


class WindowApi:
    def __init__(self) -> None:
        self._window = None
        self._maximized = False
        self._fullscreen = False

    def bind(self, window) -> None:
        self._window = window
        self._normal_rect = None

    def _hwnd(self) -> int | None:
        return _user32.FindWindowW(None, TITLE) or None

    def start_resize(self, edge: str) -> None:
        hwnd = self._hwnd()
        if hwnd and edge in _HT and not self._maximized and not self._fullscreen:
            _user32.ReleaseCapture()
            _user32.SendMessageW(wintypes.HWND(hwnd), _WM_NCLBUTTONDOWN, _HT[edge], 0)

    def is_maximized(self) -> bool:
        return self._maximized

    def minimize(self) -> None:
        self._window.minimize()

    def toggle_maximize(self) -> bool:
        """Maximize to the monitor's work area so the taskbar stays visible.

        A frameless window maximized by Windows covers the whole monitor,
        taskbar included; filling the work area by hand avoids that."""
        hwnd = self._hwnd()
        if not hwnd:
            return self._maximized
        h = wintypes.HWND(hwnd)
        if self._maximized and self._normal_rect is not None:
            left, top, right, bottom = self._normal_rect
            _user32.SetWindowPos(h, None, left, top, right - left, bottom - top, 0x0004 | 0x0010)
            self._maximized = False
            return False
        rect = wintypes.RECT()
        _user32.GetWindowRect(h, ctypes.byref(rect))
        self._normal_rect = (rect.left, rect.top, rect.right, rect.bottom)
        info = _MonitorInfo()
        info.cbSize = ctypes.sizeof(_MonitorInfo)
        _user32.GetMonitorInfoW(_user32.MonitorFromWindow(h, 2), ctypes.byref(info))
        work = info.rcWork
        _user32.SetWindowPos(h, None, work.left, work.top, work.right - work.left, work.bottom - work.top,
                             0x0004 | 0x0010)
        self._maximized = True
        return True

    def set_fullscreen(self, on: bool) -> bool:
        if bool(on) != self._fullscreen:
            self._window.toggle_fullscreen()
            self._fullscreen = bool(on)
        return self._fullscreen

    def set_on_top(self, on: bool) -> None:
        self._window.on_top = bool(on)

    def fit_aspect(self, aspect: float, chrome_css_px: float) -> None:
        """Resize the window so the stage has the picture's aspect ratio:
        as large as fits in 85% of the monitor's work area, kept centred."""
        hwnd = self._hwnd()
        if not hwnd or self._maximized or self._fullscreen or not aspect or aspect <= 0:
            return
        try:
            scale = _user32.GetDpiForWindow(wintypes.HWND(hwnd)) / 96.0 or 1.0
        except Exception:
            scale = 1.0
        rect = wintypes.RECT()
        _user32.GetWindowRect(wintypes.HWND(hwnd), ctypes.byref(rect))
        info = _MonitorInfo()
        info.cbSize = ctypes.sizeof(_MonitorInfo)
        monitor = _user32.MonitorFromWindow(wintypes.HWND(hwnd), 2)  # MONITOR_DEFAULTTONEAREST
        _user32.GetMonitorInfoW(monitor, ctypes.byref(info))
        work = info.rcWork
        work_w, work_h = work.right - work.left, work.bottom - work.top
        chrome = int(max(0.0, float(chrome_css_px)) * scale)
        max_w, max_h = int(work_w * 0.85), int(work_h * 0.88)
        stage_h = max_h - chrome
        width = int(stage_h * aspect)
        if width > max_w:
            width = max_w
            stage_h = int(width / aspect)
        min_w, min_h = int(640 * scale), int(420 * scale)
        width = max(width, min_w)
        height = max(stage_h + chrome, min_h)
        cx, cy = (rect.left + rect.right) // 2, (rect.top + rect.bottom) // 2
        left = min(max(cx - width // 2, work.left), work.right - width)
        top = min(max(cy - height // 2, work.top), work.bottom - height)
        _user32.SetWindowPos(wintypes.HWND(hwnd), None, left, top, width, height, 0x0004 | 0x0010)  # NOZORDER|NOACTIVATE

    # ------------------------------------------------------------ tray

    visible = True
    close_to_tray = False   # set from Settings: closing hides to the tray
    tray_available = False

    def set_close_to_tray(self, on: bool) -> None:
        self.close_to_tray = bool(on)

    def hide_to_tray(self) -> None:
        from zenlink.tray import trim_memory

        self._window.hide()
        self.visible = False
        trim_memory()

    def show_from_tray(self) -> None:
        self._window.show()
        self._window.restore()
        self.visible = True
        try:  # bring it in front of other windows too
            import ctypes

            hwnd = self._window.native.Handle.ToInt32()
            ctypes.windll.user32.ShowWindow(hwnd, 9)  # SW_RESTORE
            ctypes.windll.user32.SetForegroundWindow(hwnd)
        except Exception:  # noqa: BLE001
            pass

    def quit(self) -> None:
        self._window.destroy()

    def close(self) -> None:
        if self.close_to_tray and self.tray_available:
            self.hide_to_tray()
        else:
            self._window.destroy()
