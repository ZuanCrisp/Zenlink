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
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_, ev| {
        let Some(ui) = weak.upgrade() else {
            return EventResult::Propagate;
        };
        match ev {
            WindowEvent::HoveredFile(_) => {
                let on_android = ui.get_screen().as_str() == "android";
                ui.set_drop_hover(true);
                ui.set_drop_ok(on_android && phone().is_some());
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
    if ui.get_screen().as_str() != "android" {
        show_toast(
            ui,
            tr_ui(
                ui,
                "Kirim file ke iPhone/iPad belum tersedia di versi ini.",
                "Sending files to an iPhone/iPad is not available in this version yet.",
            ),
        );
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
