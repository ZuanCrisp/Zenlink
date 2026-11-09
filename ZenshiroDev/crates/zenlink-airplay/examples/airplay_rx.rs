//! Runs the receiver and counts what arrives; pair it with ZenshiroDev/tooling/dev/airplay_sim.py:
//!
//!   cargo run -p zenlink-airplay --example airplay_rx -- 7101 25
//!   python ZenshiroDev\tooling\dev\airplay_sim.py --port 7101 --seconds 10 --audio
//!
//! Args: port (default 7101), seconds to run (default 30), `adv` to advertise over mDNS.

use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use zenlink_airplay::{server, Sink, State};

#[derive(Default)]
struct Count {
    frames: AtomicUsize,
    bytes: AtomicUsize,
    keyframes: AtomicUsize,
    audio: AtomicUsize,
    states: Mutex<Vec<State>>,
}

impl Sink for Count {
    fn state(&self, s: State) {
        println!("state: {s:?}");
        self.states.lock().unwrap().push(s);
    }
    fn config(&self, d: Vec<u8>, hevc: bool, w: u32, h: u32) {
        println!("config: {} bytes, hevc {hevc}, {w}x{h}", d.len());
    }
    fn frame(&self, au: Vec<u8>) {
        self.frames.fetch_add(1, Relaxed);
        self.bytes.fetch_add(au.len(), Relaxed);
        if au.get(4).is_some_and(|b| b & 0x1F == 5) {
            self.keyframes.fetch_add(1, Relaxed);
        }
    }
    fn audio(&self, _au: Vec<u8>) {
        self.audio.fetch_add(1, Relaxed);
    }
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let port = args.next().and_then(|a| a.parse().ok()).unwrap_or(7101);
    let secs = args.next().and_then(|a| a.parse().ok()).unwrap_or(30);
    let dll = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/windows/native/playfair/playfair.dll");
    zenlink_airplay::fairplay::load(&dll).unwrap();
    let sink = Arc::new(Count::default());
    let cfg = server::Config {
        name: "ZenLinkTest".into(),
        hevc: false,
        data_dir: std::env::temp_dir(),
        port,
        advertise: args.next().as_deref() == Some("adv"),
    };
    let s = server::Server::start(cfg, sink.clone()).await.unwrap();
    println!("listening on {}", s.port);
    tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
    println!(
        "frames {} ({} keyframes), {} bytes, {} sound units, states {:?}",
        sink.frames.load(Relaxed),
        sink.keyframes.load(Relaxed),
        sink.bytes.load(Relaxed),
        sink.audio.load(Relaxed),
        sink.states.lock().unwrap()
    );
}
