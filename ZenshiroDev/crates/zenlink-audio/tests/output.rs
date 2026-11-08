//! Opens the real default output and plays silence (no audible sound).
//! Run on a PC with a sound device: `cargo test -p zenlink-audio -- --ignored`.

use std::time::{Duration, Instant};

#[test]
#[ignore = "needs a sound device"]
fn plays_silence_through_the_default_output() {
    let mut p = zenlink_audio::Player::start().expect("open the default output");
    p.set_muted(true);
    let packet = vec![0u8; 480 * 4]; // 10 ms of s16 stereo
    let start = Instant::now();
    let mut n = 0u32;
    while start.elapsed() < Duration::from_secs(2) {
        p.feed(&packet);
        n += 1;
        let due = start + Duration::from_millis(10) * n;
        if let Some(d) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(d);
        }
    }
    let s = p.stats();
    println!("{s:?}");
    assert!(s.running);
    assert!(s.output_ms > 0.0 && s.output_ms < 200.0);
    assert!(s.block_ms > 0.0, "the output pulled sound");
}
