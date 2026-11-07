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
//! Smooth mode ([`JitterBuffer::set_smooth`], Android): sound comes over ADB
//! in bursts next to the video and the phone's clock drifts from the PC's.
//! Instead of dropping whole chunks (heard as cracks) the buffer plays a
//! little faster or slower (at most 0.5%, below hearing) until the queue is
//! back at its target; a hard drop only happens after a long stall.
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
    /// Rate-adjusted playback instead of chunk drops (Android).
    smooth: bool,
    /// Fractional read position (smooth mode), frames.
    pos: f64,
    /// Queue length averaged over about half a second (smooth mode), frames.
    avg: f64,
}

/// Largest speed change in smooth mode (0.5%: about 8 cents of pitch).
pub const MAX_SKEW: f64 = 0.005;

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
            smooth: false,
            pos: 0.0,
            avg: 0.0,
        }
    }

    /// Smooth mode (see the module notes).
    pub fn set_smooth(&mut self, on: bool) {
        self.smooth = on;
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
        // Smooth mode corrects slowly, so it only cuts after a real stall
        // (150 ms above the target).
        let ceiling = if self.smooth {
            target + 7200
        } else {
            target * 2 + self.block.max(128)
        };
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
        let queued = self.queued_frames();
        if self.smooth && self.started {
            self.avg = if self.avg == 0.0 {
                queued as f64
            } else {
                self.avg * 0.98 + queued as f64 * 0.02
            };
        }
        let ratio = if self.smooth {
            let err = (self.avg - self.target() as f64) / RATE as f64;
            1.0 + (err * 0.25).clamp(-MAX_SKEW, MAX_SKEW)
        } else {
            1.0
        };
        let need = if self.smooth {
            (self.pos + frames as f64 * ratio).ceil() as usize + 1
        } else {
            frames
        };
        let played = if self.started && queued >= need {
            if self.smooth {
                self.resample(out, ratio);
            } else {
                for (o, s) in out.iter_mut().zip(self.buf.drain(..frames * CHANNELS)) {
                    *o = s;
                }
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
        self.pos = 0.0;
        self.avg = 0.0;
    }

    /// Fills `out` reading `ratio` input frames per output frame (linear
    /// interpolation), then drops the frames it consumed.
    fn resample(&mut self, out: &mut [i16], ratio: f64) {
        let mut pos = self.pos;
        for frame in out.chunks_exact_mut(CHANNELS) {
            let i = pos as usize;
            let f = (pos - i as f64) as f32;
            for (c, o) in frame.iter_mut().enumerate() {
                let a = self.buf[i * CHANNELS + c] as f32;
                let b = self.buf[(i + 1) * CHANNELS + c] as f32;
                *o = (a + (b - a) * f).round() as i16;
            }
            pos += ratio;
        }
        let used = pos as usize;
        self.buf.drain(..used * CHANNELS);
        self.pos = pos - used as f64;
    }
}

/// Frames to milliseconds.
pub fn frames_ms(frames: u64) -> f32 {
    frames as f32 * 1000.0 / RATE as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulates `secs` of a phone whose clock runs `drift` fast, sending
    /// 20 ms packets with up to `burst` packets held back and released at
    /// once (ADB next to the video), into 10 ms output pulls.
    fn run_phone(secs: u32, drift: f64, burst: u32, smooth: bool) -> JitterBuffer {
        let t0 = Instant::now();
        let mut b = JitterBuffer::new(t0);
        b.set_smooth(smooth);
        b.set_jitter_ms(if smooth { 40.0 } else { 0.0 });
        let packet = vec![1000i16; 960 * CHANNELS];
        let mut out = vec![0i16; 480 * CHANNELS];
        let (mut sent, mut held) = (0.0f64, 0u32);
        for step in 0..(secs * 100) {
            let now = t0 + Duration::from_millis(step as u64 * 10);
            // The phone produced this much by now (its clock is `drift` fast).
            let due = step as f64 * 0.01 * (1.0 + drift) / 0.02;
            while sent + 1.0 <= due {
                sent += 1.0;
                held += 1;
                // Packets are released `burst` at a time.
                if held >= burst.max(1) {
                    for _ in 0..held {
                        b.feed(&packet);
                    }
                    held = 0;
                }
            }
            b.pull(&mut out, now);
        }
        b
    }

    #[test]
    fn smooth_mode_absorbs_drift_and_bursts_without_cuts() {
        for drift in [0.003, -0.003, 0.0] {
            let b = run_phone(120, drift, 3, true);
            // The first start may wait; after that no gaps and no drops.
            assert!(b.underruns <= 1, "drift {drift}: {} gaps", b.underruns);
            assert_eq!(b.dropped_frames, 0, "drift {drift}");
            let q = frames_ms(b.queued_frames() as u64);
            assert!(q < 150.0, "drift {drift}: queue {q} ms");
        }
    }

    #[test]
    fn plain_mode_cracks_under_the_same_load() {
        // The cause seen in the field (gaps and dropped chunks every few s).
        let b = run_phone(120, 0.003, 3, false);
        assert!(b.underruns + b.dropped_frames > 0);
    }

    #[test]
    fn plain_mode_is_unchanged_by_default() {
        let b = JitterBuffer::new(Instant::now());
        assert!(!b.smooth);
    }

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
