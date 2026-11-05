use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

/// Counters shared by the decode and render threads. Everything here is
/// measured, nothing is estimated.
#[derive(Default)]
pub struct Stats {
    pub decoded: AtomicU64,
    pub presented: AtomicU64,
    /// Decoded frames replaced by a newer one before they were shown.
    pub replaced: AtomicU64,
    pub width: AtomicU32,
    pub height: AtomicU32,
    pub hardware: AtomicBool,
    /// Time from a sample entering the decoder to its picture being ready, last frame (µs).
    pub decode_us: AtomicU32,
    pub bytes: AtomicU64,
    pub errors: AtomicU32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StatsSnapshot {
    pub decoded_fps: f32,
    pub presented_fps: f32,
    pub replaced: u64,
    pub width: u32,
    pub height: u32,
    pub hardware: bool,
    pub decode_ms: f32,
    pub mbps: f32,
}

pub struct RateMeter {
    at: Instant,
    decoded: u64,
    presented: u64,
    bytes: u64,
}

impl RateMeter {
    pub fn new(s: &Stats) -> Self {
        Self { at: Instant::now(), decoded: s.decoded.load(Relaxed), presented: s.presented.load(Relaxed), bytes: s.bytes.load(Relaxed) }
    }

    pub fn sample(&mut self, s: &Stats) -> StatsSnapshot {
        let dt = self.at.elapsed().as_secs_f32().max(1e-3);
        let (d, p, b) = (s.decoded.load(Relaxed), s.presented.load(Relaxed), s.bytes.load(Relaxed));
        let snap = StatsSnapshot {
            decoded_fps: (d - self.decoded) as f32 / dt,
            presented_fps: (p - self.presented) as f32 / dt,
            replaced: s.replaced.load(Relaxed),
            width: s.width.load(Relaxed),
            height: s.height.load(Relaxed),
            hardware: s.hardware.load(Relaxed),
            decode_ms: s.decode_us.load(Relaxed) as f32 / 1000.0,
            mbps: (b - self.bytes) as f32 * 8.0 / dt / 1e6,
        };
        *self = Self { at: Instant::now(), decoded: d, presented: p, bytes: b };
        snap
    }
}
