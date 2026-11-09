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
    let result = loop {
        let dg = match tokio::time::timeout(SILENCE, video_udp.recv()).await {
            Ok(Ok(dg)) => dg,
            Ok(Err(e)) => break Err(format!("video: {e:?}")),
            Err(_) => break Err("the device stopped sending the screen".into()),
        };
        let Some(pkt) = RtpPacket::parse(&dg.data) else {
            continue;
        };
        depack.push(pkt.sequence_number, pkt.timestamp, pkt.payload);
        au.extend_from_slice(&depack.take_output());
        if pkt.marker && !au.is_empty() && !on_frame(std::mem::take(&mut au)) {
            break Ok(());
        }
    };
    let _ = client.stop_media_stream().await;
    drop(audio_udp);
    result
}
