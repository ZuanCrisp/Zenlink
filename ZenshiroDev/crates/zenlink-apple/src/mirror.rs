//! The screen over CoreDevice's display service: HEVC over RTP through the link, handed out
//! as Annex B access units. The offer follows idevice's tools/src/coredevice_stream.rs
//! (MIT, Jackson Coxson) and what Device Hub sends.

use std::time::Duration;

use idevice::core_device::{
    build_screen_audio_offer, build_screen_video_offer, build_start_audio_parameters,
    build_start_video_parameters, CallInfoBlob, DisplayServiceClient, HevcDepacketizer, RtpPacket,
};
use idevice::RsdService as _;

use crate::link::Link;

/// What the controller supports (Device Hub sends 140), not the device's mask.
const CLIENT_SUPPORTED_FEATURES: u64 = 140;
/// No packet for this long: the device stopped sending (locked, cable out, Wi-Fi gone).
const SILENCE: Duration = Duration::from_secs(8);

/// One RTP packet: sequence number, timestamp, payload, marker.
type Held = (u16, u32, Vec<u8>, bool);

/// Packets in a row a new SSRC needs before it replaces the running stream.
const SWITCH_AFTER: usize = 30;

/// Picks the screen's RTP stream by SSRC. The video port also gets a lone packet about once a
/// second from another SSRC (seq 12, a new SSRC each time; seen on iPadOS 27). Fed to idevice's
/// depacketizer it can move the cursor to seq 12, after which every video packet is "behind"
/// and dropped: the picture freezes while packets keep coming.
#[derive(Default)]
struct Picker {
    main: Option<u32>,
    /// Packets of SSRCs not (yet) accepted, kept so the first keyframe is not lost.
    held: std::collections::HashMap<u32, Vec<Held>>,
}

impl Picker {
    /// `None`: ignore the packet (it was held). `Some(packets)`: feed these, in order; after a
    /// switch to a new stream they are its held packets and the depacketizer must start fresh.
    fn push(&mut self, ssrc: u32, p: Held) -> Option<(bool, Vec<Held>)> {
        if self.main == Some(ssrc) {
            return Some((false, vec![p]));
        }
        let need = if self.main.is_some() { SWITCH_AFTER } else { 2 };
        let h = self.held.entry(ssrc).or_default();
        h.push(p);
        if h.len() >= need {
            let h = self.held.remove(&ssrc).unwrap_or_default();
            self.held.clear();
            self.main = Some(ssrc);
            return Some((true, h));
        }
        if self.held.len() > 8 {
            self.held.retain(|_, v| v.len() > 1); // the lone packets
        }
        None
    }
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

/// Streams until `on_frame` returns `false` or the stream ends. Each call of `on_frame` gets
/// one access unit in Annex B (the first ones carry VPS/SPS/PPS).
///
/// shortcut: the audio stream is negotiated (the device needs it) but its packets are dropped;
/// sound in Rust is a later step.
pub async fn stream(
    link: &mut Link,
    mut on_frame: impl FnMut(Vec<u8>) -> bool,
) -> Result<(), String> {
    let mut client = DisplayServiceClient::connect_rsd(&mut link.adapter, &mut link.rsd)
        .await
        .map_err(|e| format!("display service: {e:?}"))?;
    let udp = |e| format!("udp: {e:?}");
    let audio_udp = link.adapter.bind_udp(0).await.map_err(udp)?;
    let video_udp = link.adapter.bind_udp(0).await.map_err(udp)?;
    let host = link.adapter.host_ip().to_string();
    let peer = link.adapter.peer_ip().to_string();
    let call_info = CallInfoBlob {
        call_id: 0,
        client_version: 1,
        device_type: "Mac17,7".into(),
        framework_version: "2205.3.1".into(),
        os_version: "25F71".into(),
        device_name: None,
        audio_device_uid: None,
    };
    let session = uuid::Uuid::new_v4();
    let audio_offer = build_screen_audio_offer(&new_id(), &call_info)
        .map_err(|e| format!("audio offer: {e:?}"))?;
    client
        .start_media_stream(build_start_audio_parameters(
            &host,
            audio_udp.local_port(),
            &peer,
            50000,
            audio_offer,
            CLIENT_SUPPORTED_FEATURES,
            session,
        ))
        .await
        .map_err(|e| format!("start audio: {e:?}"))?;
    let video_offer =
        build_screen_video_offer(&new_id(), &call_info, uuid::Uuid::new_v4().as_u128() as u32)
            .map_err(|e| format!("video offer: {e:?}"))?;
    client
        .start_media_stream(build_start_video_parameters(
            &host,
            video_udp.local_port(),
            &peer,
            50001,
            video_offer,
            CLIENT_SUPPORTED_FEATURES,
            1,
            session,
        ))
        .await
        .map_err(|e| format!("start video: {e:?}"))?;

    let mut depack = HevcDepacketizer::new();
    let mut au = Vec::new();
    let mut picker = Picker::default();
    let mut frame_at = std::time::Instant::now();
    let result = loop {
        let dg = match tokio::time::timeout(SILENCE, video_udp.recv()).await {
            Ok(Ok(dg)) => dg,
            Ok(Err(e)) => break Err(format!("video: {e:?}")),
            Err(_) => break Err("the device stopped sending the screen".into()),
        };
        let Some(pkt) = RtpPacket::parse(&dg.data) else {
            continue;
        };
        let held = (pkt.sequence_number, pkt.timestamp, pkt.payload.to_vec(), pkt.marker);
        let Some((fresh, packets)) = picker.push(pkt.ssrc, held) else {
            continue;
        };
        if fresh {
            log::info!("screen: RTP stream {:08x}", pkt.ssrc);
            depack = HevcDepacketizer::new();
            au.clear();
        }
        let mut done = false;
        for (seq, ts, payload, marker) in packets {
            depack.push(seq, ts, &payload);
            au.extend_from_slice(&depack.take_output());
            if marker && !au.is_empty() {
                frame_at = std::time::Instant::now();
                if !on_frame(std::mem::take(&mut au)) {
                    done = true;
                    break;
                }
            }
        }
        if done {
            break Ok(());
        }
        if frame_at.elapsed() > Duration::from_secs(3) {
            // Packets but no picture: say what arrives, once per stall.
            frame_at = std::time::Instant::now();
            log::info!(
                "screen: packets but no frame for 3 s (ssrc {:08x}, seq {}, pt {})",
                pkt.ssrc,
                pkt.sequence_number,
                pkt.payload_type
            );
        }
    };
    let _ = client.stop_media_stream().await;
    drop(audio_udp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(seq: u16) -> Held {
        (seq, 0, vec![], false)
    }

    #[test]
    fn lone_packets_of_other_ssrcs_are_ignored() {
        let mut k = Picker::default();
        assert!(k.push(7, p(100)).is_none()); // held, not lost
        assert!(k.push(50, p(12)).is_none()); // the lone packet
        let (fresh, got) = k.push(7, p(101)).unwrap();
        assert!(fresh);
        assert_eq!(got.iter().map(|h| h.0).collect::<Vec<_>>(), [100, 101]);
        assert_eq!(k.push(7, p(102)).unwrap(), (false, vec![p(102)]));
        for i in 0..20 {
            assert!(k.push(60 + i, p(12)).is_none()); // one per second, new SSRC each
        }
        assert!(k.held.len() <= 9);
        // A real new stream takes over after SWITCH_AFTER packets in a row.
        for s in 0..SWITCH_AFTER as u16 - 1 {
            assert!(k.push(9, p(s)).is_none());
        }
        let (fresh, got) = k.push(9, p(99)).unwrap();
        assert!(fresh && got.len() == SWITCH_AFTER);
    }
}
