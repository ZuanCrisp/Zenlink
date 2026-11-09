//! The sound: RTP over UDP, each payload AES-CBC encrypted (whole blocks, the same IV every
//! packet), AAC-ELD 44.1 kHz stereo. Port of receiver.py `_Reorder` / `_AudioDecoder`.

use std::collections::HashMap;

use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes128;
use tokio::net::UdpSocket;

use crate::Sink;

/// The stream's AudioSpecificConfig: AAC-ELD 44.1 kHz stereo, 480-sample frames (receiver.py
/// AAC_ELD_44K with frameLengthFlag set).
pub const ASC: [u8; 4] = [0xF8, 0xE8, 0x50, 0x00];
pub const RATE: i32 = 44100;

/// Wi-Fi delivers RTP slightly out of order: release it in sequence, holding at most `DEPTH`
/// packets (~10 ms each) before skipping a lost one.
const DEPTH: usize = 6;

#[derive(Default)]
struct Reorder {
    next: Option<u16>,
    pending: HashMap<u16, Vec<u8>>,
}

impl Reorder {
    fn push(&mut self, seq: u16, packet: Vec<u8>) -> Vec<Vec<u8>> {
        let next = *self.next.get_or_insert(seq);
        if seq.wrapping_sub(next) > 0x8000 {
            return Vec::new(); // older than what was already played
        }
        self.pending.insert(seq, packet);
        let mut out = Vec::new();
        loop {
            let n = self.next.unwrap();
            if let Some(p) = self.pending.remove(&n) {
                out.push(p);
            } else if self.pending.len() <= DEPTH {
                return out;
            }
            self.next = Some(n.wrapping_add(1));
        }
    }
}

/// Decrypts the whole 16-byte blocks of `payload` in place; the tail stays clear.
fn decrypt(cipher: &Aes128, iv: &[u8; 16], payload: &mut [u8]) {
    let mut prev = *iv;
    for block in payload.chunks_exact_mut(16) {
        let c: [u8; 16] = block.try_into().unwrap();
        cipher.decrypt_block(block.into());
        block.iter_mut().zip(prev).for_each(|(b, p)| *b ^= p);
        prev = c;
    }
}

/// First bytes of an AAC-ELD access unit; anything else means a wrong key. iOS sends the
/// receiver.py set; 0x5A is FFmpeg's encoder (the simulator's fixture).
fn looks_like_eld(b: u8) -> bool {
    matches!(b, 0x8C | 0x8D | 0x8E | 0x80 | 0x81 | 0x82 | 0x20 | 0x5A)
}

/// Reads the data socket until the task is aborted, handing access units to the sink.
pub async fn run(data: UdpSocket, key: [u8; 16], iv: [u8; 16], ct: u64, sink: &dyn Sink) {
    if ct != 8 {
        log::warn!("airplay: audio type ct={ct} not supported (only AAC-ELD)");
    }
    let cipher = Aes128::new(&key.into());
    let mut order = Reorder::default();
    let (mut last, mut decoded, mut failed) = (None, 0u64, 0u64);
    let mut buf = [0u8; 2048];
    while let Ok(n) = data.recv(&mut buf).await {
        if n < 12 || ct != 8 {
            continue;
        }
        let seq = u16::from_be_bytes([buf[2], buf[3]]);
        for packet in order.push(seq, buf[..n].to_vec()) {
            let seq = u16::from_be_bytes([packet[2], packet[3]]);
            if last.replace(seq) == Some(seq) {
                continue;
            }
            let mut payload = packet[12..].to_vec();
            decrypt(&cipher, &iv, &mut payload);
            if !payload.first().is_some_and(|&b| looks_like_eld(b)) {
                failed += 1;
                if matches!(failed, 1 | 50 | 500) {
                    log::warn!("airplay: audio packet not AAC-ELD after decryption ({failed} so far)");
                }
                continue;
            }
            if decoded == 0 {
                log::info!("airplay: sound arrives (AAC-ELD 44.1 kHz)");
            }
            decoded += 1;
            sink.audio(payload);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;

    #[test]
    fn cbc_matches_pycryptodome() {
        // Scratch: AES.new(0..15, MODE_CBC, 16..31).encrypt(p[:32]) + p[32:], p = 8c 01..27.
        let mut c = hex("3efbe32a44b2dff306a188b949a98be94a290ca281882feeeff354b2ce5bcef72021222324252627");
        let key: [u8; 16] = std::array::from_fn(|i| i as u8);
        let iv: [u8; 16] = std::array::from_fn(|i| 16 + i as u8);
        decrypt(&Aes128::new(&key.into()), &iv, &mut c);
        let want: Vec<u8> = std::iter::once(0x8C).chain(1..40).collect();
        assert_eq!(c, want);
    }

    #[test]
    fn reorder_releases_in_sequence_and_skips_losses() {
        let mut r = Reorder::default();
        let p = |s: u16| vec![s as u8];
        assert_eq!(r.push(10, p(10)), vec![p(10)]);
        assert!(r.push(12, p(12)).is_empty());
        assert_eq!(r.push(11, p(11)), vec![p(11), p(12)]);
        assert!(r.push(9, p(9)).is_empty()); // late
        for s in 14..20 {
            assert!(r.push(s, p(s)).is_empty()); // 13 lost
        }
        // The 7th waiting packet gives up on 13 and releases the rest.
        assert_eq!(r.push(20, p(20)), (14..=20).map(p).collect::<Vec<_>>());
    }
}
