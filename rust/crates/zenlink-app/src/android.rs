//! Android mirroring glue (phase A3): turns the scrcpy video stream into the
//! decoder records `zenlink-video` already plays, so Android and Apple share
//! one decoder, one renderer and the OBS-safe `VideoView`.
//!
//! - [`DeviceSource`]: a real device through the ADB server (scrcpy server).
//! - [`FixtureSource`]: a recorded scrcpy stream (`ZL_ANDROID_FIXTURE`), for
//!   tests without a device. It is paced by the recorded timestamps.
//!
//! Sound (phase A4): raw PCM from the scrcpy audio socket goes through the
//! v2.2.31 jitter buffer (`zenlink-audio`) to WASAPI. Low-latency mode:
//! sound plays as it arrives, the picture as it decodes (the Apple cable
//! path works the same way).

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zenlink_android::adb::{AdbClient, Target};
use zenlink_android::mirror::{
    codec_name, read_device_meta, Mirror, Packet, Session, Stopper, VideoOptions, VideoReader,
    CODEC_H264,
};
use zenlink_android::scrcpy::{SERVER_FILE, SERVER_SIZE};
use zenlink_core::api::{Codec, CodecInfo};
use zenlink_core::nal::{annexb_to_lengths, avcc_from_annexb};
use zenlink_core::source::{RecordStream, VideoSource};
use zenlink_core::stream::Record;

/// After this many failed starts in a row the session gives up and shows the error.
const MAX_FAILURES: u32 = 3;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Connecting,
    Streaming,
    Error,
}

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Connecting => "connecting",
            Phase::Streaming => "streaming",
            Phase::Error => "error",
        }
    }
}

/// What the window shows about the Android session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub phase: Phase,
    /// Name the scrcpy server reported (device meta), or the list's name.
    pub device: String,
    /// Last error, in English (logged; the UI shows its own bilingual text).
    pub error: String,
}

/// Shared between the decoder thread (source) and the window.
#[derive(Default)]
pub struct Shared {
    status: Mutex<Status>,
    stop: AtomicBool,
    stopper: Mutex<Option<Stopper>>,
    /// Sound latency (queue + output) in ms; 0 = no sound.
    audio_ms: AtomicU32,
}

/// Sound settings from the window (Settings › Audio, the mute button).
static AUDIO_TO_PC: AtomicBool = AtomicBool::new(true);
static AUDIO_MUTED: AtomicBool = AtomicBool::new(false);
/// f32 bits, 0.0..=1.0.
static AUDIO_VOLUME: AtomicU32 = AtomicU32::new(0x3f80_0000);

/// Called whenever the window pushes its sound settings. "Device only"
/// (`to_pc == false`) takes effect at the next Start: the server then does
/// not capture, so the phone keeps its own sound.
pub fn set_audio(to_pc: bool, volume: f32, muted: bool) {
    AUDIO_TO_PC.store(to_pc, Relaxed);
    AUDIO_VOLUME.store(volume.clamp(0.0, 1.0).to_bits(), Relaxed);
    AUDIO_MUTED.store(muted, Relaxed);
}

impl Shared {
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Sound latency in ms (queue + output), `None` without sound.
    pub fn audio_ms(&self) -> Option<u32> {
        Some(self.audio_ms.load(Relaxed)).filter(|&v| v > 0)
    }

    fn set(&self, f: impl FnOnce(&mut Status)) {
        f(&mut self.status.lock().unwrap());
    }

    /// Ends the session: the decoder's blocked read returns at once.
    pub fn stop(&self) {
        self.stop.store(true, Relaxed);
        if let Some(s) = self.stopper.lock().unwrap().take() {
            s.stop();
        }
        self.set(|s| s.phase = Phase::Idle);
    }
}

/// Converts scrcpy packets to decoder records. Packets are Annex B; the
/// decoder takes 4-byte-length samples and an avcC record.
fn to_record(p: Packet) -> Option<Record> {
    match p {
        // The size comes from the decoder; a rotation is followed by a new config.
        Packet::Session { .. } => None,
        Packet::Config(c) => Some(Record::Config(codec_info(&c)?)),
        Packet::Media { key, data, .. } => Some(Record::Frame {
            key,
            rebuild: false,
            data: annexb_to_lengths(&data),
        }),
    }
}

fn codec_info(config: &[u8]) -> Option<CodecInfo> {
    Some(CodecInfo {
        codec: Codec::H264,
        description: avcc_from_annexb(config)?,
        // scrcpy has no keyframe request in this phase; MediaCodec sends one every 10 s.
        keyframes_on_request: false,
    })
}

/// Reads up to the first config packet; media before it cannot be decoded.
fn first_config<R: Read>(v: &mut VideoReader<R>) -> io::Result<CodecInfo> {
    if v.codec != CODEC_H264 {
        return Err(io::Error::other(format!(
            "codec {} is not supported yet",
            codec_name(v.codec)
        )));
    }
    for _ in 0..64 {
        if let Packet::Config(c) = v.next_packet()? {
            return codec_info(&c)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no SPS/PPS"));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "no config packet",
    ))
}

/// Plays timestamps in real time (recordings only; a live device paces itself).
struct Pacer {
    start: Instant,
    pts0: Option<u64>,
}

impl Pacer {
    fn wait(&mut self, pts: u64, stop: &AtomicBool) {
        let pts0 = *self.pts0.get_or_insert(pts);
        let due = self.start + Duration::from_micros(pts.saturating_sub(pts0));
        while !stop.load(Relaxed) {
            let now = Instant::now();
            if now >= due {
                return;
            }
            std::thread::sleep((due - now).min(Duration::from_millis(20)));
        }
    }
}

struct Records<R> {
    video: VideoReader<R>,
    pacer: Option<Pacer>,
    shared: Arc<Shared>,
    /// Keeps the server (shell, sockets) alive while it streams.
    _session: Option<Session>,
}

impl<R: Read + Send> RecordStream for Records<R> {
    fn next_record(&mut self) -> Option<Record> {
        loop {
            if self.shared.stop.load(Relaxed) {
                return None;
            }
            let p = match self.video.next_packet() {
                Ok(p) => p,
                Err(e) => {
                    if !self.shared.stop.load(Relaxed) {
                        log::info!("android: stream ended: {e}");
                    }
                    return None;
                }
            };
            if let (Some(pacer), Packet::Media { pts, .. }) = (self.pacer.as_mut(), &p) {
                pacer.wait(*pts, &self.shared.stop);
            }
            if let Some(r) = to_record(p) {
                return Some(r);
            }
        }
    }
}

/// A real device: starts the scrcpy server through the ADB server.
pub struct DeviceSource {
    pub client: AdbClient,
    pub serial: String,
    pub transport_id: Option<u64>,
    pub server: Arc<Vec<u8>>,
    pub options: VideoOptions,
    pub shared: Arc<Shared>,
    failures: u32,
}

impl DeviceSource {
    /// `audio_supported`: Android 11+ (scrcpy captures sound from API 30).
    pub fn new(
        serial: String,
        transport_id: Option<u64>,
        name: String,
        server: Arc<Vec<u8>>,
        shared: Arc<Shared>,
        audio_supported: bool,
    ) -> Self {
        shared.set(|s| {
            *s = Status {
                phase: Phase::Connecting,
                device: name,
                error: String::new(),
            }
        });
        Self {
            client: AdbClient::default(),
            serial,
            transport_id,
            server,
            options: VideoOptions {
                audio: audio_supported && AUDIO_TO_PC.load(Relaxed),
                ..VideoOptions::default()
            },
            shared,
            failures: 0,
        }
    }

    fn fail(&mut self, e: String) {
        log::warn!("android: {e}");
        self.failures += 1;
        let gave_up = self.failures >= MAX_FAILURES;
        self.shared.set(|s| {
            s.error = e;
            if gave_up {
                s.phase = Phase::Error;
            }
        });
    }
}

impl VideoSource for DeviceSource {
    fn open(&mut self) -> Option<(CodecInfo, Box<dyn RecordStream>)> {
        if self.finished() {
            return None;
        }
        self.shared.set(|s| s.phase = Phase::Connecting);
        let target = match self.transport_id {
            Some(id) => Target::TransportId(id),
            None => Target::Serial(&self.serial),
        };
        let mut m = match Mirror::start(&self.client, &target, &self.server, &self.options) {
            Ok(m) => m,
            Err(e) => {
                self.fail(e.to_string());
                return None;
            }
        };
        if let Ok(s) = m.stopper() {
            *self.shared.stopper.lock().unwrap() = Some(s);
        }
        if self.shared.stop.load(Relaxed) {
            return None;
        }
        let info = match first_config(&mut m.video) {
            Ok(i) => i,
            Err(e) => {
                self.fail(e.to_string());
                return None;
            }
        };
        self.failures = 0;
        let name = m.device_name.clone();
        self.shared.set(|s| {
            s.phase = Phase::Streaming;
            if !name.is_empty() {
                s.device = name;
            }
            s.error.clear();
        });
        let Mirror {
            video,
            audio,
            session,
            ..
        } = m;
        if let Some(a) = audio {
            play_audio(a, self.shared.clone());
        }
        Some((
            info,
            Box::new(Records {
                video,
                pacer: None,
                shared: self.shared.clone(),
                _session: Some(session),
            }),
        ))
    }

    fn finished(&self) -> bool {
        self.shared.stop.load(Relaxed) || self.failures >= MAX_FAILURES
    }
}

/// Plays the audio socket until it closes (stop, unplug). Without a working
/// output the video goes on alone.
fn play_audio(mut r: VideoReader<std::net::TcpStream>, shared: Arc<Shared>) {
    let _ = std::thread::Builder::new()
        .name("android-audio".into())
        .spawn(move || {
            let mut player = match zenlink_audio::Player::start() {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("android: no sound output ({e}); video only");
                    return;
                }
            };
            let mut stats_at = Instant::now();
            while !shared.stop.load(Relaxed) {
                match r.next_packet() {
                    Ok(Packet::Media { data, .. }) => {
                        player.set_muted(AUDIO_MUTED.load(Relaxed));
                        player.set_volume(f32::from_bits(AUDIO_VOLUME.load(Relaxed)));
                        player.feed(&data);
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
                if stats_at.elapsed() >= Duration::from_secs(1) {
                    stats_at = Instant::now();
                    let s = player.stats();
                    let ms = (s.queue_ms + s.output_ms).round().max(1.0) as u32;
                    shared.audio_ms.store(ms, Relaxed);
                    if std::env::var_os("ZL_STATS_LOG").is_some() {
                        log::info!(
                            "sound stats queue={:.0} output={:.1} underruns={} dropped={:.0}",
                            s.queue_ms,
                            s.output_ms,
                            s.underruns,
                            s.dropped_ms
                        );
                    }
                }
            }
            shared.audio_ms.store(0, Relaxed);
        });
}

/// A recorded stream (`ZL_ANDROID_FIXTURE`), looped and paced by its PTS.
pub struct FixtureSource {
    pub path: PathBuf,
    pub shared: Arc<Shared>,
    data: Option<Arc<Vec<u8>>>,
}

impl FixtureSource {
    pub fn new(path: PathBuf, shared: Arc<Shared>) -> Self {
        shared.set(|s| {
            *s = Status {
                phase: Phase::Connecting,
                device: String::new(),
                error: String::new(),
            }
        });
        Self {
            path,
            shared,
            data: None,
        }
    }
}

/// `Read` over a shared buffer, so each loop of a recording does not copy it.
struct SharedCursor {
    data: Arc<Vec<u8>>,
    pos: usize,
}

impl Read for SharedCursor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let rest = &self.data[self.pos.min(self.data.len())..];
        let n = rest.len().min(buf.len());
        buf[..n].copy_from_slice(&rest[..n]);
        self.pos += n;
        Ok(n)
    }
}

impl VideoSource for FixtureSource {
    fn open(&mut self) -> Option<(CodecInfo, Box<dyn RecordStream>)> {
        if self.data.is_none() {
            match std::fs::read(&self.path) {
                Ok(d) => self.data = Some(Arc::new(d)),
                Err(e) => {
                    self.shared.set(|s| {
                        s.phase = Phase::Error;
                        s.error = format!("{}: {e}", self.path.display());
                    });
                    return None;
                }
            }
        }
        let mut r = SharedCursor {
            data: self.data.clone()?,
            pos: 0,
        };
        let opened = read_device_meta(&mut r).and_then(|name| {
            let mut v = VideoReader::new(r)?;
            let info = first_config(&mut v)?;
            Ok((name, v, info))
        });
        let (name, video, info) = match opened {
            Ok(x) => x,
            Err(e) => {
                self.shared.set(|s| {
                    s.phase = Phase::Error;
                    s.error = e.to_string();
                });
                return None;
            }
        };
        self.shared.set(|s| {
            s.phase = Phase::Streaming;
            s.device = name;
        });
        Some((
            info,
            Box::new(Records {
                video,
                pacer: Some(Pacer {
                    start: Instant::now(),
                    pts0: None,
                }),
                shared: self.shared.clone(),
                _session: None,
            }),
        ))
    }

    fn finished(&self) -> bool {
        self.shared.stop.load(Relaxed)
            || (self.data.is_none() && self.shared.status().phase == Phase::Error)
    }
}

/// The bundled scrcpy server: `ZL_SCRCPY_SERVER`, else `third_party/scrcpy/`
/// next to the exe or in a parent folder (source checkout). The size must
/// match the pinned release.
pub fn load_server() -> Result<Vec<u8>, String> {
    let mut candidates = Vec::new();
    if let Some(p) = std::env::var_os("ZL_SCRCPY_SERVER") {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1).take(5) {
            candidates.push(dir.join("third_party").join("scrcpy").join(SERVER_FILE));
            candidates.push(dir.join(SERVER_FILE));
        }
    }
    for p in &candidates {
        if let Ok(d) = std::fs::read(p) {
            if d.len() as u64 == SERVER_SIZE {
                return Ok(d);
            }
            log::warn!(
                "android: {} has the wrong size ({} bytes)",
                p.display(),
                d.len()
            );
        }
    }
    Err(format!("{SERVER_FILE} not found"))
}

/// The `adb` program to start the ADB server with, when none runs.
/// `ZL_ADB=none` disables it (tests); `ZL_ADB=<path>` picks one.
/// Otherwise: bundled `platform-tools` next to the exe, the Android SDK
/// (`ANDROID_HOME`, `ANDROID_SDK_ROOT`, Android Studio's default folder), PATH.
pub fn find_adb() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "adb.exe" } else { "adb" };
    if let Some(v) = std::env::var_os("ZL_ADB") {
        if v == "none" {
            return None;
        }
        return Some(PathBuf::from(v)).filter(|p| p.is_file());
    }
    let mut c: Vec<PathBuf> = Vec::new();
    if let Ok(me) = std::env::current_exe() {
        for dir in me.ancestors().skip(1).take(5) {
            c.push(dir.join("third_party").join("platform-tools").join(exe));
            c.push(dir.join("platform-tools").join(exe));
        }
    }
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(root) = std::env::var_os(var) {
            c.push(Path::new(&root).join("platform-tools").join(exe));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        c.push(
            Path::new(&local)
                .join("Android")
                .join("Sdk")
                .join("platform-tools")
                .join(exe),
        );
    }
    if let Some(path) = std::env::var_os("PATH") {
        c.extend(std::env::split_paths(&path).map(|d| d.join(exe)));
    }
    c.into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenlink_android::mirror::{write_device_meta, write_packet};

    fn wire(packets: &[Packet]) -> Vec<u8> {
        let mut w = Vec::new();
        write_device_meta(&mut w, "Test").unwrap();
        w.extend_from_slice(&CODEC_H264.to_be_bytes());
        for p in packets {
            write_packet(&mut w, p).unwrap();
        }
        w
    }

    const CONFIG: [u8; 13] = [0, 0, 0, 1, 0x67, 0x42, 0xc0, 0x1f, 0, 0, 1, 0x68, 0xce];

    #[test]
    fn packets_become_decoder_records() {
        assert!(to_record(Packet::Session {
            width: 1,
            height: 2,
            client_resized: false
        })
        .is_none());
        match to_record(Packet::Config(CONFIG.to_vec())) {
            Some(Record::Config(c)) => {
                assert_eq!(c.codec, Codec::H264);
                assert_eq!(&c.description[..4], &[1, 0x42, 0xc0, 0x1f]);
                assert!(!c.keyframes_on_request);
            }
            r => panic!("{r:?}"),
        }
        match to_record(Packet::Media {
            pts: 0,
            key: true,
            data: vec![0, 0, 0, 1, 0x65, 1, 2],
        }) {
            Some(Record::Frame { key, rebuild, data }) => {
                assert!(key && !rebuild);
                assert_eq!(data, [0, 0, 0, 3, 0x65, 1, 2]);
            }
            r => panic!("{r:?}"),
        }
    }

    #[test]
    fn fixture_source_plays_and_loops() {
        let path = std::env::temp_dir().join(format!("zl-fixture-{}.scrcpy", std::process::id()));
        std::fs::write(
            &path,
            wire(&[
                Packet::Session {
                    width: 16,
                    height: 16,
                    client_resized: false,
                },
                Packet::Config(CONFIG.to_vec()),
                Packet::Media {
                    pts: 0,
                    key: true,
                    data: vec![0, 0, 0, 1, 0x65, 9],
                },
                Packet::Media {
                    pts: 30_000,
                    key: false,
                    data: vec![0, 0, 0, 1, 0x41, 9],
                },
            ]),
        )
        .unwrap();
        let shared = Arc::new(Shared::default());
        let mut src = FixtureSource::new(path.clone(), shared.clone());
        for _ in 0..2 {
            let (info, mut s) = src.open().unwrap();
            assert_eq!(info.codec, Codec::H264);
            assert_eq!(shared.status().phase, Phase::Streaming);
            assert_eq!(shared.status().device, "Test");
            let t = Instant::now();
            assert!(matches!(
                s.next_record(),
                Some(Record::Frame { key: true, .. })
            ));
            assert!(matches!(
                s.next_record(),
                Some(Record::Frame { key: false, .. })
            ));
            assert!(t.elapsed() >= Duration::from_millis(25), "paced by PTS");
            assert!(s.next_record().is_none(), "end of recording");
        }
        shared.stop();
        assert!(src.finished());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn committed_fixture_is_a_valid_scrcpy_stream() {
        // tests/fixtures/android: made by rust/tools/make_android_fixture.py.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/android/synthetic-h264-360x640.scrcpy");
        let data = std::fs::read(&path).unwrap();
        let mut r = &data[..];
        assert_eq!(read_device_meta(&mut r).unwrap(), "Synthetic test stream");
        let mut v = VideoReader::new(r).unwrap();
        let info = first_config(&mut v).unwrap();
        assert_eq!(info.codec, Codec::H264);
        let (mut frames, mut keys, mut last) = (0, 0, 0);
        while let Ok(p) = v.next_packet() {
            if let Packet::Media { pts, key, .. } = p {
                assert!(frames == 0 || pts > last, "PTS increase");
                last = pts;
                frames += 1;
                keys += key as u32;
            }
        }
        assert_eq!(frames, 120);
        assert!(keys >= 1);
        assert!(data.len() < 150_000, "keep the fixture small");
    }

    #[test]
    #[ignore = "needs a sound device (plays silence, muted)"]
    fn audio_socket_plays_through_the_output() {
        use std::io::Write;
        use zenlink_android::mirror::CODEC_RAW;
        set_audio(true, 1.0, true);
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let device = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            s.write_all(&CODEC_RAW.to_be_bytes()).unwrap();
            // 1.5 s of 10 ms PCM packets, in real time like a device.
            for i in 0..150u64 {
                let mut w = Vec::new();
                write_packet(
                    &mut w,
                    &Packet::Media {
                        pts: i * 10_000,
                        key: false,
                        data: vec![0; 480 * 4],
                    },
                )
                .unwrap();
                s.write_all(&w).unwrap();
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let r =
            VideoReader::new(std::net::TcpStream::connect(("127.0.0.1", port)).unwrap()).unwrap();
        let shared = Arc::new(Shared::default());
        play_audio(r, shared.clone());
        std::thread::sleep(Duration::from_millis(1300));
        let ms = shared.audio_ms().expect("sound latency reported");
        println!("sound latency {ms} ms");
        assert!(ms < 150, "{ms} ms");
        device.join().unwrap();
    }

    #[test]
    fn missing_fixture_is_an_error() {
        let shared = Arc::new(Shared::default());
        let mut src = FixtureSource::new(PathBuf::from("Z:/does/not/exist.scrcpy"), shared.clone());
        assert!(src.open().is_none());
        assert_eq!(shared.status().phase, Phase::Error);
        assert!(src.finished());
    }

    #[test]
    fn other_codecs_are_refused_for_now() {
        let mut w = Vec::new();
        w.extend_from_slice(&zenlink_android::mirror::CODEC_H265.to_be_bytes());
        let mut v = VideoReader::new(&w[..]).unwrap();
        assert!(first_config(&mut v).is_err());
    }

    #[test]
    fn bundled_server_is_found_from_the_repo() {
        std::env::set_var(
            "ZL_SCRCPY_SERVER",
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../third_party/scrcpy")
                .join(SERVER_FILE),
        );
        assert_eq!(load_server().unwrap().len() as u64, SERVER_SIZE);
    }

    #[test]
    fn adb_can_be_disabled() {
        std::env::set_var("ZL_ADB", "none");
        assert!(find_adb().is_none());
        std::env::remove_var("ZL_ADB");
    }
}
