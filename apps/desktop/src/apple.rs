//! Apple Driver mode in Rust (v2.5.12, stage 1 of the Rust transition): the device list,
//! the tunnel, the Developer Disk Image and the screen, all in ZenLink.exe on `zenlink-apple`
//! (idevice). It replaces the Python backend's Apple session; ZenLink Studio still runs there.
//!
//! The window reads the same state JSON the backend sent (`snapshot`), so `on_state` and the
//! device list stay as they are: `main.rs` lays this state over the backend's while the
//! connection mode is Apple Driver.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use zenlink_apple::{link, mirror, models, wifi};
use zenlink_core::api::{Codec, CodecInfo};
use zenlink_core::nal::{hvcc_from_annexb, split_annexb};
use zenlink_core::source::{RecordStream, VideoSource};
use zenlink_core::stream::Record;

/// USB list refresh.
const USB_EVERY: Duration = Duration::from_secs(2);
/// Wi-Fi browse (each takes `WIFI_WAIT`).
const WIFI_EVERY: Duration = Duration::from_secs(12);
const WIFI_WAIT: Duration = Duration::from_secs(4);
/// A Wi-Fi device not seen for this long leaves the list.
const WIFI_FORGET: Duration = Duration::from_secs(45);

#[derive(Clone, Debug)]
struct Dev {
    udid: String,
    /// Wi-Fi address (RemotePairing); `None` = USB.
    addr: Option<SocketAddr>,
    /// USB: this PC has a pair record (Trust done). Wi-Fi: always (the advert matched one).
    paired: bool,
    seen: Instant,
}

#[derive(Default)]
struct State {
    phase: &'static str,
    detail: String,
    error: Option<String>,
    /// The device the session runs on.
    device: Option<(String, bool)>,
    /// Clockwise degrees that show the picture upright (SpringBoard orientation).
    rotation: u32,
}

pub struct Engine {
    rt: tokio::runtime::Runtime,
    devices: Mutex<Vec<Dev>>,
    info: Mutex<HashMap<String, link::Info>>,
    state: Mutex<State>,
    /// Set to end the running session.
    stop: Mutex<Option<Arc<AtomicBool>>>,
    /// Access units of the running session, taken by the decoder's source.
    frames: Mutex<Option<Receiver<Vec<u8>>>>,
    watching: AtomicBool,
}

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();

/// The connection mode is Apple Driver: this engine serves the Apple workspace.
static IN_USE: AtomicBool = AtomicBool::new(false);
/// From the backend's state: the device picked in the list, and Wi-Fi (wireless) vs USB.
static PICKED: Mutex<Option<String>> = Mutex::new(None);
static WIRELESS: AtomicBool = AtomicBool::new(false);

pub fn in_use() -> bool {
    IN_USE.load(Relaxed)
}

/// Lays the engine's state over the backend's while the mode is Apple Driver.
/// The backend keeps the mode, the picked device, the saved devices and the drivers.
pub fn overlay(st: &mut Value) {
    let conn = |k: &str| {
        st.pointer(&format!("/connection/{k}"))
            .and_then(Value::as_str)
    };
    let apple = conn("active") == Some("apple");
    WIRELESS.store(conn("link") == Some("wireless"), Relaxed);
    *PICKED.lock().unwrap() = st
        .get("selected")
        .and_then(Value::as_str)
        .map(str::to_string);
    if IN_USE.swap(apple, Relaxed) && !apple {
        engine().stop(); // the mode changed away from Apple Driver
    }
    if !apple {
        return;
    }
    if let (Some(dst), Value::Object(src)) = (st.as_object_mut(), engine().snapshot()) {
        dst.extend(src);
    }
}

/// Start in the Apple workspace (the window's Start button).
pub fn start() {
    let picked = PICKED.lock().unwrap().clone();
    engine().start(picked, WIRELESS.load(Relaxed));
}

pub fn engine() -> &'static Arc<Engine> {
    ENGINE.get_or_init(|| {
        Arc::new(Engine {
            rt: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("zenlink-apple")
                .enable_all()
                .build()
                .expect("tokio runtime"),
            devices: Mutex::new(Vec::new()),
            info: Mutex::new(load_info()),
            state: Mutex::new(State {
                phase: "idle",
                ..State::default()
            }),
            stop: Mutex::new(None),
            frames: Mutex::new(None),
            watching: AtomicBool::new(false),
        })
    })
}

fn info_file() -> Option<std::path::PathBuf> {
    zenlink_core::local_data_dir().map(|d| d.join("apple-devices.json"))
}

/// Names learned over USB, so a Wi-Fi-only device keeps its name.
fn load_info() -> HashMap<String, link::Info> {
    let Some(v) = info_file()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    else {
        return HashMap::new();
    };
    let s = |d: &Value, k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    v.as_object()
        .into_iter()
        .flatten()
        .map(|(udid, d)| {
            (
                udid.clone(),
                link::Info {
                    name: s(d, "name"),
                    product_type: s(d, "productType"),
                    version: s(d, "version"),
                    device_class: s(d, "deviceClass"),
                },
            )
        })
        .collect()
}

fn save_info(all: &HashMap<String, link::Info>) {
    let v: serde_json::Map<String, Value> = all
        .iter()
        .map(|(u, i)| {
            (
                u.clone(),
                json!({ "name": i.name, "productType": i.product_type,
                        "version": i.version, "deviceClass": i.device_class }),
            )
        })
        .collect();
    if let Some(p) = info_file() {
        let _ = std::fs::write(
            p,
            serde_json::to_vec_pretty(&Value::Object(v)).unwrap_or_default(),
        );
    }
}

/// A name the user gave the device in ZenLink (the backend's devices.json).
/// shortcut: reads %LOCALAPPDATA%\ZenLink-Next only, not a portable build's folder; move the
/// rename feature to Rust in stage 3.
fn custom_name(udid: &str) -> Option<String> {
    let p = zenlink_core::local_data_dir()?.join("devices.json");
    let v: Value = serde_json::from_slice(&std::fs::read(p).ok()?).ok()?;
    v.get(udid.replace('-', ""))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl Engine {
    fn set(&self, phase: &'static str, detail: &str, error: Option<String>) {
        let mut s = self.state.lock().unwrap();
        s.phase = phase;
        s.detail = detail.to_string();
        s.error = error;
    }

    fn watch(self: &Arc<Self>) {
        if self.watching.swap(true, Relaxed) {
            return;
        }
        let me = self.clone();
        self.rt.spawn(async move {
            let mut wifi_at = Instant::now() - WIFI_EVERY;
            loop {
                me.refresh_usb().await;
                if wifi_at.elapsed() >= WIFI_EVERY {
                    wifi_at = Instant::now();
                    let me2 = me.clone();
                    tokio::spawn(async move { me2.refresh_wifi().await });
                }
                tokio::time::sleep(USB_EVERY).await;
            }
        });
    }

    async fn refresh_usb(&self) {
        let list = link::usb_devices().await.unwrap_or_default();
        let mut usb = Vec::new();
        for (udid, paired) in list {
            if paired && !self.info.lock().unwrap().contains_key(&udid) {
                if let Ok(i) = link::info(&udid).await {
                    let mut all = self.info.lock().unwrap();
                    all.insert(udid.clone(), i);
                    save_info(&all);
                }
            }
            usb.push(Dev {
                udid,
                addr: None,
                paired,
                seen: Instant::now(),
            });
        }
        let mut devs = self.devices.lock().unwrap();
        devs.retain(|d| d.addr.is_some());
        devs.extend(usb);
    }

    async fn refresh_wifi(&self) {
        let Some(dir) = wifi::record_dir() else {
            return;
        };
        let records = wifi::records(&dir);
        if records.is_empty() {
            return;
        }
        let found = wifi::browse(&records, WIFI_WAIT).await.unwrap_or_default();
        let mut devs = self.devices.lock().unwrap();
        for f in found {
            devs.retain(|d| !(d.addr.is_some() && d.udid == f.udid));
            devs.push(Dev {
                udid: f.udid,
                addr: Some(f.addr),
                paired: true,
                seen: Instant::now(),
            });
        }
        devs.retain(|d| d.addr.is_none() || d.seen.elapsed() < WIFI_FORGET);
    }

    fn entry(&self, d: &Dev) -> Value {
        let info = self
            .info
            .lock()
            .unwrap()
            .get(&d.udid)
            .cloned()
            .unwrap_or_default();
        let family = match info.device_class.as_str() {
            "iPad" | "iPhone" => info.device_class.as_str(),
            _ => "iOS device",
        };
        let model = custom_name(&d.udid)
            .or_else(|| models::product_name(&info.product_type).map(str::to_string))
            .unwrap_or_else(|| models::series_name(&d.udid, family));
        json!({
            "id": d.udid,
            "name": model,
            "deviceName": (!info.name.is_empty()).then_some(info.name),
            "os": (!info.version.is_empty()).then_some(info.version),
            "ready": d.paired,
            "wifi": d.addr.is_some(),
            "need": if d.paired { Value::Null } else { "trust".into() },
        })
    }

    /// State for the window, in the backend's shape. USB entries come before Wi-Fi ones;
    /// a device on both is listed twice (the window keeps the first).
    pub fn snapshot(self: &Arc<Self>) -> Value {
        self.watch();
        let mut devs = self.devices.lock().unwrap().clone();
        devs.sort_by_key(|d| d.addr.is_some());
        let devices: Vec<Value> = devs.iter().map(|d| self.entry(d)).collect();
        let s = self.state.lock().unwrap();
        let live = matches!(s.phase, "connecting" | "activating" | "streaming");
        json!({
            "phase": s.phase,
            "detail": s.detail,
            "error": s.error,
            "devices": devices,
            "device": s.device.as_ref().map(|(n, w)| json!({ "name": n, "wifi": w })),
            "capture": live.then(|| json!({ "backend": "apple", "codec": "HEVC",
                                            "source": "CoreDevice DisplayService" })),
            "rotation": s.rotation,
        })
    }

    /// Starts the picked device (when listed), else the first ready one; USB before Wi-Fi
    /// unless `wireless`.
    pub fn start(self: &Arc<Self>, picked: Option<String>, wireless: bool) {
        self.stop();
        let devs = self.devices.lock().unwrap().clone();
        let ready = |d: &&Dev| d.paired;
        let on_link = |d: &&Dev| d.addr.is_some() == wireless;
        let dev = picked
            .as_ref()
            .and_then(|p| {
                devs.iter()
                    .filter(|d| &d.udid == p)
                    .filter(ready)
                    .min_by_key(|d| d.addr.is_some() != wireless)
            })
            .or_else(|| devs.iter().filter(ready).find(on_link))
            .or_else(|| devs.iter().find(ready))
            .cloned();
        let Some(dev) = dev else {
            let msg = if devs.is_empty() {
                "No iPhone or iPad found. Connect it with a cable, unlock it and tap Trust."
            } else {
                "Unlock the device and tap Trust, then press Start again."
            };
            self.set("error", "", Some(msg.into()));
            return;
        };
        let name = self
            .entry(&dev)
            .get("deviceName")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "iPhone/iPad".into());
        let stop = Arc::new(AtomicBool::new(false));
        *self.stop.lock().unwrap() = Some(stop.clone());
        let (tx, rx) = sync_channel(120);
        *self.frames.lock().unwrap() = Some(rx);
        self.state.lock().unwrap().device = Some((name, dev.addr.is_some()));
        self.set(
            "connecting",
            if dev.addr.is_some() {
                "Opening a secure Wi-Fi tunnel to the device"
            } else {
                "Opening the USB tunnel to the device"
            },
            None,
        );
        let me = self.clone();
        // A thread of its own: idevice's display-service futures are not `Send`.
        let spawned = std::thread::Builder::new()
            .name("zenlink-apple-session".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => return me.set("error", "", Some(format!("runtime: {e}"))),
                };
                let r = rt.block_on(me.session(&dev, &stop, tx));
                if stop.load(Relaxed) {
                    return; // stopped by the user: stop() already set idle
                }
                match r {
                    Ok(()) => me.set("idle", "", None),
                    Err(e) => {
                        log::warn!("apple: {e}");
                        me.set("error", "", Some(e));
                    }
                }
            });
        if let Err(e) = spawned {
            self.set("error", "", Some(format!("thread: {e}")));
        }
    }

    async fn session(
        &self,
        dev: &Dev,
        stop: &AtomicBool,
        tx: SyncSender<Vec<u8>>,
    ) -> Result<(), String> {
        let version = self
            .info
            .lock()
            .unwrap()
            .get(&dev.udid)
            .map(|i| i.version.clone())
            .unwrap_or_default();
        let mut l = match dev.addr {
            Some(addr) => {
                let dir = wifi::record_dir().ok_or("no user folder")?;
                let r = wifi::records(&dir)
                    .into_iter()
                    .find(|r| r.udid == dev.udid)
                    .ok_or("the Wi-Fi pairing record is gone: connect the cable once")?;
                link::Link::wifi(addr, &r).await?
            }
            None => link::Link::usb(&dev.udid).await?,
        };
        if !l.display_offered() {
            self.set(
                "activating",
                "Preparing the device (first time downloads a small image)",
                None,
            );
            l.ensure_ddi(&version, |_| {}).await?;
        }
        let watch_rot = zenlink_apple::watch::orientation(l.adapter.clone(), l.rsd.clone(), |r| {
            self.state.lock().unwrap().rotation = r;
        });
        let watch_lock = zenlink_apple::watch::lock(l.adapter.clone(), l.rsd.clone(), || {
            log::info!("apple: device locked");
        });
        let mut first = true;
        let stream = mirror::stream(&mut l, |au| {
            if stop.load(Relaxed) {
                return false;
            }
            if first {
                first = false;
                self.set("streaming", "", None);
            }
            // A full queue means the decoder stalls: drop the frame, the next keyframe heals it.
            let _ = tx.try_send(au);
            true
        });
        // The watchers never end; the session ends with the stream.
        tokio::select! {
            r = stream => r,
            _ = watch_rot => Ok(()),
            _ = watch_lock => Ok(()),
        }
    }

    pub fn stop(&self) {
        if let Some(s) = self.stop.lock().unwrap().take() {
            s.store(true, Relaxed);
        }
        *self.frames.lock().unwrap() = None;
        let mut st = self.state.lock().unwrap();
        st.phase = "idle";
        st.detail.clear();
        st.error = None;
        st.device = None;
        st.rotation = 0;
    }

    pub fn source(self: &Arc<Self>) -> Box<dyn VideoSource> {
        Box::new(AppleSource {
            engine: self.clone(),
        })
    }
}

/// The decoder's view of the Rust session.
struct AppleSource {
    engine: Arc<Engine>,
}

impl VideoSource for AppleSource {
    fn open(&mut self) -> Option<(CodecInfo, Box<dyn RecordStream>)> {
        let rx = self.engine.frames.lock().unwrap().take()?;
        // The stream opens with VPS/SPS/PPS; frames before them cannot be decoded.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let au = match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(au) => au,
                Err(_) => return None,
            };
            if let Some(info) = codec_info(&au) {
                let pending = frame(&au);
                return Some((
                    info.clone(),
                    Box::new(AppleRecords {
                        rx: Some(rx),
                        engine: self.engine.clone(),
                        config: info.description,
                        pending,
                    }),
                ));
            }
        }
    }

    fn finished(&self) -> bool {
        self.engine.stop.lock().unwrap().is_none()
    }
}

struct AppleRecords {
    /// Handed back to the engine on drop, so the decoder can reopen after a decode error.
    rx: Option<Receiver<Vec<u8>>>,
    engine: Arc<Engine>,
    config: Vec<u8>,
    pending: Option<Record>,
}

impl RecordStream for AppleRecords {
    fn next_record(&mut self) -> Option<Record> {
        if let Some(r) = self.pending.take() {
            return Some(r);
        }
        loop {
            let au = match self.rx.as_ref()?.recv_timeout(Duration::from_secs(1)) {
                Ok(au) => au,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return None,
            };
            // New parameter sets (rotation, size change): reconfigure first.
            if let Some(info) = codec_info(&au).filter(|i| i.description != self.config) {
                self.config = info.description.clone();
                self.pending = frame(&au);
                return Some(Record::Config(info));
            }
            if let Some(f) = frame(&au) {
                return Some(f);
            }
        }
    }
}

impl Drop for AppleRecords {
    fn drop(&mut self) {
        let mut slot = self.engine.frames.lock().unwrap();
        // Only for the same session: stop() or a new start() already replaced it.
        if slot.is_none() && self.engine.stop.lock().unwrap().is_some() {
            *slot = self.rx.take();
        }
    }
}

fn nal_type(n: &[u8]) -> u8 {
    (n[0] >> 1) & 0x3f
}

fn codec_info(au: &[u8]) -> Option<CodecInfo> {
    Some(CodecInfo {
        codec: Codec::Hevc,
        description: hvcc_from_annexb(au)?,
        // The display service has no keyframe request; it sends one when the picture changes size.
        keyframes_on_request: false,
    })
}

/// One decoder sample: the picture NAL units with 4-byte lengths; VPS/SPS/PPS go in the config.
fn frame(au: &[u8]) -> Option<Record> {
    let mut data = Vec::with_capacity(au.len());
    let mut key = false;
    for n in split_annexb(au).into_iter().filter(|n| n.len() > 2) {
        let t = nal_type(n);
        if (32..=34).contains(&t) {
            continue;
        }
        key |= (16..=21).contains(&t);
        data.extend_from_slice(&(n.len() as u32).to_be_bytes());
        data.extend_from_slice(n);
    }
    (!data.is_empty()).then_some(Record::Frame {
        key,
        rebuild: false,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NAL header bytes for type `t` (layer 0, temporal id 1).
    fn nal(t: u8, body: &[u8]) -> Vec<u8> {
        let mut v = vec![0, 0, 0, 1, t << 1, 1];
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn frame_drops_parameter_sets_and_marks_keyframes() {
        let mut au = nal(32, &[1, 2, 3]);
        au.extend(nal(33, &[4; 16]));
        au.extend(nal(34, &[5, 6]));
        au.extend(nal(19, &[7, 8, 9])); // IDR_W_RADL
        let Some(Record::Frame { key, data, .. }) = frame(&au) else {
            panic!("no frame")
        };
        assert!(key);
        assert_eq!(data, [&[0, 0, 0, 5][..], &[19 << 1, 1, 7, 8, 9]].concat());
        assert!(codec_info(&au).is_some());

        let Some(Record::Frame { key, .. }) = frame(&nal(1, &[1, 2])) else {
            panic!("no frame")
        };
        assert!(!key);
        assert!(codec_info(&nal(1, &[1, 2])).is_none());
        assert!(frame(&nal(32, &[1, 2])).is_none());
    }
}
