//! The jitter buffer of ZenLink's native sound output, ported unchanged from
//! v2.2.31 `zenlink/audio_out.py` (`NativeAudio`). Its rules were measured
//! on real devices; keep them:
//!
//! - playback starts once one packet plus a margin is queued;
//! - an underrun adds 5 ms of margin (10 ms with a jitter floor), capped;
//!   30 s without one takes 1 ms off, never below 10 ms ([`MIN_MARGIN`]);
//! - anything queued beyond twice the target is dropped, so sound never trails;
//! - the output's own block size counts too: MME / Bluetooth outputs pull
//!   sound in large blocks, and a target smaller than one block ran dry on
//!   every pull (measured: gaps every few seconds at the 50 ms limit).
//!
//! Units are frames (one s16 sample per channel) at [`RATE`].

use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
/// Smallest margin the buffer shrinks to (10 ms).
pub const MIN_MARGIN: usize = 480;
/// Time without an underrun before the margin shrinks.
pub const CALM: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub struct JitterBuffer {
    buf: VecDeque<i16>,
    started: bool,
    /// Largest recent packet, frames.
    pub packet: usize,
    /// Current margin, frames.
    pub margin: usize,
    /// Minimum margin (Wi-Fi jitter), frames.
    pub floor: usize,
    calm_since: Instant,
    pub underruns: u64,
    pub dropped_frames: u64,
    /// Largest block the output asked for, frames.
    pub block: usize,
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self::new(Instant::now())
    }
}

impl JitterBuffer {
    pub fn new(now: Instant) -> Self {
        Self {
            buf: VecDeque::new(),
            started: false,
            packet: 0,
            margin: MIN_MARGIN,
            floor: 0,
            calm_since: now,
            underruns: 0,
            dropped_frames: 0,
            block: 0,
        }
    }

    /// Keep at least `ms` of sound queued (0 for a cable).
    pub fn set_jitter_ms(&mut self, ms: f32) {
        self.floor = (RATE as f32 * ms / 1000.0) as usize;
        self.margin = self.margin.max(self.floor);
    }

    /// Frames queued before playback (re)starts.
    pub fn target(&self) -> usize {
        let cap = 4800.max(self.floor + 2400).max(3 * self.block);
        cap.min(self.packet + self.margin.max(self.floor).max(self.block))
    }

    fn margin_cap(&self) -> usize {
        1440.max(self.floor + 2400).max(2 * self.block)
    }

    pub fn queued_frames(&self) -> usize {
        self.buf.len() / CHANNELS
    }

    /// Adds interleaved stereo samples from the device.
    pub fn feed(&mut self, samples: &[i16]) {
        let frames = samples.len() / CHANNELS;
        if frames == 0 {
            return;
        }
        let samples = &samples[..frames * CHANNELS];
        self.packet = frames.max((self.packet as f64 * 0.995) as usize);
        let target = self.target();
        let ceiling = target * 2 + self.block.max(128);
        let queued = self.queued_frames();
        if queued + frames > ceiling {
            let drop = queued.min(queued + frames - target);
            self.buf.drain(..drop * CHANNELS);
            self.dropped_frames += drop as u64;
        }
        self.buf.extend(samples);
    }

    /// Fills `out` (interleaved stereo) for the output. Returns `false` and
    /// leaves `out` untouched when it has to play silence instead.
    pub fn pull(&mut self, out: &mut [i16], now: Instant) -> bool {
        let frames = out.len() / CHANNELS;
        if frames > self.block {
            self.block = frames;
        }
        if !self.started && self.queued_frames() >= self.target() {
            self.started = true;
        }
        let played = if self.started && self.queued_frames() >= frames {
            for (o, s) in out.iter_mut().zip(self.buf.drain(..frames * CHANNELS)) {
                *o = s;
            }
            true
        } else {
            if self.started {
                self.started = false;
                self.underruns += 1;
                let step = if self.floor > 0 { 480 } else { 240 };
                self.margin = self.margin_cap().min(self.margin + step);
                self.calm_since = now;
            }
            false
        };
        if now.duration_since(self.calm_since) > CALM && self.margin > MIN_MARGIN.max(self.floor) {
            self.margin -= 48;
            self.calm_since = now;
        }
        played
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.started = false;
        self.block = 0;
    }
}

/// Frames to milliseconds.
pub fn frames_ms(frames: u64) -> f32 {
    frames as f32 * 1000.0 / RATE as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(frames: usize, v: i16) -> Vec<i16> {
        vec![v; frames * CHANNELS]
    }

    #[test]
    fn starts_after_one_packet_plus_margin() {
        let t = Instant::now();
        let mut b = JitterBuffer::new(t);
        let mut out = vec![0i16; 480 * CHANNELS];
        b.feed(&packet(480, 1));
        // target = packet 480 + margin 480 (block 0 until the first pull).
        assert!(!b.pull(&mut out, t), "960 frames needed first");
        b.feed(&packet(480, 2));
        assert!(b.pull(&mut out, t));
        assert_eq!(out[0], 1);
        assert_eq!(b.underruns, 0);
    }

    #[test]
    fn underrun_grows_margin_and_calm_shrinks_it() {
        let t = Instant::now();
        let mut b = JitterBuffer::new(t);
        let mut out = vec![0i16; 480 * CHANNELS];
        // One 960-frame packet + 480 margin = 1440 queued before playing.
        b.feed(&packet(960, 1));
        b.feed(&packet(480, 1));
        for _ in 0..3 {
            assert!(b.pull(&mut out, t));
        }
        assert!(!b.pull(&mut out, t), "ran dry");
        assert_eq!(b.underruns, 1);
        assert_eq!(b.margin, MIN_MARGIN + 240, "5 ms more");
        // 30 s later without an underrun: 1 ms less.
        b.pull(&mut out, t + CALM + Duration::from_millis(1));
        assert_eq!(b.margin, MIN_MARGIN + 240 - 48);
    }

    #[test]
    fn margin_never_below_minimum_and_capped() {
        let t = Instant::now();
        let mut b = JitterBuffer::new(t);
        let mut out = vec![0i16; 480 * CHANNELS];
        for _ in 0..20 {
            b.feed(&packet(2000, 1));
            while b.pull(&mut out, t) {}
        }
        assert_eq!(b.margin, 2400, "capped at 50 ms (floor + 2400)");
        let mut b = JitterBuffer::new(t);
        b.pull(&mut out, t + CALM * 3);
        assert_eq!(b.margin, MIN_MARGIN);
    }

    #[test]
    fn drops_what_trails_beyond_twice_the_target() {
        let t = Instant::now();
        let mut b = JitterBuffer::new(t);
        for _ in 0..10 {
            b.feed(&packet(480, 1));
        }
        // target 960, ceiling 2 * 960 + 128.
        assert!(b.queued_frames() <= 2 * 960 + 128);
        assert!(b.dropped_frames > 0);
    }

    #[test]
    fn large_output_blocks_raise_the_target() {
        // Bluetooth on MME pulls ~91 ms (4368 frames) at once.
        let t = Instant::now();
        let mut b = JitterBuffer::new(t);
        let mut out = vec![0i16; 4368 * CHANNELS];
        b.feed(&packet(480, 1));
        b.pull(&mut out, t);
        assert_eq!(b.block, 4368);
        assert_eq!(b.target(), 480 + 4368);
    }

    #[test]
    fn jitter_floor() {
        let mut b = JitterBuffer::default();
        b.set_jitter_ms(100.0);
        assert_eq!(b.floor, 4800);
        assert_eq!(b.margin, 4800);
    }
}
