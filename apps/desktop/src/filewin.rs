//! v2.6: an Android phone's files in their own window.
//!
//! The main window starts `ZenLink.exe --android-files serial=… device=…`
//! (one process per phone, like the app and device windows). It browses the
//! phone's shared storage over the ADB sync protocol
//! (`zenlink_android::browse`), copies files to the PC
//! (`Downloads\ZenLink\<phone>`) and sends files dropped from Explorer to the
//! folder that is open. Copies run one at a time on their own thread with
//! progress and cancel; browsing stays responsive meanwhile.
//!
//! This process has no backend, no tray and no single-instance check; it
//! reads the prefs (language, theme) and never writes them.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::winit_030::winit::event::WindowEvent;
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, VecModel};
use zenlink_android::adb::{AdbClient, Target};
use zenlink_android::browse::{self, CopyError, Entry};

use crate::{prefs, FileRow, FileWin, I18n, PlaceRow, Theme};

/// First argument that turns ZenLink.exe into a file window.
pub const ARG: &str = "--android-files";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileArgs {
    pub serial: String,
    pub transport_id: Option<u64>,
    pub device: String,
}

impl FileArgs {
    pub fn to_args(&self) -> Vec<String> {
        let mut v = vec![
            ARG.to_string(),
            format!("serial={}", self.serial),
            format!("device={}", self.device),
        ];
        if let Some(t) = self.transport_id {
            v.push(format!("transport={t}"));
        }
        v
    }

    /// `None` unless the first argument is [`ARG`] and the serial is safe
    /// for ADB requests (no spaces or control characters).
    pub fn parse(args: &[String]) -> Option<Self> {
        let (first, rest) = args.split_first()?;
        if first != ARG {
            return None;
        }
        let mut a = FileArgs::default();
        for kv in rest {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            match k {
                "serial" => a.serial = v.to_string(),
                "device" => a.device = v.to_string(),
                "transport" => a.transport_id = v.parse().ok(),
                _ => {}
            }
        }
        let ok = !a.serial.is_empty()
            && a.serial.len() <= 128
            && a.serial.bytes().all(|b| b.is_ascii_graphic());
        if !ok {
            return None;
        }
        if a.device.trim().is_empty() {
            a.device = a.serial.clone();
        }
        Some(a)
    }

    fn target(&self) -> Target<'_> {
        match self.transport_id {
            Some(id) => Target::TransportId(id),
            None => Target::Serial(&self.serial),
        }
    }
}

pub fn spawn(a: &FileArgs) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .args(a.to_args())
        .spawn()
        .map(|_| ())
}

/// "12.3 MB"; folders show a word instead.
pub fn size_text(bytes: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else if v < 10.0 {
        format!("{v:.1} {}", U[i])
    } else {
        format!("{v:.0} {}", U[i])
    }
}

/// `YYYY-MM-DD` (UTC) from Unix seconds; empty for 0.
pub fn date_text(secs: u32) -> String {
    if secs == 0 {
        return String::new();
    }
    // Civil-from-days (Howard Hinnant), no time zone library needed.
    let z = i64::from(secs) / 86_400 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Segoe Fluent Icons code point for an entry.
pub fn icon_for(e: &Entry) -> &'static str {
    if e.is_dir() {
        return "\u{E8B7}";
    }
    let ext = e
        .name
        .rsplit_once('.')
        .map(|(_, x)| x.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif" | "bmp" | "dng" => "\u{EB9F}",
        "mp4" | "mkv" | "mov" | "webm" | "3gp" | "avi" => "\u{E714}",
        "mp3" | "m4a" | "aac" | "flac" | "wav" | "ogg" | "opus" | "amr" => "\u{E8D6}",
        "apk" | "apks" | "xapk" => "\u{E7B8}",
        "pdf" | "doc" | "docx" | "txt" | "xls" | "xlsx" | "ppt" | "pptx" | "md" => "\u{E8A5}",
        "zip" | "rar" | "7z" | "tar" | "gz" => "\u{F012}",
        _ => "\u{E7C3}",
    }
}

/// Where copies from the phone land: `Downloads\ZenLink\<phone>`.
pub fn download_dir(device: &str) -> PathBuf {
    let base = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Downloads")
        .join("ZenLink");
    let name = zenlink_android::files::safe_name(device).unwrap_or_else(|| "Android".into());
    base.join(name)
}

enum CopyJob {
    Pull {
        remote: String,
        size: u64,
        open: bool,
    },
    Push {
        local: PathBuf,
        dir: String,
    },
}

impl CopyJob {
    fn label(&self) -> String {
        match self {
            CopyJob::Pull { remote, .. } => remote.rsplit('/').next().unwrap_or("").to_string(),
            CopyJob::Push { local, .. } => local
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }
}

struct State {
    path: String,
    entries: Vec<Entry>,
    hidden: bool,
    /// Bumped on every listing request; a late answer for an older folder is dropped.
    gen: u64,
}

thread_local! {
    static ST: RefCell<State> = const {
        RefCell::new(State { path: String::new(), entries: Vec::new(), hidden: false, gen: 0 })
    };
}

fn tr(ui: &FileWin, id: &str, en: &str) -> String {
    if ui.global::<I18n>().get_id() {
        id.to_string()
    } else {
        en.to_string()
    }
}

fn show_entries(ui: &FileWin) {
    let rows: Vec<FileRow> = ST.with_borrow(|s| {
        s.entries
            .iter()
            .map(|e| FileRow {
                name: e.name.clone().into(),
                size: if e.is_dir() {
                    "".into()
                } else {
                    size_text(e.size).into()
                },
                date: date_text(e.mtime).into(),
                dir: e.is_dir(),
                icon: icon_for(e).into(),
            })
            .collect()
    });
    ui.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_selected(-1);
}

/// Lists `path` on a short-lived thread and shows it when it arrives.
fn go(ui: &FileWin, a: &FileArgs, path: String) {
    let (gen, hidden) = ST.with_borrow_mut(|s| {
        s.gen += 1;
        s.path = path.clone();
        (s.gen, s.hidden)
    });
    ui.set_path(path.clone().into());
    ui.set_phase("loading".into());
    let weak = ui.as_weak();
    let a = a.clone();
    std::thread::spawn(move || {
        let r = browse::list(&AdbClient::default(), &a.target(), &path, hidden);
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = weak.upgrade() else { return };
            if ST.with_borrow(|s| s.gen) != gen {
                return;
            }
            match r {
                Ok(v) => {
                    ST.with_borrow_mut(|s| s.entries = v);
                    show_entries(&ui);
                    ui.set_phase("ready".into());
                }
                Err(e) => {
                    log::warn!("files: list {path}: {e}");
                    let msg = error_text(&ui, &e.to_string());
                    ui.set_message(msg.into());
                    ui.set_phase("error".into());
                }
            }
        });
    });
}

fn error_text(ui: &FileWin, e: &str) -> String {
    let low = e.to_lowercase();
    if low.contains("permission denied") {
        tr(
            ui,
            "Android tidak mengizinkan folder ini dibuka.",
            "Android does not allow opening this folder.",
        )
    } else if low.contains("not found") || low.contains("offline") || low.contains("no devices") {
        tr(
            ui,
            "HP tidak tersambung. Sambungkan lagi, lalu Muat ulang.",
            "The phone is not connected. Connect it again, then Refresh.",
        )
    } else {
        format!("{}: {e}", tr(ui, "Gagal", "Failed"))
    }
}

fn entry_at(i: i32) -> Option<(String, Entry)> {
    ST.with_borrow(|s| {
        let e = s.entries.get(usize::try_from(i).ok()?)?.clone();
        Some((browse::join(&s.path, &e.name), e))
    })
}

/// The copy thread: one job at a time, progress shown in the strip.
fn start_copier(
    ui: &FileWin,
    a: &FileArgs,
    cancel: Arc<AtomicBool>,
    queued: Arc<AtomicUsize>,
) -> Sender<CopyJob> {
    let (tx, rx) = channel::<CopyJob>();
    let weak = ui.as_weak();
    let a = a.clone();
    std::thread::spawn(move || {
        let client = AdbClient::default();
        for job in rx {
            queued.fetch_sub(1, Ordering::Relaxed);
            cancel.store(false, Ordering::Relaxed);
            let label = job.label();
            let total = match &job {
                CopyJob::Pull { size, .. } => *size,
                CopyJob::Push { local, .. } => std::fs::metadata(local).map_or(0, |m| m.len()),
            };
            {
                let weak = weak.clone();
                let label = label.clone();
                let q = queued.load(Ordering::Relaxed) as i32;
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_busy(true);
                        ui.set_job(label.into());
                        ui.set_job_progress(if total > 0 { 0.0 } else { -1.0 });
                        ui.set_queued(q);
                    }
                });
            }
            let mut last = Instant::now();
            let mut progress = |done: u64| {
                if total == 0 || last.elapsed() < Duration::from_millis(80) {
                    return;
                }
                last = Instant::now();
                let f = (done as f64 / total as f64).min(1.0) as f32;
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_job_progress(f);
                    }
                });
            };
            let target = a.target();
            let result: Result<(String, Option<PathBuf>, bool), CopyError> = match &job {
                CopyJob::Pull { remote, open, .. } => {
                    let dir = download_dir(&a.device);
                    std::fs::create_dir_all(&dir)
                        .map_err(CopyError::from)
                        .and_then(|_| {
                            browse::pull(&client, &target, remote, &dir, &mut progress, &cancel)
                        })
                        .map(|p| (label.clone(), Some(p), *open))
                }
                CopyJob::Push { local, dir } => {
                    browse::push(&client, &target, local, dir, &mut progress, &cancel)
                        .map(|name| (name, None, false))
                }
            };
            let refresh = matches!(job, CopyJob::Push { .. });
            let weak = weak.clone();
            let a2 = a.clone();
            let left = queued.load(Ordering::Relaxed);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = weak.upgrade() else { return };
                if left == 0 {
                    ui.set_busy(false);
                }
                match result {
                    Ok((name, Some(path), open)) => {
                        log::info!("files: copied {} to {}", name, path.display());
                        ui.set_notice(
                            format!("{} {}", tr(&ui, "Tersalin ke", "Copied to"), path.display())
                                .into(),
                        );
                        if open {
                            open_on_pc(&path, false);
                        }
                    }
                    Ok((name, None, _)) => {
                        log::info!("files: sent {name}");
                        ui.set_notice(format!("{} {name}", tr(&ui, "Terkirim:", "Sent:")).into());
                    }
                    Err(CopyError::Cancelled) => {
                        ui.set_notice(tr(&ui, "Dibatalkan.", "Cancelled.").into());
                    }
                    Err(CopyError::Adb(e)) => {
                        log::warn!("files: copy failed: {e}");
                        ui.set_notice(error_text(&ui, &e.to_string()).into());
                    }
                }
                if refresh {
                    let path = ST.with_borrow(|s| s.path.clone());
                    go(&ui, &a2, path);
                }
            });
        }
    });
    tx
}

/// Opens a file with its app, or a folder (or `select` a file) in Explorer.
fn open_on_pc(path: &Path, select: bool) {
    let mut cmd = std::process::Command::new("explorer.exe");
    if select {
        cmd.arg(format!("/select,{}", path.display()));
    } else {
        cmd.arg(path);
    }
    if let Err(e) = cmd.spawn() {
        log::warn!("files: explorer: {e}");
    }
}

/// Runs the file window until it is closed.
pub fn run(a: FileArgs) -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()?;
    let ui = FileWin::new()?;
    let p = prefs::load();
    let dark = match p.theme.as_str() {
        "light" => false,
        "system" => crate::system_dark(),
        _ => true,
    };
    ui.global::<Theme>().set_dark(dark);
    ui.global::<Theme>().set_glass(0.0);
    ui.global::<Theme>().set_icons(crate::icon_font().into());
    let id = p.lang.as_str() != "en";
    ui.global::<I18n>().set_id(id);
    ui.set_device(a.device.clone().into());
    let places: Vec<PlaceRow> = browse::QUICK_DIRS
        .iter()
        .map(|(label, path)| {
            let (label_id, icon) = match *label {
                "Internal storage" => ("Penyimpanan internal", "\u{EDA2}"),
                "Camera" => ("Kamera", "\u{E722}"),
                "Download" => ("Unduhan", "\u{E896}"),
                "Pictures" => ("Gambar", "\u{EB9F}"),
                "Movies" => ("Video", "\u{E714}"),
                "Documents" => ("Dokumen", "\u{E8A5}"),
                "Music" => ("Musik", "\u{E8D6}"),
                other => (other, "\u{E8B7}"),
            };
            PlaceRow {
                label: if id { label_id } else { label }.into(),
                path: (*path).into(),
                icon: icon.into(),
            }
        })
        .collect();
    ui.set_places(ModelRc::from(Rc::new(VecModel::from(places))));

    let cancel = Arc::new(AtomicBool::new(false));
    let queued = Arc::new(AtomicUsize::new(0));
    let copier = start_copier(&ui, &a, cancel.clone(), queued.clone());
    let enqueue = {
        let queued = queued.clone();
        move |tx: &Sender<CopyJob>, job: CopyJob| {
            queued.fetch_add(1, Ordering::Relaxed);
            let _ = tx.send(job);
        }
    };

    {
        let weak = ui.as_weak();
        let a = a.clone();
        ui.on_go(move |path| {
            if let Some(ui) = weak.upgrade() {
                go(&ui, &a, path.to_string());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let a = a.clone();
        ui.on_up(move || {
            let Some(ui) = weak.upgrade() else { return };
            let path = ST.with_borrow(|s| s.path.clone());
            if let Some(p) = browse::parent(&path) {
                go(&ui, &a, p.to_string());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let a = a.clone();
        ui.on_refresh(move || {
            if let Some(ui) = weak.upgrade() {
                let path = ST.with_borrow(|s| s.path.clone());
                go(&ui, &a, path);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let a = a.clone();
        ui.on_show_hidden(move |on| {
            let Some(ui) = weak.upgrade() else { return };
            ST.with_borrow_mut(|s| s.hidden = on);
            ui.set_hidden_files(on);
            let path = ST.with_borrow(|s| s.path.clone());
            go(&ui, &a, path);
        });
    }
    {
        let weak = ui.as_weak();
        let a = a.clone();
        let tx = copier.clone();
        let enqueue = enqueue.clone();
        ui.on_open_row(move |i| {
            let Some(ui) = weak.upgrade() else { return };
            let Some((path, e)) = entry_at(i) else { return };
            if e.is_dir() || e.is_link() {
                go(&ui, &a, path);
            } else {
                enqueue(
                    &tx,
                    CopyJob::Pull {
                        remote: path,
                        size: e.size,
                        open: true,
                    },
                );
            }
        });
    }
    {
        let tx = copier.clone();
        let enqueue = enqueue.clone();
        ui.on_copy_to_pc(move |i| {
            if let Some((path, e)) = entry_at(i) {
                if !e.is_dir() {
                    enqueue(
                        &tx,
                        CopyJob::Pull {
                            remote: path,
                            size: e.size,
                            open: false,
                        },
                    );
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let a = a.clone();
        ui.on_delete_row(move |i| {
            let Some(ui) = weak.upgrade() else { return };
            let Some((path, _)) = entry_at(i) else { return };
            let weak = ui.as_weak();
            let a = a.clone();
            std::thread::spawn(move || {
                let r = browse::delete(&AdbClient::default(), &a.target(), &path);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    match r {
                        Ok(()) => {
                            log::info!("files: deleted {path}");
                            ui.set_notice(tr(&ui, "Terhapus.", "Deleted.").into());
                        }
                        Err(e) => ui.set_notice(error_text(&ui, &e.to_string()).into()),
                    }
                    let cur = ST.with_borrow(|s| s.path.clone());
                    go(&ui, &a, cur);
                });
            });
        });
    }
    {
        let weak = ui.as_weak();
        let a = a.clone();
        ui.on_new_folder(move |name| {
            let Some(ui) = weak.upgrade() else { return };
            let Some(name) = zenlink_android::files::safe_name(name.as_str()) else {
                ui.set_notice(
                    tr(
                        &ui,
                        "Nama folder tidak bisa dipakai.",
                        "That folder name cannot be used.",
                    )
                    .into(),
                );
                return;
            };
            let dir = browse::join(&ST.with_borrow(|s| s.path.clone()), &name);
            let weak = ui.as_weak();
            let a = a.clone();
            std::thread::spawn(move || {
                let r = browse::make_dir(&AdbClient::default(), &a.target(), &dir);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    if let Err(e) = r {
                        ui.set_notice(error_text(&ui, &e.to_string()).into());
                    }
                    let cur = ST.with_borrow(|s| s.path.clone());
                    go(&ui, &a, cur);
                });
            });
        });
    }
    {
        let cancel = cancel.clone();
        ui.on_cancel_job(move || cancel.store(true, Ordering::Relaxed));
    }
    {
        let dir = download_dir(&a.device);
        ui.on_open_downloads(move || {
            let _ = std::fs::create_dir_all(&dir);
            open_on_pc(&dir, false);
        });
    }
    // Files dragged in from Explorer go to the open folder.
    {
        let weak = ui.as_weak();
        let tx = copier.clone();
        let enqueue = enqueue.clone();
        ui.window().on_winit_window_event(move |_, ev| match ev {
            WindowEvent::DroppedFile(path) => {
                if let Some(ui) = weak.upgrade() {
                    if path.is_file() {
                        let dir = ST.with_borrow(|s| s.path.clone());
                        enqueue(
                            &tx,
                            CopyJob::Push {
                                local: path.clone(),
                                dir,
                            },
                        );
                    } else {
                        ui.set_notice(
                            tr(
                                &ui,
                                "Folder belum bisa dikirim: pilih file-nya.",
                                "Folders cannot be sent yet: drop the files.",
                            )
                            .into(),
                        );
                    }
                }
                EventResult::PreventDefault
            }
            _ => EventResult::Propagate,
        });
    }
    {
        let cancel = cancel.clone();
        ui.window().on_close_requested(move || {
            cancel.store(true, Ordering::Relaxed);
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        });
    }

    go(&ui, &a, "/sdcard".into());
    let timer = slint::Timer::default();
    {
        let weak = ui.as_weak();
        let mut dark_bar = false;
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(100),
            move || {
                if dark_bar {
                    return;
                }
                if let Some(ui) = weak.upgrade() {
                    if let Some(h) = crate::hwnd_of(ui.window()) {
                        crate::backdrop::dark_title_bar(h, dark);
                        dark_bar = true;
                    }
                }
            },
        );
    }
    ui.run()?;
    cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_round_trip() {
        let a = FileArgs {
            serial: "192.168.1.20:37099".into(),
            transport_id: Some(3),
            device: "Pixel 8".into(),
        };
        assert_eq!(FileArgs::parse(&a.to_args()), Some(a));
        assert_eq!(FileArgs::parse(&[ARG.into(), "serial=a b".into()]), None);
        assert_eq!(FileArgs::parse(&["--android-app".into()]), None);
        let b = FileArgs::parse(&[ARG.into(), "serial=X1".into()]).unwrap();
        assert_eq!(b.device, "X1");
    }

    #[test]
    fn sizes_and_dates() {
        assert_eq!(size_text(512), "512 B");
        assert_eq!(size_text(1536), "1.5 KB");
        assert_eq!(size_text(250 * 1024 * 1024), "250 MB");
        assert_eq!(date_text(0), "");
        assert_eq!(date_text(1_700_000_000), "2023-11-14");
        assert_eq!(date_text(951_782_400), "2000-02-29");
    }

    #[test]
    fn icons_by_kind() {
        let e = |n: &str, mode| Entry {
            name: n.into(),
            mode,
            size: 0,
            mtime: 0,
        };
        assert_eq!(icon_for(&e("DCIM", 0o040771)), "\u{E8B7}");
        assert_eq!(icon_for(&e("IMG_1.JPG", 0o100660)), "\u{EB9F}");
        assert_eq!(icon_for(&e("a.apk", 0o100660)), "\u{E7B8}");
        assert_eq!(icon_for(&e("noext", 0o100660)), "\u{E7C3}");
    }

    #[test]
    fn downloads_go_to_a_safe_folder() {
        let d = download_dir("Pixel: 8/Pro");
        assert!(d.ends_with("Pixel_ 8_Pro"));
        assert!(d.to_string_lossy().contains("ZenLink"));
    }
}
