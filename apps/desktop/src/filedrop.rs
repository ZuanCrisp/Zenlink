//! Files dropped on the window (v2.5.1): while an Android phone is
//! mirrored, an `.apk` is installed and any other file goes to the phone's
//! Download folder (`zenlink_android::files`). One file at a time, in a
//! background thread; the window shows a progress pill.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

use slint::winit_030::winit::event::WindowEvent;
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::ComponentHandle;
use zenlink_android::adb::{AdbClient, Target};
use zenlink_android::files::{self, Installed, Kind};

use crate::{show_toast, tr_ui, AppWindow, APP};

/// One dropped file and the phone it goes to.
struct Job {
    path: PathBuf,
    serial: String,
    transport_id: Option<u64>,
    device: String,
}

fn queue(ui: &AppWindow) -> &'static Mutex<Sender<Job>> {
    static Q: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    Q.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            for job in rx {
                run(&weak, job);
            }
        });
        Mutex::new(tx)
    })
}

/// Sets the pill from any thread: `both` is "Indonesian|English" (empty
/// hides it), `progress` 0..1.
fn pill(weak: &slint::Weak<AppWindow>, both: &str, progress: f32) {
    let (id, en) = both.split_once('|').unwrap_or((both, both));
    let (id, en) = (id.to_string(), en.to_string());
    let w = weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = w.upgrade() {
            ui.set_transfer(tr_ui(&ui, &id, &en).into());
            ui.set_transfer_p(progress);
        }
    });
}

fn finished(weak: &slint::Weak<AppWindow>, id: String, en: String) {
    let w = weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = w.upgrade() {
            ui.set_transfer("".into());
            ui.set_transfer_p(-1.0);
            let t = tr_ui(&ui, &id, &en);
            show_toast(&ui, t);
        }
    });
}

fn run(weak: &slint::Weak<AppWindow>, job: Job) {
    let name = job
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let size = std::fs::metadata(&job.path).map(|m| m.len()).unwrap_or(0);
    let kind = files::kind(&job.path);
    let start = match kind {
        Kind::Install => format!("Memasang {name}…|Installing {name}…"),
        Kind::Copy => format!("Mengirim {name}…|Sending {name}…"),
    };
    pill(weak, &start, 0.0);
    let client = AdbClient::default();
    let target = match job.transport_id {
        Some(id) => Target::TransportId(id),
        None => Target::Serial(&job.serial),
    };
    let mut last = std::time::Instant::now();
    let w = weak.clone();
    let mut progress = move |n: u64| {
        if last.elapsed().as_millis() >= 100 && size > 0 {
            last = std::time::Instant::now();
            pill(&w, &start, (n as f64 / size as f64).min(1.0) as f32);
        }
    };
    let t = std::time::Instant::now();
    let device = &job.device;
    match kind {
        Kind::Copy => match files::copy_to_download(&client, &target, &job.path, &mut progress) {
            Ok(saved) => {
                log::info!(
                    "drop: {} ({size} bytes) to Download/{saved} on {device} in {:?}",
                    job.path.display(),
                    t.elapsed()
                );
                finished(
                    weak,
                    format!("Tersimpan di {device} › Download: {saved}"),
                    format!("Saved on {device} › Download: {saved}"),
                );
            }
            Err(e) => {
                log::warn!("drop: {} failed: {e}", job.path.display());
                finished(
                    weak,
                    format!("Gagal mengirim {name}: {e}"),
                    format!("Could not send {name}: {e}"),
                );
            }
        },
        Kind::Install => {
            // The copy is most of the time; the install itself has no progress.
            let r = files::install_apk(&client, &target, &job.path, &mut progress);
            match r {
                Ok(Installed::Ok) => {
                    log::info!(
                        "drop: installed {} on {device} in {:?}",
                        job.path.display(),
                        t.elapsed()
                    );
                    finished(
                        weak,
                        format!("{name} terpasang di {device}."),
                        format!("{name} is installed on {device}."),
                    );
                }
                Ok(Installed::Failed(why)) => {
                    log::warn!("drop: install {} failed: {why}", job.path.display());
                    finished(
                        weak,
                        format!("{name} tidak terpasang: {why}"),
                        format!("{name} was not installed: {why}"),
                    );
                }
                Err(e) => {
                    log::warn!("drop: install {} failed: {e}", job.path.display());
                    finished(
                        weak,
                        format!("{name} tidak terpasang: {e}"),
                        format!("{name} was not installed: {e}"),
                    );
                }
            }
        }
    }
    pill(weak, "", -1.0);
}

/// The phone files go to: the one being mirrored, if any.
fn phone() -> Option<(String, Option<u64>, String)> {
    APP.with_borrow(|a| {
        a.android.as_ref()?;
        let d = a.apps_device.as_ref()?;
        Some((
            d.primary.serial.clone(),
            d.primary.transport_id,
            d.name.display.clone(),
        ))
    })
}

/// Watches the window for files dragged in from Explorer.
pub fn install(ui: &AppWindow) {
    install_ios(ui);
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_, ev| {
        let Some(ui) = weak.upgrade() else {
            return EventResult::Propagate;
        };
        match ev {
            WindowEvent::HoveredFile(_) => {
                let apple = ui.get_screen().as_str() != "android";
                ui.set_drop_hover(true);
                ui.set_drop_apple(apple);
                ui.set_drop_ok(if apple {
                    ui.get_mode_active().as_str() == "apple"
                } else {
                    phone().is_some() || ui.get_drawer_files()
                });
                EventResult::PreventDefault
            }
            WindowEvent::HoveredFileCancelled => {
                ui.set_drop_hover(false);
                EventResult::PreventDefault
            }
            WindowEvent::DroppedFile(path) => {
                ui.set_drop_hover(false);
                drop_file(&ui, path.clone());
                EventResult::PreventDefault
            }
            _ => EventResult::Propagate,
        }
    });
}

fn drop_file(ui: &AppWindow, path: PathBuf) {
    // The Files drawer is open: the file goes to the folder it shows.
    if crate::filewin::dropped(ui, &path) {
        return;
    }
    if ui.get_screen().as_str() != "android" {
        ios::dropped(ui, path);
        return;
    }
    let Some((serial, transport_id, device)) = phone() else {
        show_toast(
            ui,
            tr_ui(
                ui,
                "Mulai mirroring HP Android dulu, lalu tarik file ke sini.",
                "Start mirroring an Android phone first, then drop the file here.",
            ),
        );
        return;
    };
    if !path.is_file() {
        show_toast(
            ui,
            tr_ui(
                ui,
                "Folder belum bisa dikirim: pilih file-nya.",
                "Folders cannot be sent yet: drop the files.",
            ),
        );
        return;
    }
    let _ = queue(ui).lock().unwrap().send(Job {
        path,
        serial,
        transport_id,
        device,
    });
}

/// Registers the iPhone/iPad app picker's callbacks.
fn install_ios(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_files_pick(move |i| {
        if let Some(ui) = weak.upgrade() {
            ios::pick(&ui, i);
        }
    });
    let weak = ui.as_weak();
    ui.on_files_close(move || {
        if let Some(ui) = weak.upgrade() {
            ios::close(&ui);
        }
    });
}

/// iPhone/iPad (Apple Driver mode): the backend streams the files into the
/// Documents folder of an app with File Sharing (zenlink/apple/files.py).
mod ios {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::time::Duration;

    use serde_json::{json, Value};
    use slint::ComponentHandle;
    use zenlink_core::api::Api;

    use super::pill;
    use crate::{app_letters, app_tint, show_toast, tr_ui, AppRow, AppWindow};

    thread_local! {
        /// Files waiting for the user to pick an app.
        static PENDING: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
        /// (bundle id, name) of the apps shown.
        static APPS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
    }

    fn msg(v: &Value) -> String {
        v.get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    fn set_title(ui: &AppWindow) {
        let n = PENDING.with_borrow(Vec::len);
        let t = tr_ui(
            ui,
            &format!("Kirim {n} file ke app di iPhone/iPad"),
            &format!("Send {n} file(s) to an app on the iPhone/iPad"),
        );
        ui.set_files_title(t.into());
    }

    fn list_error(ui: &AppWindow, v: Option<&Value>) -> String {
        let m = v.map(msg).unwrap_or_default();
        if !m.is_empty() {
            return m;
        }
        tr_ui(
            ui,
            "Daftar app iPhone/iPad tidak terbaca. Pastikan perangkat tersambung di mode Apple Driver dan sudah dipercaya (Trust).",
            "Could not read the iPhone/iPad apps. Check that the device is connected in Apple Driver mode and trusted.",
        )
    }

    pub fn dropped(ui: &AppWindow, path: PathBuf) {
        if ui.get_mode_active().as_str() != "apple" {
            show_toast(
                ui,
                tr_ui(
                    ui,
                    "Kirim file ke iPhone/iPad butuh mode Apple Driver (ZenLink Studio tidak bisa).",
                    "Sending files to an iPhone/iPad needs Apple Driver mode (ZenLink Studio cannot).",
                ),
            );
            return;
        }
        if !path.is_file() {
            show_toast(
                ui,
                tr_ui(
                    ui,
                    "Folder belum bisa dikirim: pilih file-nya.",
                    "Folders cannot be sent yet: drop the files.",
                ),
            );
            return;
        }
        let first = PENDING.with_borrow_mut(|p| {
            p.push(path);
            p.len() == 1
        });
        set_title(ui);
        if !first || ui.get_files_open() {
            return;
        }
        ui.set_files_open(true);
        ui.set_files_busy(true);
        ui.set_files_apps(std::rc::Rc::new(slint::VecModel::<AppRow>::default()).into());
        ui.set_files_status(
            tr_ui(
                ui,
                "Membaca app di iPhone/iPad…",
                "Reading the iPhone/iPad apps…",
            )
            .into(),
        );
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let r = Api::new().get_slow("/api/files/apps", 30);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = weak.upgrade() else { return };
                ui.set_files_busy(false);
                let ok = r
                    .as_ref()
                    .and_then(|v| v.get("ok"))
                    .and_then(Value::as_bool)
                    == Some(true);
                if !ok {
                    let m = list_error(&ui, r.as_ref());
                    ui.set_files_status(m.into());
                    return;
                }
                let apps: Vec<(String, String)> = r
                    .as_ref()
                    .and_then(|v| v.get("apps"))
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| {
                                Some((
                                    x.get("bundle")?.as_str()?.to_string(),
                                    x.get("name")?.as_str()?.to_string(),
                                ))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let rows: Vec<AppRow> = apps
                    .iter()
                    .map(|(b, n)| AppRow {
                        name: n.as_str().into(),
                        sub: b.as_str().into(),
                        letter: app_letters(n).into(),
                        tint: app_tint(b),
                    })
                    .collect();
                let status = if rows.is_empty() {
                    tr_ui(
                        &ui,
                        "Tidak ada app yang menerima file (File Sharing). Pasang mis. VLC atau Documents dari App Store.",
                        "No app accepts files (File Sharing). Install e.g. VLC or Documents from the App Store.",
                    )
                } else {
                    tr_ui(
                        &ui,
                        "Pilih app: file masuk ke folder app itu dan terlihat di app Files › Di iPhone Saya.",
                        "Pick an app: the files go to its folder and show in Files › On My iPhone.",
                    )
                };
                ui.set_files_status(status.into());
                APPS.with_borrow_mut(|a| *a = apps);
                ui.set_files_apps(std::rc::Rc::new(slint::VecModel::from(rows)).into());
            });
        });
    }

    pub fn close(ui: &AppWindow) {
        ui.set_files_open(false);
        PENDING.with_borrow_mut(Vec::clear);
    }

    pub fn pick(ui: &AppWindow, index: i32) {
        let Some((bundle, app)) = APPS.with_borrow(|a| a.get(index.max(0) as usize).cloned())
        else {
            return;
        };
        let paths = PENDING.with_borrow_mut(std::mem::take);
        ui.set_files_open(false);
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let api = Api::new();
            let total = paths.len();
            let mut sent = 0;
            let mut last_error = String::new();
            for (i, path) in paths.iter().enumerate() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let k = i + 1;
                let label = format!(
                    "Mengirim {name} ke {app} ({k}/{total})…|Sending {name} to {app} ({k}/{total})…"
                );
                pill(&weak, &label, 0.0);
                let r = api.post_slow(
                    "/api/files/send",
                    &json!({"path": path.to_string_lossy(), "bundle": bundle, "app": app}),
                    15,
                );
                if r.as_ref()
                    .and_then(|v| v.get("ok"))
                    .and_then(Value::as_bool)
                    != Some(true)
                {
                    last_error = r.as_ref().map(msg).unwrap_or_else(|| "backend".into());
                    log::warn!("files: {} not started: {last_error}", path.display());
                    continue;
                }
                loop {
                    std::thread::sleep(Duration::from_millis(200));
                    let Some(j) = api.get("/api/files/job") else {
                        continue;
                    };
                    let state = j.get("state").and_then(Value::as_str).unwrap_or("");
                    let done = j.get("sent").and_then(Value::as_u64).unwrap_or(0);
                    let size = j.get("total").and_then(Value::as_u64).unwrap_or(0);
                    match state {
                        "running" => {
                            let p = if size > 0 {
                                done as f32 / size as f32
                            } else {
                                0.0
                            };
                            pill(&weak, &label, p);
                        }
                        "done" => {
                            sent += 1;
                            break;
                        }
                        _ => {
                            last_error = msg(&j);
                            break;
                        }
                    }
                }
            }
            pill(&weak, "", -1.0);
            let (id, en) = if sent == total {
                (
                    format!("{sent} file terkirim ke {app}. Buka app {app} atau Files › Di iPhone Saya."),
                    format!("{sent} file(s) sent to {app}. Open {app} or Files › On My iPhone."),
                )
            } else {
                (
                    format!("{sent} dari {total} file terkirim ke {app}. {last_error}"),
                    format!("{sent} of {total} file(s) sent to {app}. {last_error}"),
                )
            };
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    let t = tr_ui(&ui, &id, &en);
                    show_toast(&ui, t);
                }
            });
        });
    }
}
