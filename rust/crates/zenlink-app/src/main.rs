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

/// Minimal file logger: %TEMP%\zenlink-next.log (truncated on start).
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
    let path = std::env::temp_dir().join("zenlink-next.log");
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
    meter_at: Option<Instant>,
    dark: bool,
    backdrop: Option<(bool, bool)>,
    on_top: Option<bool>,
    dots: Option<dots::Dots>,
    dots_at: Option<Instant>,
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
    ui.global::<Theme>().set_glass(p.glass / 100.0);
    ui.global::<I18n>().set_id(p.lang.as_str() != "en");
    let glass_on = p.glass > 0.0;
    APP.with_borrow_mut(|a| {
        a.dark = dark;
        if a.backdrop != Some((glass_on, dark)) {
            if let Some(h) = hwnd_of(ui.window()) {
                let mode = if glass_on { backdrop::Backdrop::Accent } else { backdrop::Backdrop::None };
                backdrop::apply(h, mode, dark);
                a.backdrop = Some((glass_on, dark));
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

/// Backend state -> UI. Runs on the UI thread.
fn on_state(ui: &AppWindow, api: &Api, st: &Value) {
    let tr = |id: &str, en: &str| -> String { if ui.global::<I18n>().get_id() { id.into() } else { en.into() } };
    let phase = str_of(st, &["phase"]).unwrap_or("idle").to_string();
    let first = st.get("devices").and_then(|d| d.get(0));
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
    let message = if phase == "error" { str_of(st, &["error"]).unwrap_or("").to_string() } else { String::new() };
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
    let service = st.get("service").and_then(Value::as_bool).unwrap_or(true);
    let native_audio = st.get("nativeAudio").and_then(Value::as_bool).unwrap_or(false);
    ui.set_driver_text(
        format!(
            "Apple Mobile Device Service: {}\n{}: {}",
            if service { tr("berjalan", "running") } else { tr("tidak berjalan", "not running") },
            tr("Audio native", "Native audio"),
            if native_audio { tr("tersedia", "available") } else { tr("tidak tersedia", "not available") },
        )
        .into(),
    );
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
fn fit_window(ui: &AppWindow, shown: (u32, u32)) {
    let Some((wx, wy, ww, wh)) = work_area(ui) else { return };
    let chrome_w = ui.get_chrome_w();
    let chrome_h = ui.get_chrome_h();
    let aspect = shown.0 as f32 / shown.1.max(1) as f32;
    // Leave room for the title bar and the taskbar edge.
    // Comfortable, not maximised: portrait ~72% of the screen height,
    // landscape at most ~60% of the screen width.
    let max_h = wh * 0.62 - chrome_h;
    let max_w = ww * 0.55 - chrome_w;
    let (mut sw, mut sh) = (max_h * aspect, max_h);
    if sw > max_w {
        sw = max_w;
        sh = sw / aspect;
    }
    let size = LogicalSize::new((sw + chrome_w).max(360.0), (sh + chrome_h).max(420.0));
    ui.window().set_size(size);
    keep_on_screen(ui, (wx, wy, ww, wh), size);
}

/// Move the window back inside the work area if a resize pushed it out.
fn keep_on_screen(ui: &AppWindow, (wx, wy, ww, wh): (f32, f32, f32, f32), size: LogicalSize) {
    let sf = ui.window().scale_factor();
    let pos = ui.window().position().to_logical(sf);
    let (w, h) = (size.width + 16.0, size.height + 40.0); // frame + title bar
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
        .with_winit_window_attributes_hook(|attrs| attrs.with_transparent(true).with_system_backdrop(BackdropType::None))
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
        ui.on_start_cast(move || {
            let a = api.clone();
            std::thread::spawn(move || a.start_cast());
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
        ui.on_restart(move || {
            let a = api.clone();
            std::thread::spawn(move || {
                a.stop_cast();
                std::thread::sleep(Duration::from_millis(700));
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
                APP.with_borrow_mut(|a| a.fitted = None);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_settings_toggled(move |open| {
            let Some(ui) = weak.upgrade() else { return };
            let fitted = APP.with_borrow(|a| a.streaming && ui.get_prefs().fit_window && a.fitted.is_some());
            if fitted {
                APP.with_borrow_mut(|a| a.fitted = None);
            } else {
                let s = ui.window().size().to_logical(ui.window().scale_factor());
                let dw = if open { 368.0 } else { -368.0 };
                ui.window().set_size(LogicalSize::new((s.width + dw).max(360.0), s.height));
            }
        });
    }
    {
        let weak = ui.as_weak();
        let api = api.clone();
        let input = input.clone();
        ui.on_action(move |a| {
            let Some(ui) = weak.upgrade() else { return };
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
        ui.on_driver_action(move |a| {
            let api = api.clone();
            let a = a.to_string();
            std::thread::spawn(move || match a.as_str() {
                "wifi" => {
                    let _ = api.post("/api/wifi/prepare", &json!({}));
                }
                _ => {
                    let _ = api.get("/api/drivers");
                }
            });
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
            let bar = ui.get_stage_w() < 600.0;
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
                // Rotation decision uses the unrotated frame size.
                v.set_rotation(0);
                let raw = v.shown_size();
                let rot = effective_rotation(&p, raw, device_rotation);
                v.set_rotation(rot);
                input.rotation.store(rot, Relaxed);
                v.set_background(0x000000);
                let (x, y, w, h) = (ui.get_stage_x() * sf, ui.get_stage_y() * sf, ui.get_stage_w() * sf, ui.get_stage_h() * sf);
                v.set_bounds(x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32, streaming, (22.0 * sf).round() as i32);
                let shown = v.shown_size();
                if streaming && p.fit_window && shown.0 > 0 {
                    let key = (shown.0, shown.1, ui.get_settings_open());
                    if a.fitted != Some(key) {
                        a.fitted = Some(key);
                        fit_window(&ui, shown);
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
