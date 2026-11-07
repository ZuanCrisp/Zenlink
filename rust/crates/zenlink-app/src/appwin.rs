//! v2.5.1: one Android app in its own window.
//!
//! The main window starts `ZenLink.exe --android-app serial=â€¦ package=â€¦`.
//! That process opens a small window with its own taskbar entry and starts
//! one scrcpy session on a new virtual display of the phone
//! (`new_display`, Android 10+), then starts the app on it (`START_APP`).
//! The phone's own screen stays free, and several apps can be open at once,
//! each in its own window and session.
//!
//! The window is the same picture path as the main window (`VideoView`,
//! D3D11 child window, one frame per vblank). The virtual display follows
//! the window size (`flex_display` + `RESIZE_DISPLAY`), so the app lays
//! itself out for the window like on a tablet or desktop. Sound is off: it
//! is the phone's sound, and the main window may already play it.
//!
//! This process has no backend, no tray and no single-instance check; it
//! reads the prefs (language, theme, video settings) and never writes them.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::ComponentHandle;
use zenlink_android::apps::is_package_name;
use zenlink_android::control::Control;
use zenlink_android::mirror::NewDisplay;
use zenlink_video::{Fit, PointerEvent, VideoView};

use crate::{android, prefs, AppWin, I18n, Theme};

/// First argument that turns ZenLink.exe into an app window.
pub const ARG: &str = "--android-app";

/// What the main window passes to an app window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppArgs {
    pub serial: String,
    pub transport_id: Option<u64>,
    pub package: String,
    /// App label and device name, for the title only.
    pub name: String,
    pub device: String,
}

impl AppArgs {
    /// The command line after the exe: `--android-app key=valueâ€¦`.
    pub fn to_args(&self) -> Vec<String> {
        let mut v = vec![
            ARG.to_string(),
            format!("serial={}", self.serial),
            format!("package={}", self.package),
            format!("name={}", self.name),
            format!("device={}", self.device),
        ];
        if let Some(t) = self.transport_id {
            v.push(format!("transport={t}"));
        }
        v
    }

    /// Parses the arguments after the exe; `None` when the first one is not
    /// [`ARG`] or the serial or package is missing or unsafe.
    pub fn parse(args: &[String]) -> Option<Self> {
        let (first, rest) = args.split_first()?;
        if first != ARG {
            return None;
        }
        let mut a = AppArgs::default();
        for kv in rest {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            match k {
                "serial" => a.serial = v.to_string(),
                "package" => a.package = v.to_string(),
                "name" => a.name = v.to_string(),
                "device" => a.device = v.to_string(),
                "transport" => a.transport_id = v.parse().ok(),
                _ => {}
            }
        }
        // The serial goes into ADB requests ("host:transport:<serial>"):
        // no spaces or control characters.
        let serial_ok = !a.serial.is_empty()
            && a.serial.len() <= 128
            && a.serial.bytes().all(|b| b.is_ascii_graphic());
        if !serial_ok || !is_package_name(&a.package) {
            return None;
        }
        if a.name.trim().is_empty() {
            a.name = a.package.clone();
        }
        Some(a)
    }
}

/// Opens an app window: a new ZenLink.exe process. A test prefs file
/// (`ZL_PREFS`) is inherited with the environment.
pub fn spawn(a: &AppArgs) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .args(a.to_args())
        .spawn()
        .map(|_| ())
}

/// Virtual display size for a picture area of `w`Ã—`h` pixels: multiples of
/// 8 (video encoders work in blocks), within what phone encoders take.
pub fn display_size(w: f32, h: f32) -> (u16, u16) {
    let fit = |v: f32| ((v.max(0.0) as u32 / 8 * 8).clamp(160, 3840)) as u16;
    (fit(w), fit(h))
}

/// Density for the virtual display: Android's 160 dpi is 1 dp per pixel,
/// so the app's layout follows the PC's display scale.
pub fn display_dpi(scale: f32) -> u32 {
    ((160.0 * scale).round() as u32).clamp(120, 640)
}

struct State {
    view: Option<VideoView>,
    shared: Option<Arc<android::Shared>>,
    /// Display size asked for (start or last RESIZE_DISPLAY).
    asked: (u16, u16),
    /// A new size waiting to settle before it is sent (window being dragged).
    pending: Option<((u16, u16), Instant)>,
    started: bool,
}

thread_local! {
    static ST: RefCell<State> = const {
        RefCell::new(State { view: None, shared: None, asked: (0, 0), pending: None, started: false })
    };
}

/// Picture area in physical pixels: the window below the button strip.
fn picture_rect(ui: &AppWin) -> (f32, f32, f32, f32) {
    let sf = ui.window().scale_factor();
    let size = ui.window().size();
    let top = ui.get_bar_h() * sf;
    (
        0.0,
        top,
        size.width as f32,
        (size.height as f32 - top).max(1.0),
    )
}

fn start_session(ui: &AppWin, a: &AppArgs) {
    stop_session();
    let server = match crate::scrcpy_server() {
        Ok(s) => s,
        Err(e) => {
            ui.set_phase("error".into());
            ui.set_message(e.into());
            return;
        }
    };
    let (_, _, w, h) = picture_rect(ui);
    let size = display_size(w, h);
    let p = prefs::load();
    let mut o = crate::android_options(&p, false);
    o.audio = false;
    o.max_size = 0; // the display already has the window's size
    o.new_display = Some(NewDisplay {
        width: size.0.into(),
        height: size.1.into(),
        dpi: display_dpi(ui.window().scale_factor()),
        flex: true,
    });
    let shared = Arc::new(android::Shared::default());
    let mut src = android::DeviceSource::new(
        a.serial.clone(),
        a.transport_id,
        a.device.clone(),
        server,
        shared.clone(),
        o,
        android::Smart {
            pref: crate::android_pref(&p),
            dec: crate::android_decoders(),
            known: None,
        },
    );
    src.start_app = Some(a.package.clone());
    log::info!(
        "app window: {} on {} ({}x{})",
        a.package,
        a.serial,
        size.0,
        size.1
    );
    android::set_active(Some(shared.clone()));
    ST.with_borrow_mut(|s| {
        s.asked = size;
        s.pending = None;
        s.shared = Some(shared);
        s.started = true;
        if let Some(v) = s.view.as_mut() {
            v.start_source(Box::new(src));
        }
    });
    ui.set_phase("connecting".into());
    ui.set_message("".into());
}

fn stop_session() {
    ST.with_borrow_mut(|s| {
        android::set_active(None);
        if let Some(sh) = s.shared.take() {
            sh.stop();
        }
        if let Some(v) = s.view.as_mut() {
            v.stop();
        }
    });
}

/// Every frame tick: the video view, its place, the status and resizes.
fn tick(ui: &AppWin, a: &AppArgs) {
    let need_view = ST.with_borrow(|s| s.view.is_none());
    if need_view {
        let Some(h) = crate::hwnd_of(ui.window()) else {
            return;
        };
        match VideoView::new(h.0 as isize) {
            Ok(v) => {
                let weak = ui.as_weak();
                v.on_pointer(move |e| {
                    if matches!(e, PointerEvent::Down(..)) {
                        let weak = weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = weak.upgrade() {
                                ui.invoke_focus_keys();
                            }
                        });
                    }
                    if let Some(sh) = android::active() {
                        sh.pointer(e, 0);
                    }
                });
                v.set_fit(Fit::Contain);
                v.set_background(0x000000);
                ST.with_borrow_mut(|s| s.view = Some(v));
            }
            Err(e) => {
                ui.set_phase("error".into());
                ui.set_message(format!("Video: {e}").into());
                return;
            }
        }
    }
    let size = ui.window().size();
    if size.width == 0 || size.height == 0 {
        return;
    }
    if !ST.with_borrow(|s| s.started) {
        start_session(ui, a);
    }
    let (x, y, w, h) = picture_rect(ui);
    let phase = ST.with_borrow(|s| s.shared.as_ref().map(|sh| sh.status()));
    let streaming = phase
        .as_ref()
        .is_some_and(|st| st.phase == android::Phase::Streaming);
    if let Some(st) = &phase {
        let ui_phase = match st.phase {
            android::Phase::Streaming => "streaming",
            android::Phase::Error => "error",
            _ => "connecting",
        };
        if ui.get_phase().as_str() != ui_phase {
            ui.set_phase(ui_phase.into());
            if st.phase == android::Phase::Error {
                log::warn!("app window: {}", st.error);
                ui.set_message(error_text(ui, &st.error).into());
            }
        }
    }
    ST.with_borrow_mut(|s| {
        let Some(v) = s.view.as_ref() else { return };
        v.set_bounds(x as i32, y as i32, w as i32, h as i32, streaming, 0);
        // Flex display: follow the window once its size settles.
        let want = display_size(w, h);
        let now = Instant::now();
        if want == s.asked {
            s.pending = None;
        } else if s.pending.map(|(sz, _)| sz) != Some(want) {
            s.pending = Some((want, now));
        } else if let Some((sz, at)) = s.pending {
            if streaming && now.duration_since(at) >= Duration::from_millis(250) {
                if let Some(sh) = &s.shared {
                    sh.send(&[Control::ResizeDisplay {
                        width: sz.0,
                        height: sz.1,
                    }]);
                }
                s.asked = sz;
                s.pending = None;
            }
        }
    });
}

/// A short bilingual reason for the window, from the session's English error.
fn error_text(ui: &AppWin, e: &str) -> String {
    let id = ui.global::<I18n>().get_id();
    let low = e.to_lowercase();
    if low.contains("device") && (low.contains("not found") || low.contains("offline")) {
        return if id {
            "HP tidak tersambung lagi. Sambungkan lagi, lalu Coba lagi.".into()
        } else {
            "The phone is no longer connected. Connect it again, then Try again.".into()
        };
    }
    if id {
        format!("App tidak bisa dibuka di layar virtual (Android 10+): {e}")
    } else {
        format!("The app could not open on a virtual display (Android 10+): {e}")
    }
}

/// Runs the app window until it is closed.
pub fn run(a: AppArgs) -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()?;
    let ui = AppWin::new()?;
    let p = prefs::load();
    let dark = match p.theme.as_str() {
        "light" => false,
        "system" => crate::system_dark(),
        _ => true,
    };
    ui.global::<Theme>().set_dark(dark);
    ui.global::<Theme>().set_glass(0.0);
    ui.global::<Theme>().set_icons(crate::icon_font().into());
    ui.global::<I18n>().set_id(p.lang.as_str() != "en");
    ui.set_app_name(a.name.clone().into());
    ui.set_device(a.device.clone().into());

    ui.on_key_input(|text, ctrl, shift, alt| {
        let Some(sh) = android::active() else {
            return false;
        };
        match android::key_input(text.as_str(), ctrl, shift, alt) {
            android::KeyInput::Send(m) => sh.send(&m),
            android::KeyInput::Paste => {
                if let Some(t) = crate::pc_clipboard_text() {
                    sh.paste(t);
                }
            }
            android::KeyInput::Ignore => return false,
        }
        true
    });
    {
        let package = a.package.clone();
        ui.on_action(move |name| {
            let Some(sh) = android::active() else { return };
            match name.as_str() {
                // "+" force-stops it first: a clean start of the app.
                "restart-app" => sh.send(&[Control::StartApp(format!("+{package}"))]),
                n => {
                    sh.action(n);
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let a = a.clone();
        ui.on_retry(move || {
            if let Some(ui) = weak.upgrade() {
                start_session(&ui, &a);
            }
        });
    }
    ui.window().on_close_requested(|| {
        stop_session();
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });

    let timer = slint::Timer::default();
    {
        let weak = ui.as_weak();
        let mut dark_bar = false;
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(ui) = weak.upgrade() else { return };
                if !dark_bar {
                    if let Some(h) = crate::hwnd_of(ui.window()) {
                        crate::backdrop::dark_title_bar(h, dark);
                        dark_bar = true;
                    }
                }
                tick(&ui, &a);
            },
        );
    }
    ui.run()?;
    stop_session();
    ST.with_borrow_mut(|s| s.view = None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AppArgs {
        AppArgs {
            serial: "192.168.1.20:37099".into(),
            transport_id: Some(7),
            package: "org.videolan.vlc".into(),
            name: "VLC for Android".into(),
            device: "TECNO POVA 6".into(),
        }
    }

    #[test]
    fn arguments_round_trip() {
        let a = sample();
        assert_eq!(AppArgs::parse(&a.to_args()), Some(a));
    }

    #[test]
    fn only_the_app_window_argument_starts_it() {
        assert_eq!(AppArgs::parse(&[]), None);
        assert_eq!(AppArgs::parse(&["--background".into()]), None);
    }

    #[test]
    fn unsafe_values_are_refused() {
        let mut a = sample();
        a.package = "org.x;reboot".into();
        assert_eq!(AppArgs::parse(&a.to_args()), None);
        let mut a = sample();
        a.serial = "abc def".into();
        assert_eq!(AppArgs::parse(&a.to_args()), None);
        let mut a = sample();
        a.serial.clear();
        assert_eq!(AppArgs::parse(&a.to_args()), None);
    }

    #[test]
    fn missing_name_falls_back_to_the_package() {
        let a =
            AppArgs::parse(&[ARG.into(), "serial=X1".into(), "package=com.a.b".into()]).unwrap();
        assert_eq!(a.name, "com.a.b");
        assert_eq!(a.transport_id, None);
    }

    #[test]
    fn display_follows_the_window_in_blocks_of_8() {
        assert_eq!(display_size(430.0, 776.0), (424, 776));
        assert_eq!(display_size(100.0, 9000.0), (160, 3840));
        assert_eq!(display_dpi(1.0), 160);
        assert_eq!(display_dpi(1.5), 240);
        assert_eq!(display_dpi(10.0), 640);
    }
}
