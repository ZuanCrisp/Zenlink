#![windows_subsystem = "windows"]
//! ZenLink Next: Rust shell (ADR-0001 Stage 1).

mod backdrop;
mod dots;
mod input;
mod prefs;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use slint::winit_030::winit::platform::windows::{BackdropType, WindowAttributesExtWindows};
use slint::winit_030::winit::window::WindowLevel;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, LogicalSize};
use windows::Win32::Foundation::HWND;

use zenlink_core::api::Api;
use zenlink_core::sidecar::Sidecar;
use zenlink_video::{Fit, PointerEvent, VideoView};

use input::{Cmd, Input};

slint::include_modules!();

static SIDECAR: OnceLock<Mutex<Option<Sidecar>>> = OnceLock::new();

/// Minimal file logger: %TEMP%\zenlink-beta.log (truncated on start).
struct FileLog(Mutex<std::fs::File>);

impl log::Log for FileLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record) {
        use std::io::Write;
        if self.enabled(r.metadata()) {
            let _ = writeln!(self.0.lock().unwrap(), "[{}] {}: {}", r.level(), r.target(), r.args());
        }
    }
    fn flush(&self) {}
}

fn init_log() {
    let path = std::env::temp_dir().join(zenlink_core::LOG_FILE_NAME);
    if let Ok(f) = std::fs::File::create(path) {
        let l: &'static FileLog = Box::leak(Box::new(FileLog(Mutex::new(f))));
        let _ = log::set_logger(l).map(|()| log::set_max_level(log::LevelFilter::Info));
    }
}

#[derive(Default)]
struct App {
    view: Option<VideoView>,
    streaming: bool,
    device_rotation: u32,
    /// (shown w, shown h, settings open) the window was last fitted to.
    fitted: Option<(u32, u32, bool)>,
    /// Logical window size ZenLink itself last set while fitting; any other
    /// size while streaming means the user resized the window.
    fit_set: Option<(f32, f32)>,
    /// Picture size the user chose by resizing, relative to the default fit
    /// (0 = never resized). Kept when the picture turns or settings change.
    user_scale: f32,
    /// Picture prefs the window was fitted for (fit, rotation, crop, follow):
    /// only these may resize the window, not volume or other settings.
    fit_prefs: Option<String>,
    meter_at: Option<Instant>,
    dark: bool,
    backdrop: Option<(bool, bool, i32, bool)>,
    on_top: Option<bool>,
    dots: Option<dots::Dots>,
    // Stage width (logical) to keep while the settings panel slides.
    hold_stage: Option<f32>,
    dots_at: Option<Instant>,
    /// A mode choice was just sent: keep the chooser closed until the
    /// backend's state catches up.
    mode_sent: Option<Instant>,
    /// A choice is waiting for the backend (it may still be starting).
    mode_pending: bool,
    /// Result of the last "Check drivers", shown under the service line.
    drivers_text: Option<String>,
    /// Device overlay ZenLink opened last ("control-center" / "notifications").
    panel: Option<&'static str>,
    /// v2-style connect card: when Start was pressed (the backend may still
    /// say "idle" for a moment).
    flow_started: Option<Instant>,
    /// USB instance id of the device, for driver changes.
    drv_instance: Option<String>,
    /// What the open notice sheet is for ("drv:restore", "restart", "info").
    sheet: Option<String>,
    /// Full screen: where the window was before (position, size, maximised).
    full_restore: Option<(slint::PhysicalPosition, slint::PhysicalSize, bool)>,
}

/// ZenLink's own full screen (the window covers the monitor). winit's full
/// screen left Windows' glass blur grey afterwards (light theme after dark)
/// until the app restarted.
fn is_full() -> bool {
    APP.with_borrow(|a| a.full_restore.is_some())
}

/// Physical bounds of the monitor the window is on.
fn monitor_rect(ui: &AppWindow) -> Option<(i32, i32, u32, u32)> {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    let h = hwnd_of(ui.window())?;
    unsafe {
        let m = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(m, &mut mi).as_bool() {
            return None;
        }
        let r = mi.rcMonitor;
        Some((r.left, r.top, (r.right - r.left) as u32, (r.bottom - r.top) as u32))
    }
}

thread_local! {
    static APP: RefCell<App> = RefCell::new(App::default());
    static START: Instant = Instant::now();
}

fn hwnd_of(w: &slint::Window) -> Option<HWND> {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    w.with_winit_window(|ww| match ww.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as _)),
        _ => None,
    })
    .flatten()
}

/// Windows "app mode" (Settings > Personalization > Colors).
fn system_dark() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut v = 1u32;
    let mut n = 4u32;
    unsafe {
        let _ = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut _ as _),
            Some(&mut n),
        );
    }
    v == 0
}

fn apply_look(ui: &AppWindow) {
    let p = ui.get_prefs();
    let dark = match p.theme.as_str() {
        "dark" => true,
        "system" => system_dark(),
        _ => false,
    };
    ui.global::<Theme>().set_dark(dark);
    // Thickness 0..100 -> clarity: 0% thick = 1.0 (clear), 100% = 0.25.
    let thick = (p.glass / 100.0).clamp(0.0, 1.0);
    ui.global::<Theme>().set_glass(if p.glass_on { 1.0 - 0.75 * thick } else { 0.0 });
    ui.global::<I18n>().set_id(p.lang.as_str() != "en");
    let glass_on = p.glass_on;
    let glass_k = thick;
    // Square corners in full screen / maximised (rounded ones show the desktop).
    let full = is_full();
    let round = !full && !ui.window().is_maximized();
    APP.with_borrow_mut(|a| {
        a.dark = dark;
        let glass_q = (glass_k * 20.0).round() as i32; // re-apply in 5% steps
        // Full screen paints a solid background, so no blur there. A dark
        // blur kept through full screen came back grey afterwards (also in
        // light theme) until the app restarted; turning it off avoids that.
        let blur = glass_on && !full;
        if a.backdrop != Some((blur, dark, glass_q, round)) {
            if let Some(h) = hwnd_of(ui.window()) {
                let mode = if blur { backdrop::Backdrop::Accent } else { backdrop::Backdrop::None };
                backdrop::apply(h, mode, dark, glass_k, round);
                a.backdrop = Some((blur, dark, glass_q, round));
            }
        }
        if a.on_top != Some(p.on_top) {
            let lvl = if p.on_top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal };
            ui.window().with_winit_window(|w| w.set_window_level(lvl));
            a.on_top = Some(p.on_top);
        }
    });
}

fn push_audio(api: &Api, p: &Prefs, muted: bool) {
    let api = api.clone();
    let body = json!({ "enabled": p.audio_out.as_str() == "pc", "volume": p.volume / 100.0, "muted": muted });
    std::thread::spawn(move || api.audio(&body));
}

fn str_of<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_str()
}

fn tr_ui(ui: &AppWindow, id: &str, en: &str) -> String {
    if ui.global::<I18n>().get_id() { id.into() } else { en.into() }
}

/// Backend messages are English; show the common ones in Indonesian.
fn tr_backend(ui: &AppWindow, en: &str) -> String {
    if !ui.global::<I18n>().get_id() {
        return en.into();
    }
    const MAP: &[(&str, &str)] = &[
        ("Unlock the device and tap Trust, then try again.", "Buka kunci perangkat dan tekan Percayai (Trust), lalu coba lagi."),
        ("Connect the device with the cable, unlock it and tap Trust first.", "Colokkan perangkat pakai kabel, buka kunci, lalu tekan Percayai (Trust) dulu."),
        ("Wi-Fi is already set up.", "Wi-Fi sudah siap. Bisa tanpa kabel."),
        ("Wi-Fi needs Apple Driver mode.", "Wi-Fi butuh mode Apple Driver."),
        ("Stop casting first.", "Hentikan mirroring dulu."),
        ("Choose a connection mode first.", "Pilih mode koneksi dulu."),
        ("Connect an iPhone or iPad with a USB cable.", "Colokkan iPhone atau iPad pakai kabel USB."),
        (
            "This device uses the ZenLink Studio driver, so Apple mode can't see it. Press Restore Apple driver in Settings > Connection & Drivers, or switch to ZenLink Studio.",
            "Perangkat ini memakai driver ZenLink Studio, jadi mode Apple tidak bisa melihatnya. Tekan Kembalikan driver Apple di Pengaturan > Koneksi & driver, atau ganti ke ZenLink Studio.",
        ),
        (
            "ZenLink Studio needs its driver on this device. Press Install ZenLink Studio driver in Settings > Connection & Drivers, or switch to Apple Driver mode.",
            "ZenLink Studio butuh drivernya di perangkat ini. Tekan Pasang driver ZenLink Studio di Pengaturan > Koneksi & driver, atau ganti ke mode Apple Driver.",
        ),
        (
            "A libusb0 filter is stacked on Apple's driver. Press Remove libusb0 filter in Settings > Connection & Drivers first.",
            "Filter libusb0 menumpuk di driver Apple. Tekan Hapus filter libusb0 di Pengaturan > Koneksi & driver dulu.",
        ),
        ("Stop casting before changing drivers.", "Hentikan mirroring dulu sebelum mengganti driver."),
        ("Another driver change is running.", "Penggantian driver lain sedang berjalan."),
        (
            "The device isn't on Wi-Fi yet. Press Set up Wi-Fi in Settings with the cable connected, keep both on the same network, or switch to Wired.",
            "Perangkat belum tersambung Wi-Fi. Tekan Atur Wi-Fi saat kabel tersambung, pastikan di jaringan yang sama, atau pilih Kabel.",
        ),
    ];
    if let Some((_, id)) = MAP.iter().find(|(e, _)| *e == en) {
        return (*id).into();
    }
    if let Some(rest) = en.strip_prefix("Wi-Fi setup failed: ") {
        return format!("Wi-Fi gagal disiapkan: {rest}");
    }
    en.into()
}

/// Backend state -> UI. Runs on the UI thread.
fn on_state(ui: &AppWindow, api: &Api, st: &Value) {
    let tr = |id: &str, en: &str| -> String { if ui.global::<I18n>().get_id() { id.into() } else { en.into() } };
    let phase = str_of(st, &["phase"]).unwrap_or("idle").to_string();
    // The backend starts the first ready device; show that one.
    let list = st.get("devices").and_then(Value::as_array);
    let first = list
        .and_then(|l| l.iter().find(|d| d.get("ready").and_then(Value::as_bool).unwrap_or(false)))
        .or_else(|| list.and_then(|l| l.first()));
    let device = str_of(st, &["device", "name"]).or_else(|| first.and_then(|d| str_of(d, &["name"]))).unwrap_or("");
    let wifi = first.and_then(|d| d.get("wifi")).and_then(Value::as_bool).unwrap_or(false);
    let backend = str_of(st, &["capture", "backend"]).or_else(|| str_of(st, &["connection", "active"])).unwrap_or("");
    let link = match backend {
        "usbdirect" => "ZenLink Studio".to_string(),
        "apple" | "hybrid" => format!("Apple · {}", if wifi { "Wi-Fi" } else { "USB" }),
        _ if wifi => "Wi-Fi".into(),
        _ if !device.is_empty() => "USB".into(),
        _ => String::new(),
    };
    let message = if phase == "error" { tr_backend(ui, str_of(st, &["error"]).unwrap_or("")) } else { String::new() };
    ui.set_phase(phase.clone().into());
    ui.set_device(device.into());
    ui.set_link(link.into());
    ui.set_message(message.into());
    let conn = match str_of(st, &["connection", "active"]) {
        Some("apple") => tr("Apple Driver (Standar)", "Apple Driver (Standard)"),
        Some("usbdirect") => "ZenLink Studio".into(),
        _ => tr("Belum dipilih", "Not chosen yet"),
    };
    ui.set_connection(conn.into());
    // Connection mode chooser: nothing connects until a mode is chosen.
    let c = st.get("connection");
    let needs = c.and_then(|c| c.get("needsChoice")).and_then(Value::as_bool).unwrap_or(false);
    ui.set_mode_active(str_of(st, &["connection", "active"]).unwrap_or("").into());
    ui.set_mode_link(str_of(st, &["connection", "link"]).unwrap_or("wired").into());
    ui.set_mode_remember(c.and_then(|c| c.get("remember")).and_then(Value::as_bool).unwrap_or(true) || needs);
    let just_sent = APP.with_borrow(|a| a.mode_pending || a.mode_sent.map(|t| t.elapsed() < Duration::from_secs(3)).unwrap_or(false));
    if needs && !just_sent && !ui.get_chooser_open() {
        ui.set_chooser_cancelable(false);
        open_chooser(ui);
    }
    // Opened at launch before the backend answered, but a remembered mode
    // is already active: nothing to choose.
    if !needs && !just_sent && ui.get_chooser_open() && !ui.get_chooser_cancelable() {
        ui.set_chooser_open(false);
    }
    if let Some(p) = str_of(st, &["connection", "pending"]) {
        let name = if p == "apple" { "Apple Driver" } else { "ZenLink Studio" };
        ui.set_connection(format!("{} · {}", ui.get_connection(), tr(&format!("{name} aktif setelah restart app"), &format!("{name} after an app restart"))).into());
    }
    let service = st.get("service").and_then(Value::as_bool).unwrap_or(true);
    let native_audio = st.get("nativeAudio").and_then(Value::as_bool).unwrap_or(false);
    let checked = APP.with_borrow(|a| a.drivers_text.clone().map(|t| format!("\n{t}")).unwrap_or_default());
    ui.set_driver_text(
        format!(
            "Apple Mobile Device Service: {}\n{}: {}{checked}",
            if service { tr("berjalan", "running") } else { tr("tidak berjalan", "not running") },
            tr("Audio native", "Native audio"),
            if native_audio { tr("tersedia", "available") } else { tr("tidak tersedia", "not available") },
        )
        .into(),
    );
    // Connect card (1-2-3-4) like v2. AirPlay / Hybrid wait for the device
    // to pick ZenLink in Screen Mirroring: no card then (the guide shows).
    let airplay = first.map(|d| d.get("airplay").and_then(Value::as_bool).unwrap_or(false) || d.get("hybrid").and_then(Value::as_bool).unwrap_or(false)).unwrap_or(false);
    let kind = if str_of(st, &["connection", "active"]) == Some("usbdirect") { "usbdirect" } else { "apple" };
    ui.set_flow_kind(kind.into());
    ui.set_device_wifi(wifi || str_of(st, &["device", "wifi"]).is_some() && st["device"]["wifi"].as_bool().unwrap_or(false));
    // What the device still needs before Start can work ("" = ready).
    let need = first
        .map(|d| match d.get("need").and_then(Value::as_str) {
            Some(n) => n.to_string(),
            None if !d.get("ready").and_then(Value::as_bool).unwrap_or(true) => "trust".into(),
            None => String::new(),
        })
        .unwrap_or_default();
    ui.set_device_need(need.into());
    // Driver buttons follow the device's driver (like v2).
    let drv = st.get("drivers");
    let dev = drv.and_then(|d| d.get("devices")).and_then(|d| d.get(0));
    let lib = drv.and_then(|d| d.pointer("/libusb0/available")).and_then(Value::as_bool).unwrap_or(false);
    let unsafe_filter = dev.and_then(|d| d.get("unsafeFilter")).and_then(Value::as_bool).unwrap_or(false);
    let dmode = dev.and_then(|d| str_of(d, &["mode"])).unwrap_or("");
    ui.set_drv_filter(dev.is_some() && unsafe_filter);
    ui.set_drv_restore(dev.is_some() && !unsafe_filter && (dmode == "replaced" || dmode == "unknown"));
    ui.set_drv_bind(dev.is_some() && !unsafe_filter && dmode == "apple" && lib);
    APP.with_borrow_mut(|a| a.drv_instance = dev.and_then(|d| str_of(d, &["instance"])).map(String::from));
    let fresh = APP.with_borrow(|a| a.flow_started.map(|t| t.elapsed() < Duration::from_secs(3)).unwrap_or(false));
    let step = ui.get_flow_step();
    match phase.as_str() {
        "connecting" | "activating" if airplay => ui.set_flow_step(-1),
        "connecting" | "activating" => {
            let n = if phase == "connecting" { 1 } else { 2 };
            ui.set_flow_failed(false);
            ui.set_flow_step(step.max(n));
        }
        "streaming" => {
            ui.set_flow_step(-1);
            ui.set_flow_failed(false);
            APP.with_borrow_mut(|a| a.flow_started = None);
        }
        "error" if step >= 0 => ui.set_flow_failed(true),
        _ if step >= 0 && !fresh && !ui.get_flow_failed() => ui.set_flow_step(-1),
        _ => {}
    }
    let rot = st.get("rotation").and_then(Value::as_u64).unwrap_or(0) as u32;
    let live = phase == "streaming";
    APP.with_borrow_mut(|a| {
        a.device_rotation = if [0, 90, 180, 270].contains(&rot) { rot } else { 0 };
        if live != a.streaming {
            a.streaming = live;
            a.fitted = None;
            if let Some(v) = a.view.as_mut() {
                if live {
                    v.start(api.clone());
                } else {
                    v.stop();
                }
            }
            if live {
                push_audio(api, &ui.get_prefs(), ui.get_muted());
            }
        }
    });
}

fn effective_rotation(p: &Prefs, shown: (u32, u32), device_rotation: u32) -> u32 {
    match p.rotation.as_str() {
        "auto" => {
            if shown.0 > shown.1 {
                0
            } else {
                device_rotation
            }
        }
        r => r.parse().unwrap_or(0),
    }
}

/// Logical work area of the monitor the window is on.
fn work_area(ui: &AppWindow) -> Option<(f32, f32, f32, f32)> {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    let h = hwnd_of(ui.window())?;
    let sf = ui.window().scale_factor();
    unsafe {
        let m = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(m, &mut mi).as_bool() {
            return None;
        }
        let r = mi.rcWork;
        Some((r.left as f32 / sf, r.top as f32 / sf, (r.right - r.left) as f32 / sf, (r.bottom - r.top) as f32 / sf))
    }
}

/// Size the window around the picture (portrait or landscape).
/// Gap between the picture and the glass stage edge (logical px).
const PIC_INSET: f32 = 6.0;
/// Narrowest window that still fits the header (logo, name, buttons,
/// window controls).
const MIN_W: f32 = 470.0;
/// Stage width (logical px) while there is no picture yet.
const IDLE_STAGE_W: f32 = 440.0;

/// Picture prefs that change the window's fitted size.
fn fit_prefs_key(p: &Prefs) -> String {
    format!("{}|{}|{}|{}|{}|{}|{}", p.fit, p.rotation, p.fit_window, p.crop_l, p.crop_t, p.crop_r, p.crop_b)
}

/// Default fitted picture size (logical px, without chrome) for `shown`.
fn default_fit(ui: &AppWindow, shown: (u32, u32)) -> Option<(f32, f32)> {
    let (_, _, ww, wh) = work_area(ui)?;
    let chrome_w = ui.get_chrome_w() + 2.0 * PIC_INSET;
    let chrome_h = ui.get_chrome_h() + 2.0 * PIC_INSET;
    let aspect = shown.0 as f32 / shown.1.max(1) as f32;
    let max_h = wh * 0.62 - chrome_h;
    let max_w = ww * 0.46 - chrome_w;
    let (mut sw, mut sh) = (max_h * aspect, max_h);
    if sw > max_w {
        sw = max_w;
        sh = sw / aspect;
    }
    Some((sw.max(1.0), sh.max(1.0)))
}

/// Fit the window to the picture. `scale` > 0: the size the user chose by
/// resizing, relative to the default fit (kept within the screen).
fn fit_window(ui: &AppWindow, shown: (u32, u32), scale: f32) -> Option<(f32, f32)> {
    let (wx, wy, ww, wh) = work_area(ui)?;
    let chrome_w = ui.get_chrome_w() + 2.0 * PIC_INSET;
    let chrome_h = ui.get_chrome_h() + 2.0 * PIC_INSET;
    let (mut sw, mut sh) = default_fit(ui, shown)?;
    if scale > 0.0 {
        let k = scale.min((ww - chrome_w) / sw).min((wh - chrome_h) / sh).max(0.2);
        sw *= k;
        sh *= k;
    }
    let size = LogicalSize::new((sw + chrome_w).max(MIN_W), (sh + chrome_h).max(440.0));
    ui.window().set_size(size);
    keep_on_screen(ui, (wx, wy, ww, wh), size);
    Some((size.width, size.height))
}

/// Show the connection mode chooser, widening the window so the sheet fits.
fn open_chooser(ui: &AppWindow) {
    ui.set_chooser_open(true);
    let Some((wx, wy, ww, wh)) = work_area(ui) else { return };
    let cur = ui.window().size().to_logical(ui.window().scale_factor());
    // Never narrower than the idle stage plus the open settings panel (the
    // window may not have taken that size yet when this runs at launch).
    let want = if ui.get_settings_open() { IDLE_STAGE_W + 16.0 + 368.0 } else { 600.0 };
    let w = cur.width.max(want.max(600.0).min(ww - 16.0));
    let h = cur.height.max(820.0f32.min(wh - 16.0));
    if (w, h) != (cur.width, cur.height) {
        let size = LogicalSize::new(w, h);
        ui.window().set_size(size);
        keep_on_screen(ui, (wx, wy, ww, wh), size);
    }
}

/// True when the backend's settings.json remembers a connection mode
/// (source checkout: next to zenlink/; installed: %LOCALAPPDATA%\ZenLink-Beta).
fn mode_remembered() -> bool {
    let mut files = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1) {
            if dir.join("zenlink").join("connection.py").exists() || dir.join("portable.txt").exists() {
                files.push(dir.join("settings.json"));
                break;
            }
        }
    }
    // Installed and portable builds keep settings in %LOCALAPPDATA%\ZenLink-Beta;
    // a source checkout or portable folder only uses its own file.
    if files.is_empty() {
        if let Some(d) = zenlink_core::local_data_dir() {
            files.push(d.join("settings.json"));
        }
    }
    files.iter().filter_map(|f| std::fs::read(f).ok()).filter_map(|b| serde_json::from_slice::<Value>(&b).ok()).any(|v| {
        v.get("rememberConnectionMode").and_then(Value::as_bool).unwrap_or(false)
            && matches!(v.get("connectionMode").and_then(Value::as_str), Some("apple" | "usbdirect"))
    })
}

/// Short message above the dock for 3.5 s.
fn show_toast(ui: &AppWindow, text: String) {
    ui.set_toast("".into());
    ui.set_toast(text.into());
    let w = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(3500), move || {
        if let Some(ui) = w.upgrade() {
            ui.set_toast("".into());
        }
    });
}

/// "Check drivers" result, one line per device.
fn drivers_summary(ui: &AppWindow, d: &Value) -> String {
    let id = ui.global::<I18n>().get_id();
    let mut lines = Vec::new();
    for dev in d.get("devices").and_then(Value::as_array).into_iter().flatten() {
        let fam = dev.get("family").and_then(Value::as_str).unwrap_or("iPhone");
        let mode = match dev.get("mode").and_then(Value::as_str).unwrap_or("") {
            "apple" => "Apple Driver",
            "replaced" => "ZenLink Studio (libusb0)",
            "coexist" => if id { "campuran, perlu diperbaiki" } else { "mixed, needs repair" },
            _ => if id { "tidak dikenal" } else { "unknown" },
        };
        lines.push(format!("{fam}: {mode}"));
    }
    if lines.is_empty() {
        lines.push(if id { "Tidak ada iPhone/iPad di USB".into() } else { "No iPhone/iPad on USB".into() });
    }
    lines.join("\n")
}

fn open_sheet(ui: &AppWindow, kind: &str, title: String, body: String, ok: String, cancel: String) {
    APP.with_borrow_mut(|a| a.sheet = Some(kind.into()));
    ui.set_sheet_title(title.into());
    ui.set_sheet_body(body.into());
    ui.set_sheet_ok(ok.into());
    ui.set_sheet_cancel(cancel.into());
    ui.set_sheet_open(true);
}

/// Ask before a driver change (it needs administrator approval).
fn confirm_driver(ui: &AppWindow, action: &str) {
    if matches!(ui.get_phase().as_str(), "connecting" | "activating" | "streaming") {
        show_toast(ui, tr_ui(ui, "Hentikan mirroring dulu sebelum mengganti driver.", "Stop casting before changing drivers."));
        return;
    }
    if APP.with_borrow(|a| a.drv_instance.is_none()) {
        show_toast(ui, tr_ui(ui, "Colokkan perangkat dulu.", "Connect the device first."));
        return;
    }
    let (title, body) = match action {
        "restore" => (
            tr_ui(ui, "Kembalikan driver Apple?", "Restore the Apple driver?"),
            tr_ui(ui,
                "Mode Apple Driver bisa melihat perangkat lagi. ZenLink Studio berhenti sampai drivernya dipasang lagi. Cadangan disimpan dulu. Windows akan minta izin administrator. Cabut lalu colok lagi kabelnya setelah selesai.",
                "Apple Driver mode can see the device again. ZenLink Studio stops until its driver is installed again. A backup is saved first. Windows will ask for administrator permission. Unplug and reconnect the cable afterwards."),
        ),
        "bind" => (
            tr_ui(ui, "Pasang driver ZenLink Studio?", "Install the ZenLink Studio driver?"),
            tr_ui(ui,
                "Seperti TikTok LIVE Studio, ZenLink memasang driver libusb-win32 untuk perangkat ini. Selama terpasang, mode Apple dan aplikasi Apple tidak melihat perangkat; tekan Kembalikan driver Apple untuk balik. Kalau belum ada paket driver untuk model ini, ZenLink membuatnya dan menandatanganinya dengan sertifikat baru bernama \"ZenLink ZenLink Studio\" yang ditambahkan ke sertifikat tepercaya Windows (kunci privatnya dihapus). Cadangan disimpan dulu, dan driver Apple dipasang lagi otomatis kalau perangkat gagal jalan. Windows akan minta izin administrator. Cabut lalu colok lagi kabelnya setelah selesai.",
                "Like TikTok LIVE Studio, ZenLink installs the libusb-win32 driver for this device. While it is installed, Apple mode and Apple apps can't see the device; press Restore Apple driver to switch back. If there is no driver package for this model yet, ZenLink builds one and signs it with a new certificate named \"ZenLink ZenLink Studio\", added to Windows' trusted certificates (its private key is deleted). A backup is saved first, and Apple's driver comes back automatically if the device doesn't start. Windows will ask for administrator permission. Unplug and reconnect the cable afterwards."),
        ),
        _ => (
            tr_ui(ui, "Hapus filter libusb0?", "Remove the libusb0 filter?"),
            tr_ui(ui,
                "Filter ini menumpuk di driver Apple dan bisa membuat Windows crash. Driver Apple tetap ada. Cadangan disimpan dulu. Windows akan minta izin administrator.",
                "It is stacked on Apple's driver, which can crash Windows. Apple's driver stays. A backup is saved first. Windows will ask for administrator permission."),
        ),
    };
    open_sheet(ui, &format!("drv:{action}"), title, body, tr_ui(ui, "Lanjut", "Continue"), tr_ui(ui, "Batal", "Cancel"));
}

/// Run a confirmed driver change, then offer to restart the service.
fn run_driver(ui: &AppWindow, api: &Api, action: String) {
    let Some(instance) = APP.with_borrow(|a| a.drv_instance.clone()) else { return };
    open_sheet(
        ui,
        "busy",
        tr_ui(ui, "Mengganti driver perangkat…", "Changing the device's driver…"),
        tr_ui(ui, "Setujui izin Windows. Jendela PowerShell menampilkan tiap langkah; biarkan kabel tetap tercolok sampai selesai.",
            "Approve the Windows permission prompt. A PowerShell window shows each step; keep the cable plugged in until it finishes."),
        String::new(),
        String::new(),
    );
    let remember = ui.get_mode_remember();
    let (api, w) = (api.clone(), ui.as_weak());
    std::thread::spawn(move || {
        let r = api.post_slow("/api/drivers/action", &json!({ "action": action, "instance": instance, "confirmed": true }), 900);
        let ok = r.as_ref().and_then(|r| r.get("ok")).and_then(Value::as_bool).unwrap_or(false);
        if ok && action != "removefilter" {
            // Keep the connection mode in step with the driver now installed.
            let mode = if action == "bind" { "usbdirect" } else { "apple" };
            api.post("/api/connection", &json!({ "mode": mode, "remember": remember }));
        }
        let msg = r.as_ref().and_then(|r| str_of(r, &["message"])).unwrap_or("").to_string();
        let rolled = r.as_ref().and_then(|r| r.get("rolledBack")).and_then(Value::as_bool).unwrap_or(false);
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = w.upgrade() else { return };
            if ok {
                let target = if action == "bind" { "ZenLink Studio" } else { "Apple Driver" };
                open_sheet(
                    &ui,
                    "restart",
                    tr_ui(&ui, "Driver sudah diganti", "Driver changed"),
                    format!("{msg}

{}", tr_ui(&ui,
                        &format!("Layanan ZenLink perlu dimulai ulang untuk memakai mode {target}. Cabut lalu colok lagi kabelnya."),
                        &format!("The ZenLink service needs a restart to use {target} mode. Unplug the cable and plug it in again."))),
                    tr_ui(&ui, "Mulai ulang sekarang", "Restart now"),
                    tr_ui(&ui, "Nanti", "Later"),
                );
            } else {
                let body = if msg.is_empty() { tr_ui(&ui, "Backend tidak menjawab.", "The backend did not answer.") } else { tr_backend(&ui, &msg) };
                let body = if rolled { format!("{body} {}", tr_ui(&ui, "Kondisi sebelumnya sudah dipulihkan.", "The previous state was restored.")) } else { body };
                open_sheet(&ui, "info", tr_ui(&ui, "Driver tidak diubah", "Driver not changed"), body, "OK".into(), String::new());
            }
        });
    });
}

/// Move the window back inside the work area if a resize pushed it out.
fn keep_on_screen(ui: &AppWindow, (wx, wy, ww, wh): (f32, f32, f32, f32), size: LogicalSize) {
    let sf = ui.window().scale_factor();
    let pos = ui.window().position().to_logical(sf);
    let (w, h) = (size.width, size.height); // frameless: client = window
    let x = pos.x.min(wx + ww - w).max(wx);
    let y = pos.y.min(wy + wh - h).max(wy);
    if (x - pos.x).abs() > 0.5 || (y - pos.y).abs() > 0.5 {
        ui.window().set_position(slint::LogicalPosition::new(x, y));
    }
}

fn main() -> Result<(), slint::PlatformError> {
    init_log();
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_window_attributes_hook(|attrs| {
            attrs.with_transparent(true).with_decorations(false).with_system_backdrop(BackdropType::None)
        })
        .select()?;

    let ui = AppWindow::new()?;
    ui.set_prefs(prefs::load());
    let api = Api::new();
    let input = Input::new(api.clone());
    let touch_on = Arc::new(AtomicBool::new(ui.get_prefs().touch));
    let gestures_on = Arc::new(AtomicBool::new(ui.get_prefs().gestures));
    ui.set_message("Menyiapkan ZenLink…".into());
    if std::env::var_os("ZL_OPEN_SETTINGS").is_some() {
        ui.set_settings_open(true);
    }

    // Backend (Python sidecar for now), then a state poll loop.
    {
        let weak = ui.as_weak();
        let api = api.clone();
        std::thread::spawn(move || {
            match Sidecar::ensure(&api, Duration::from_secs(40)) {
                Ok(s) => {
                    let _ = SIDECAR.set(Mutex::new(Some(s)));
                }
                Err(e) => {
                    let w = weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = w.upgrade() {
                            ui.set_phase("error".into());
                            ui.set_message(format!("Backend: {e}").into());
                        }
                    });
                    return;
                }
            }
            loop {
                if let Some(st) = api.state() {
                    let w = weak.clone();
                    let a = api.clone();
                    if slint::invoke_from_event_loop(move || {
                        if let Some(ui) = w.upgrade() {
                            on_state(&ui, &a, &st);
                        }
                    })
                    .is_err()
                    {
                        return;
                    }
                }
                std::thread::sleep(Duration::from_millis(400));
            }
        });
    }

    // Callbacks.
    {
        let api = api.clone();
        let weak = ui.as_weak();
        ui.on_start_cast(move || {
            if let Some(ui) = weak.upgrade() {
                // The card opens at step 0 (detect device) at once.
                let airplay = ui.get_mode_link().as_str() == "wireless" && ui.get_flow_kind().as_str() == "usbdirect";
                if !airplay {
                    ui.set_flow_failed(false);
                    ui.set_flow_step(0);
                    APP.with_borrow_mut(|a| a.flow_started = Some(Instant::now()));
                }
            }
            let a = api.clone();
            std::thread::spawn(move || a.start_cast());
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_flow_dismiss(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_flow_step(-1);
                ui.set_flow_failed(false);
            }
        });
    }
    {
        // The ready view's button when the device needs something first.
        let weak = ui.as_weak();
        ui.on_need_fix(move || {
            let Some(ui) = weak.upgrade() else { return };
            match ui.get_device_need().as_str() {
                n @ ("restore" | "bind" | "removefilter") => confirm_driver(&ui, n),
                "mode" => {
                    ui.set_chooser_cancelable(true);
                    open_chooser(&ui);
                }
                _ => {
                    ui.set_settings_open(true);
                    ui.invoke_settings_toggled(true);
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let api = api.clone();
        ui.on_sheet_answer(move |yes| {
            let Some(ui) = weak.upgrade() else { return };
            let kind = APP.with_borrow_mut(|a| a.sheet.take()).unwrap_or_default();
            if kind == "busy" {
                APP.with_borrow_mut(|a| a.sheet = Some(kind));
                return; // no buttons while the change runs
            }
            ui.set_sheet_open(false);
            if !yes {
                return;
            }
            if let Some(action) = kind.strip_prefix("drv:") {
                run_driver(&ui, &api, action.to_string());
            } else if kind == "restart" {
                show_toast(&ui, tr_ui(&ui, "Memulai ulang layanan ZenLink…", "Restarting the ZenLink service…"));
                let api = api.clone();
                std::thread::spawn(move || api.post_slow("/api/app/restart", &json!({}), 20));
            }
        });
    }
    {
        let api = api.clone();
        ui.on_stop_cast(move || {
            let a = api.clone();
            std::thread::spawn(move || a.stop_cast());
        });
    }
    {
        let api = api.clone();
        let weak = ui.as_weak();
        ui.on_restart(move || {
            if let Some(ui) = weak.upgrade() {
                show_toast(&ui, tr_ui(&ui, "Memulai ulang koneksi…", "Restarting the connection…"));
            }
            let a = api.clone();
            std::thread::spawn(move || {
                a.stop_cast();
                // The backend ignores Start while the old session is still
                // closing (that can take a few seconds): wait until it is
                // gone ("capture" is null), then start again.
                let t0 = Instant::now();
                while t0.elapsed() < Duration::from_secs(15) {
                    match a.state() {
                        Some(st) if st.get("capture").map_or(true, Value::is_null) => break,
                        _ => std::thread::sleep(Duration::from_millis(250)),
                    }
                }
                std::thread::sleep(Duration::from_millis(300));
                a.start_cast();
            });
        });
    }
    {
        let weak = ui.as_weak();
        let api = api.clone();
        let (t, g) = (touch_on.clone(), gestures_on.clone());
        ui.on_prefs_changed(move |p| {
            prefs::save(&p);
            t.store(p.touch, Relaxed);
            g.store(p.gestures, Relaxed);
            if let Some(ui) = weak.upgrade() {
                apply_look(&ui);
                push_audio(&api, &p, ui.get_muted());
                // Only picture settings refit the window. Any other setting
                // (volume, lip sync, theme...) used to shrink a window the
                // user had resized back to the default fit.
                let key = fit_prefs_key(&p);
                APP.with_borrow_mut(|a| {
                    if a.fit_prefs.as_deref() != Some(key.as_str()) {
                        a.fit_prefs = Some(key);
                        a.fitted = None;
                    }
                });
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_settings_toggled(move |open| {
            let Some(ui) = weak.upgrade() else { return };
            // The window grows/shrinks frame by frame with the panel slide
            // (see the frame timer), keeping the picture size steady.
            let sf = ui.window().scale_factor();
            let stage = ui.window().size().to_logical(sf).width - ui.get_chrome_w();
            APP.with_borrow_mut(|a| {
                a.hold_stage = Some(stage);
                if let Some((w, h, _)) = a.fitted {
                    a.fitted = Some((w, h, open));
                }
            });
        });
    }
    if ui.get_settings_open() {
        // Settings start open: the window is the usual idle stage plus the
        // panel, not the panel squeezed into the default window (the window
        // has no size yet here, so do not measure it).
        APP.with_borrow_mut(|a| a.hold_stage = Some(IDLE_STAGE_W));
    }
    // Show the chooser at once unless a remembered mode exists; the
    // backend can take a while to start.
    if !mode_remembered() {
        ui.set_chooser_cancelable(false);
        ui.set_chooser_open(true);
        // Widen once the window exists on screen.
        let w = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(60), move || {
            if let Some(ui) = w.upgrade() {
                if ui.get_chooser_open() {
                    open_chooser(&ui);
                }
            }
        });
    }
    // Test hook: ZL_DEMO_SETTINGS=1 opens/closes settings every 1.5 s so the
    // slide can be filmed frame by frame.
    let demo = slint::Timer::default();
    if std::env::var_os("ZL_DEMO_SETTINGS").is_some() {
        let weak = ui.as_weak();
        demo.start(slint::TimerMode::Repeated, Duration::from_millis(1500), move || {
            if let Some(ui) = weak.upgrade() {
                let open = !ui.get_settings_open();
                ui.set_settings_open(open);
                ui.invoke_settings_toggled(open);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let api = api.clone();
        let input = input.clone();
        ui.on_action(move |a| {
            let Some(ui) = weak.upgrade() else { return };
            // Control Center / Notification Center: press again to close,
            // press the other one to switch (close first, then open).
            if let Some(name) = match a.as_str() {
                "control-center" => Some("control-center"),
                "notifications" => Some("notifications"),
                _ => None,
            } {
                let open = |input: &Input, n: &str| {
                    if n == "control-center" {
                        input.send(Cmd::Stroke(0.92, 0.0, 0.92, 0.62, 12, 16));
                    } else {
                        input.send(Cmd::Stroke(0.22, 0.0, 0.22, 0.62, 12, 16));
                    }
                };
                // Swipe up from the bottom closes either overlay.
                let close = |input: &Input| input.send(Cmd::Stroke(0.5, 0.97, 0.5, 0.25, 12, 16));
                let was = APP.with_borrow(|x| x.panel);
                match was {
                    Some(p) if p == name => {
                        close(&input);
                        APP.with_borrow_mut(|x| x.panel = None);
                    }
                    Some(_) => {
                        close(&input);
                        APP.with_borrow_mut(|x| x.panel = Some(name));
                        let input = input.clone();
                        slint::Timer::single_shot(Duration::from_millis(550), move || open(&input, name));
                    }
                    None => {
                        open(&input, name);
                        APP.with_borrow_mut(|x| x.panel = Some(name));
                    }
                }
                return;
            }
            if matches!(a.as_str(), "swipe-left" | "swipe-right" | "home" | "switcher" | "lock") {
                APP.with_borrow_mut(|x| x.panel = None);
            }
            match a.as_str() {
                "swipe-left" => input.send(Cmd::Stroke(1.0, 0.5, 0.12, 0.5, 10, 20)),
                "swipe-right" => input.send(Cmd::Stroke(0.0, 0.5, 0.88, 0.5, 10, 20)),
                "home" => input.send(Cmd::Home(1)),
                "switcher" => input.send(Cmd::Home(2)),
                "control-center" => input.send(Cmd::Stroke(0.92, 0.0, 0.92, 0.62, 12, 16)),
                "notifications" => input.send(Cmd::Stroke(0.22, 0.0, 0.22, 0.62, 12, 16)),
                "lock" => input.send(Cmd::Action("lock")),
                "mute" => {
                    let m = !ui.get_muted();
                    ui.set_muted(m);
                    push_audio(&api, &ui.get_prefs(), m);
                }
                "rotate" => {
                    let mut p = ui.get_prefs();
                    p.rotation = match p.rotation.as_str() {
                        "auto" => "90",
                        "90" => "180",
                        "180" => "270",
                        "270" => "0",
                        _ => "auto",
                    }
                    .into();
                    ui.set_prefs(p.clone());
                    prefs::save(&p);
                    APP.with_borrow_mut(|a| a.fitted = None);
                }
                _ => {}
            }
        });
    }
    {
        let api = api.clone();
        let weak = ui.as_weak();
        ui.on_choose_mode(move |mode, link, remember| {
            APP.with_borrow_mut(|a| a.mode_pending = true);
            let (api, w) = (api.clone(), weak.clone());
            let (mode, link) = (mode.to_string(), link.to_string());
            std::thread::spawn(move || {
                // The backend may still be starting (the chooser shows at
                // launch): wait for it, up to a minute.
                let t0 = Instant::now();
                let mut cur = api.get("/api/connection");
                while cur.is_none() && t0.elapsed() < Duration::from_secs(60) {
                    std::thread::sleep(Duration::from_millis(500));
                    cur = api.get("/api/connection");
                }
                let ok_link = cur.is_some()
                    && (cur.as_ref().and_then(|c| c.get("link")).and_then(Value::as_str) == Some(link.as_str())
                        || api.post("/api/connection", &json!({ "link": link })).is_some());
                // A refused choice answers HTTP 400 (post() gives None). The
                // reply is the whole app state, whose "error" field is
                // usually null: not a failure.
                let failed = !ok_link || api.post("/api/connection", &json!({ "mode": mode, "remember": remember })).is_none();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = w.upgrade() else { return };
                    APP.with_borrow_mut(|a| {
                        a.mode_pending = false;
                        a.mode_sent = (!failed).then(Instant::now);
                    });
                    if failed {
                        show_toast(&ui, tr_ui(&ui, "Mode belum tersimpan, coba lagi.", "The mode was not saved, try again."));
                        open_chooser(&ui);
                    }
                });
            });
        });
    }
    {
        // Cable / Wi-Fi from Settings (both modes). The backend ends a
        // running session first.
        let api = api.clone();
        let weak = ui.as_weak();
        ui.on_set_link(move |link| {
            let Some(ui) = weak.upgrade() else { return };
            if ui.get_mode_link() == link {
                return;
            }
            ui.set_mode_link(link.clone());
            show_toast(&ui, if link.as_str() == "wireless" { tr_ui(&ui, "Beralih ke Wi-Fi…", "Switching to Wi-Fi…") } else { tr_ui(&ui, "Beralih ke kabel…", "Switching to the cable…") });
            let (api, w) = (api.clone(), weak.clone());
            let link = link.to_string();
            std::thread::spawn(move || {
                let ok = api.post("/api/connection", &json!({ "link": link })).is_some();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = w.upgrade() {
                        if !ok {
                            show_toast(&ui, tr_ui(&ui, "Sambungan belum bisa diganti, coba lagi.", "The connection could not be changed, try again."));
                        }
                    }
                });
            });
        });
    }
    {
        let api = api.clone();
        let weak = ui.as_weak();
        ui.on_driver_action(move |a: slint::SharedString| {
            if matches!(a.as_str(), "restore" | "bind" | "removefilter") {
                if let Some(ui) = weak.upgrade() {
                    confirm_driver(&ui, a.as_str());
                }
                return;
            }
            if a.as_str() == "mode" {
                if let Some(ui) = weak.upgrade() {
                    ui.set_chooser_cancelable(true);
                    open_chooser(&ui);
                }
                return;
            }
            if let Some(ui) = weak.upgrade() {
                show_toast(&ui, if a.as_str() == "wifi" { tr_ui(&ui, "Menyiapkan Wi-Fi…", "Setting up Wi-Fi…") } else { tr_ui(&ui, "Memeriksa driver…", "Checking drivers…") });
            }
            let (api, w) = (api.clone(), weak.clone());
            let a = a.to_string();
            std::thread::spawn(move || {
                let r = if a == "wifi" { api.post_slow("/api/wifi/prepare", &json!({}), 120) } else { api.get("/api/drivers") };
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = w.upgrade() else { return };
                    let Some(r) = r else {
                        show_toast(&ui, tr_ui(&ui, "Backend tidak menjawab.", "The backend did not answer."));
                        return;
                    };
                    if a == "wifi" {
                        let ok = r.get("ok").and_then(Value::as_bool).unwrap_or(false);
                        let msg = r.get("message").and_then(Value::as_str).map(|m| tr_backend(&ui, m)).unwrap_or_else(|| {
                            if ok { tr_ui(&ui, "Wi-Fi siap. Sekarang bisa tanpa kabel.", "Wi-Fi is ready. You can unplug the cable.") }
                            else { tr_ui(&ui, "Wi-Fi belum bisa disiapkan.", "Wi-Fi could not be set up.") }
                        });
                        show_toast(&ui, msg);
                    } else {
                        let t = drivers_summary(&ui, &r);
                        APP.with_borrow_mut(|x| x.drivers_text = Some(t));
                        show_toast(&ui, tr_ui(&ui, "Driver sudah diperiksa.", "Drivers checked."));
                    }
                });
            });
        });
    }

    // Frameless window: move, resize, minimise, maximise, close.
    {
        let weak = ui.as_weak();
        let last_drag = std::cell::Cell::new(None::<Instant>);
        ui.on_window_action(move |a| {
            use slint::winit_030::winit::window::ResizeDirection as R;
            let Some(ui) = weak.upgrade() else { return };
            let w = ui.window();
            match a.as_str() {
                "close" => {
                    let _ = w.hide();
                    let _ = slint::quit_event_loop();
                }
                "min" => w.set_minimized(true),
                "max" if is_full() => {}
                "max" => w.set_maximized(!w.is_maximized()),
                "fullscreen" => {
                    if !is_full() {
                        let was_max = w.is_maximized();
                        if was_max {
                            w.set_maximized(false);
                        }
                        if let Some((x, y, mw, mh)) = monitor_rect(&ui) {
                            let (pos, size) = (w.position(), w.size());
                            APP.with_borrow_mut(|a| a.full_restore = Some((pos, size, was_max)));
                            w.set_position(slint::PhysicalPosition::new(x, y));
                            // One pixel taller than the monitor: a GL window that
                            // exactly covers it is switched by the driver to an
                            // opaque full-screen mode that stayed after leaving
                            // (no glass, grey window, until a restart).
                            w.set_size(slint::PhysicalSize::new(mw, mh + 1));
                        }
                    } else if let Some((pos, size, was_max)) = APP.with_borrow_mut(|a| a.full_restore.take()) {
                        w.set_position(pos);
                        w.set_size(size);
                        if was_max {
                            w.set_maximized(true);
                        }
                    }
                    ui.set_fullscreen(is_full());
                    // Leaving full screen: fit the window to the picture again.
                    // Corners and glass follow the new state: apply the theme
                    // again now and once the change has settled.
                    APP.with_borrow_mut(|a| {
                        a.fitted = None;
                        a.backdrop = None;
                    });
                    let w2 = ui.as_weak();
                    slint::Timer::single_shot(Duration::from_millis(350), move || {
                        if w2.upgrade().is_some() {
                            APP.with_borrow_mut(|a| a.backdrop = None);
                        }
                    });
                }
                "drag" if is_full() => {}
                "drag" => {
                    // Second press within 400 ms = double-click: maximise / restore.
                    let now = Instant::now();
                    if last_drag.get().map(|t| now - t < Duration::from_millis(400)).unwrap_or(false) {
                        last_drag.set(None);
                        w.set_maximized(!w.is_maximized());
                    } else {
                        last_drag.set(Some(now));
                        w.with_winit_window(|ww| ww.drag_window());
                    }
                }
                r if r.starts_with("resize-") => {
                    let dir = match &r[7..] {
                        "n" => R::North,
                        "s" => R::South,
                        "w" => R::West,
                        "e" => R::East,
                        "nw" => R::NorthWest,
                        "ne" => R::NorthEast,
                        "sw" => R::SouthWest,
                        _ => R::SouthEast,
                    };
                    w.with_winit_window(|ww| ww.drag_resize_window(dir));
                }
                _ => {}
            }
        });
    }

    // Frame timer: native view placement, window fitting, stats.
    let timer = slint::Timer::default();
    {
        let weak = ui.as_weak();
        let input = input.clone();
        let (t, g) = (touch_on.clone(), gestures_on.clone());
        timer.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || {
            let Some(ui) = weak.upgrade() else { return };
            APP.with_borrow_mut(|a| {
                if a.view.is_none() {
                    let Some(h) = hwnd_of(ui.window()) else { return };
                    a.backdrop = None;
                    match VideoView::new(h.0 as isize) {
                        Ok(v) => {
                            let input = input.clone();
                            let (t, g) = (t.clone(), g.clone());
                            v.on_pointer(move |e| match e {
                                PointerEvent::Down(u, v) if t.load(Relaxed) => input.send(Cmd::Down(u, v)),
                                PointerEvent::Move(u, v) if t.load(Relaxed) => input.send(Cmd::Move(u, v)),
                                PointerEvent::Up(u, v) if t.load(Relaxed) => input.send(Cmd::Up(u, v)),
                                PointerEvent::Back if g.load(Relaxed) => input.send(Cmd::Stroke(0.0, 0.5, 0.65, 0.5, 14, 16)),
                                PointerEvent::Wheel(u, v, d) if g.load(Relaxed) => {
                                    let end = (v + 0.12 * d).clamp(0.02, 0.98);
                                    input.send(Cmd::Stroke(u, v, u, end, 5, 10));
                                }
                                _ => {}
                            });
                            a.view = Some(v);
                            if a.streaming {
                                a.view.as_mut().unwrap().start(Api::new());
                            }
                        }
                        Err(e) => {
                            ui.set_message(format!("Video: {e}").into());
                            return;
                        }
                    }
                }
            });
            if APP.with_borrow(|a| a.backdrop.is_none()) {
                apply_look(&ui);
            }
            let p = ui.get_prefs();
            let sf = ui.window().scale_factor();
            let lw = ui.window().size().width as f32 / sf;
            if (lw < 640.0) != ui.get_compact() {
                ui.set_compact(lw < 640.0);
            }
            let (maxed, full) = (ui.window().is_maximized(), is_full());
            if maxed != ui.get_maximized() || full != ui.get_fullscreen() {
                // Window style changed: corners and glass go again.
                APP.with_borrow_mut(|a| a.backdrop = None);
            }
            ui.set_maximized(maxed);
            ui.set_fullscreen(full);
            // Settings slide: window width follows the animated chrome so
            // the picture keeps its size; ends when the slide settles.
            if let Some(stage) = APP.with_borrow(|a| a.hold_stage) {
                let chrome = ui.get_chrome_w();
                let target = (stage + chrome).max(MIN_W);
                let lh = ui.window().size().height as f32 / sf;
                if (lw - target).abs() > 0.5 && !ui.window().is_maximized() && !is_full() {
                    let size = LogicalSize::new(target, lh);
                    ui.window().set_size(size);
                    if let Some(wa) = work_area(&ui) {
                        keep_on_screen(&ui, wa, size);
                    }
                }
                let end = if ui.get_settings_open() { 16.0 + 368.0 } else { 16.0 };
                if (chrome - end).abs() < 0.01 {
                    APP.with_borrow_mut(|a| {
                        a.hold_stage = None;
                        a.fit_set = Some((target, lh));
                    });
                }
            }
            // Labelled dock is ~690 px wide (measured): icons only below that.
            let bar = ui.get_stage_w() < 720.0;
            if bar != ui.get_compact_bar() {
                ui.set_compact_bar(bar);
            }
            APP.with_borrow_mut(|a| {
                let streaming = a.streaming;
                let device_rotation = a.device_rotation;
                let Some(v) = a.view.as_ref() else { return };
                v.set_crop(p.crop_l as u32, p.crop_t as u32, p.crop_r as u32, p.crop_b as u32);
                v.set_fit(match p.fit.as_str() {
                    "cover" => Fit::Cover,
                    "fill" => Fit::Fill,
                    _ => Fit::Contain,
                });
                // Rotation decision uses the unrotated frame size (read
                // directly: briefly setting rotation 0 let the render thread
                // draw a wrong frame).
                let raw = v.source_size();
                let rot = effective_rotation(&p, raw, device_rotation);
                v.set_rotation(rot);
                input.rotation.store(rot, Relaxed);
                v.set_background(0x000000);
                let (mut x, mut y, mut w, mut h) = (ui.get_stage_x() * sf, ui.get_stage_y() * sf, ui.get_stage_w() * sf, ui.get_stage_h() * sf);
                // Auto-Fit: shrink the native view to the picture itself, so the
                // glass stage shows around it instead of black letterbox bars.
                // Even inset on every side inside the glass stage.
                let inset = PIC_INSET * sf;
                if p.fit != "fill" {
                    x += inset;
                    y += inset;
                    w = (w - 2.0 * inset).max(1.0);
                    h = (h - 2.0 * inset).max(1.0);
                }
                let pic = v.shown_size();
                if p.fit != "cover" && p.fit != "fill" && pic.0 > 0 && pic.1 > 0 && w > 0.0 && h > 0.0 {
                    let aspect = pic.0 as f32 / pic.1 as f32;
                    let (pw, ph) = if w / h > aspect { (h * aspect, h) } else { (w, w / aspect) };
                    x += (w - pw) / 2.0;
                    y += (h - ph) / 2.0;
                    (w, h) = (pw, ph);
                }
                v.set_bounds(x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32, streaming, ((22.0 - PIC_INSET) * sf).round() as i32);
                let shown = v.shown_size();
                if a.fit_prefs.is_none() {
                    a.fit_prefs = Some(fit_prefs_key(&p));
                }
                let free = a.full_restore.is_none() && !ui.window().is_maximized();
                if streaming && p.fit_window && shown.0 > 0 && free {
                    let key = (shown.0, shown.1, ui.get_settings_open());
                    if a.fitted != Some(key) {
                        a.fitted = Some(key);
                        a.fit_set = fit_window(&ui, shown, a.user_scale);
                    } else if a.hold_stage.is_none() {
                        // Not a size ZenLink set: the user resized the window.
                        // Remember how big they want the picture.
                        let cur = ui.window().size().to_logical(sf);
                        let moved = a.fit_set.map(|(w, h)| (cur.width - w).abs() > 2.0 || (cur.height - h).abs() > 2.0).unwrap_or(false);
                        if moved {
                            if let Some((dw, dh)) = default_fit(&ui, shown) {
                                let cw = ui.get_chrome_w() + 2.0 * PIC_INSET;
                                let ch = ui.get_chrome_h() + 2.0 * PIC_INSET;
                                let k = ((cur.width - cw) / dw).min((cur.height - ch) / dh);
                                if k.is_finite() && k > 0.0 {
                                    a.user_scale = k.clamp(0.2, 4.0);
                                }
                            }
                            a.fit_set = Some((cur.width, cur.height));
                        }
                    }
                }
                // Dot field: ~30 fps, only while there is no picture.
                if !streaming && p.dots {
                    let fresh = a.dots_at.map(|t| t.elapsed() >= Duration::from_millis(33)).unwrap_or(true);
                    if fresh {
                        a.dots_at = Some(Instant::now());
                        let t = START.with(|s| s.elapsed().as_secs_f32());
                        let mouse = ui.get_stage_hover().then(|| (ui.get_stage_mouse_x(), ui.get_stage_mouse_y()));
                        let dark = a.dark;
                        let img = a.dots.get_or_insert_with(dots::Dots::new).render(ui.get_stage_w(), ui.get_stage_h(), sf, t, mouse, dark);
                        ui.set_dots_image(img);
                    }
                } else if a.dots.take().is_some() {
                    ui.set_dots_image(slint::Image::default());
                }
                let due = a.meter_at.map(|t| t.elapsed() >= Duration::from_secs(1)).unwrap_or(true);
                if due {
                    a.meter_at = Some(Instant::now());
                    let s = v.stats();
                    if streaming && s.width > 0 && std::env::var_os("ZL_STATS_LOG").is_some() {
                        // Measuring frame pacing (tests): once a second, to the log.
                        log::info!("stats decoded={:.1} presented={:.1} dropped_total={}", s.decoded_fps, s.presented_fps, s.replaced);
                    }
                    if streaming && s.width > 0 {
                        ui.set_stats_line(format!("{}×{}  ·  {:.0} fps  ·  {:.1} Mbps  ·  decode {:.1} ms", s.width, s.height, s.presented_fps.min(s.decoded_fps), s.mbps, s.decode_ms).into());
                        ui.set_stats_full(
                            format!(
                                "Resolusi      {}×{}\nDecode        {:.1} fps ({})\nTampil        {:.1} fps\nTerlewat      {}\nDecode frame  {:.1} ms\nBitrate       {:.1} Mbps",
                                s.width,
                                s.height,
                                s.decoded_fps,
                                if s.hardware { "GPU" } else { "CPU" },
                                s.presented_fps,
                                s.replaced,
                                s.decode_ms,
                                s.mbps
                            )
                            .into(),
                        );
                    } else {
                        ui.set_stats_line("".into());
                        ui.set_stats_full("—".into());
                    }
                }
            });
        });
    }

    // Follow the Windows theme while "System" is selected.
    let theme_timer = slint::Timer::default();
    {
        let weak = ui.as_weak();
        theme_timer.start(slint::TimerMode::Repeated, Duration::from_secs(2), move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_prefs().theme.as_str() == "system" && system_dark() != APP.with_borrow(|a| a.dark) {
                    apply_look(&ui);
                }
            }
        });
    }

    apply_look(&ui);
    let r = ui.run();
    APP.with_borrow_mut(|a| a.view = None);
    if let Some(m) = SIDECAR.get() {
        m.lock().unwrap().take();
    }
    r
}
