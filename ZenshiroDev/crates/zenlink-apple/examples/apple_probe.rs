//! Apple Driver prototype on idevice: checks one iPhone / iPad end to end.
//!
//! cargo run -p zenlink-apple --example apple_probe -- [--udid X] [--mount] [--stream SECONDS] [--out FILE]
//!
//! Read-only by default: device, iOS, Developer Mode, DDI state, USB tunnel, display service.
//! --mount installs the Developer Disk Image when missing (same order as pymobiledevice3:
//! already there? then Apple's TSS). --stream receives the screen for SECONDS and reports fps.
//! The screen-stream offer follows idevice's tools/src/coredevice_stream.rs (MIT, Jackson Coxson).

use std::time::{Duration, Instant};

use idevice::{
    core_device::{
        build_screen_audio_offer, build_screen_video_offer, build_start_audio_parameters,
        build_start_video_parameters, CallInfoBlob, DisplayServiceClient, HevcDepacketizer,
        RtpPacket,
    },
    core_device_proxy::CoreDeviceProxy,
    cryptexd::{install_ddi, installed_ddi, Cryptex1Assets},
    lockdown::LockdownClient,
    mobile_image_mounter::ImageMounter,
    provider::IdeviceProvider,
    rsd::RsdHandshake,
    usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdConnection},
    IdeviceService, RsdService,
};
use zenlink_apple::ddi::{self, Kind};

const DISPLAY_SERVICE: &str = "com.apple.coredevice.displayservice";
/// What the controller supports (Device Hub sends 140), not the device's mask.
const CLIENT_SUPPORTED_FEATURES: u64 = 140;

struct Args {
    udid: Option<String>,
    mount: bool,
    stream: Option<u64>,
    out: Option<String>,
}

fn args() -> Args {
    let mut a = Args {
        udid: None,
        mount: false,
        stream: None,
        out: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "--udid" => a.udid = it.next(),
            "--mount" => a.mount = true,
            "--stream" => a.stream = it.next().and_then(|v| v.parse().ok()),
            "--out" => a.out = it.next(),
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    a
}

macro_rules! step {
    ($($t:tt)*) => {{ println!("[{:>6.2}s] {}", START.with(|s| s.elapsed().as_secs_f32()), format!($($t)*)); }};
}
thread_local! { static START: Instant = Instant::now(); }

#[tokio::main]
async fn main() {
    if let Err(e) = run(args()).await {
        step!("FAILED: {e}");
        std::process::exit(1);
    }
    step!("done");
}

async fn run(a: Args) -> Result<(), String> {
    // 1. Device over Apple's usbmuxd (Apple Mobile Device Service).
    let mut mux = UsbmuxdConnection::default()
        .await
        .map_err(|e| format!("usbmuxd (Apple Devices / iTunes driver running?): {e:?}"))?;
    let devs = mux
        .get_devices()
        .await
        .map_err(|e| format!("device list: {e:?}"))?;
    for d in &devs {
        step!("device {} via {:?}", d.udid, d.connection_type);
    }
    let dev = match &a.udid {
        Some(u) => devs.iter().find(|d| &d.udid == u),
        None => devs
            .iter()
            .find(|d| d.connection_type == Connection::Usb)
            .or(devs.first()),
    }
    .ok_or("no device connected")?;
    let addr = UsbmuxdAddr::from_env_var().map_err(|e| format!("usbmuxd address: {e:?}"))?;
    let provider = dev.to_provider(addr, "zenlink-probe");

    // 2. Lockdown: iOS version and names.
    let mut lockdown = LockdownClient::connect(&provider)
        .await
        .map_err(|e| format!("lockdown: {e:?}"))?;
    let pairing = provider
        .get_pairing_file()
        .await
        .map_err(|e| format!("pairing record (Trust this computer first): {e:?}"))?;
    lockdown
        .start_session(&pairing)
        .await
        .map_err(|e| format!("lockdown session: {e:?}"))?;
    let value = |v: Result<plist::Value, _>| -> String {
        v.ok()
            .and_then(|v: plist::Value| v.as_string().map(str::to_owned))
            .unwrap_or_default()
    };
    let version = value(lockdown.get_value(Some("ProductVersion"), None).await);
    let product = value(lockdown.get_value(Some("ProductType"), None).await);
    let name = value(lockdown.get_value(Some("DeviceName"), None).await);
    step!("{name} ({product}), iOS {version}");
    let kind = Kind::for_version(&version).ok_or("iOS 17 or later is needed")?;
    step!("DDI kind: {kind:?}");

    // 3. Developer Mode and the image mounter view (Personalized DMG only).
    let mut mounter = ImageMounter::connect(&provider)
        .await
        .map_err(|e| format!("image mounter: {e:?}"))?;
    match mounter.query_developer_mode_status().await {
        Ok(on) => step!("Developer Mode: {}", if on { "on" } else { "OFF" }),
        Err(e) => step!("Developer Mode: unknown ({e:?})"),
    }
    let mounted_dmg = mounter
        .copy_devices()
        .await
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    step!(
        "image mounter lists {} image(s)",
        if mounted_dmg { "some" } else { "no" }
    );

    // 4. USB tunnel (iOS 17.4+, in-process TCP: no wintun) and RSD.
    let proxy = CoreDeviceProxy::connect(&provider)
        .await
        .map_err(|e| format!("CoreDeviceProxy: {e:?}"))?;
    let rsd_port = proxy.tunnel_info().server_rsd_port;
    let mut adapter = proxy
        .create_software_tunnel()
        .map_err(|e| format!("software tunnel: {e:?}"))?
        .to_async_handle();
    let stream = adapter
        .connect(rsd_port)
        .await
        .map_err(|e| format!("RSD connect: {e:?}"))?;
    let mut handshake = RsdHandshake::new(stream)
        .await
        .map_err(|e| format!("RSD handshake: {e:?}"))?;
    step!("tunnel up: {} RSD services", handshake.services.len());

    // 5. DDI: installed? otherwise install with --mount.
    let has_ddi = match kind {
        Kind::Cryptex => installed_ddi(&mut adapter, &mut handshake)
            .await
            .map_err(|e| format!("cryptexd copy-installed: {e:?}"))?
            .inspect(|c| step!("DDI cryptex installed: {} {}", c.identifier, c.version))
            .is_some(),
        Kind::Personalized => mounted_dmg,
    };
    let display_ready = handshake.services.contains_key(DISPLAY_SERVICE);
    step!("DDI present: {has_ddi}, display service offered: {display_ready}");

    if !display_ready && !has_ddi {
        if !a.mount {
            return Err("no Developer Disk Image: run again with --mount".into());
        }
        let dirs = ddi::search_dirs(kind);
        let dir = ddi::find(kind, &dirs).ok_or_else(|| {
            format!("DDI files not found in {dirs:?} (download from doronz88/DeveloperDiskImage)")
        })?;
        step!("installing the {kind:?} DDI from {}", dir.display());
        let f = ddi::read(kind, &dir).map_err(|e| format!("read DDI: {e}"))?;
        match kind {
            Kind::Cryptex => {
                let manifest: plist::Dictionary = plist::from_bytes(&f.build_manifest)
                    .map_err(|e| format!("BuildManifest: {e}"))?;
                let identity = idevice::tss::select_cryptex_build_identity(&manifest)
                    .map_err(|e| format!("build identity: {e:?}"))?
                    .clone();
                let assets = Cryptex1Assets::from_parts(
                    f.image,
                    f.trustcache,
                    f.cryptex_info,
                    f.root_hash,
                    identity,
                );
                let c = install_ddi(&mut adapter, &mut handshake, &assets)
                    .await
                    .map_err(|e| format!("cryptex install (TSS + cryptexd): {e:?}"))?;
                step!("installed {} {}", c.identifier, c.version);
            }
            Kind::Personalized => {
                let ecid = lockdown
                    .get_value(Some("UniqueChipID"), None)
                    .await
                    .ok()
                    .and_then(|v| v.as_unsigned_integer())
                    .ok_or("UniqueChipID")?;
                mounter
                    .mount_personalized(
                        &provider,
                        f.image,
                        f.trustcache,
                        &f.build_manifest,
                        None,
                        ecid,
                    )
                    .await
                    .map_err(|e| format!("personalized mount: {e:?}"))?;
                step!("personalized DDI mounted");
            }
        }
        // The display service appears in a fresh RSD handshake after the install.
        let stream = adapter
            .connect(rsd_port)
            .await
            .map_err(|e| format!("RSD reconnect: {e:?}"))?;
        handshake = RsdHandshake::new(stream)
            .await
            .map_err(|e| format!("RSD handshake: {e:?}"))?;
        step!(
            "display service offered now: {}",
            handshake.services.contains_key(DISPLAY_SERVICE)
        );
    }

    // 6. Screen stream: HEVC over RTP through the tunnel.
    let Some(secs) = a.stream else { return Ok(()) };
    let mut client = DisplayServiceClient::connect_rsd(&mut adapter, &mut handshake)
        .await
        .map_err(|e| format!("display service: {e:?}"))?;
    let audio_udp = adapter
        .bind_udp(0)
        .await
        .map_err(|e| format!("udp: {e:?}"))?;
    let video_udp = adapter
        .bind_udp(0)
        .await
        .map_err(|e| format!("udp: {e:?}"))?;
    let host = adapter.host_ip().to_string();
    let peer = adapter.peer_ip().to_string();
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
    let audio_offer =
        build_screen_audio_offer(&uuid::Uuid::new_v4().to_string().to_uppercase(), &call_info)
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
    let video_offer = build_screen_video_offer(
        &uuid::Uuid::new_v4().to_string().to_uppercase(),
        &call_info,
        uuid::Uuid::new_v4().as_u128() as u32,
    )
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
    step!("streaming for {secs}s (move something on the screen)");

    let mut out = a
        .out
        .as_ref()
        .map(|p| std::fs::File::create(p).expect("create --out"));
    let mut depack = HevcDepacketizer::new();
    let (mut packets, mut frames, mut bytes) = (0usize, 0usize, 0usize);
    let mut first_frame: Option<Instant> = None;
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        let left = end.saturating_duration_since(Instant::now());
        let Ok(Ok(dg)) = tokio::time::timeout(left, video_udp.recv()).await else {
            break;
        };
        let Some(pkt) = RtpPacket::parse(&dg.data) else {
            continue;
        };
        packets += 1;
        if pkt.marker {
            frames += 1;
            first_frame.get_or_insert_with(Instant::now);
        }
        depack.push(pkt.sequence_number, pkt.timestamp, pkt.payload);
        let nal = depack.take_output();
        bytes += nal.len();
        if let Some(f) = out.as_mut() {
            use std::io::Write;
            let _ = f.write_all(&nal);
        }
    }
    let _ = client.stop_media_stream().await;
    let fps = first_frame
        .map(|t| frames as f32 / t.elapsed().as_secs_f32().max(0.001))
        .unwrap_or(0.0);
    step!(
        "{packets} RTP packets, {frames} frames (~{fps:.1} fps), {:.1} MB HEVC, parameter sets: {}",
        bytes as f32 / 1e6,
        depack.has_parameter_sets()
    );
    if frames == 0 {
        return Err("no video frames received".into());
    }
    Ok(())
}
