//! The Android device list: ADB entries + properties -> named, identified devices.
//!
//! One phone connected over USB and Wi-Fi at the same time shows once, with
//! both transports; USB is preferred for mirroring (lower latency, no pairing).

use crate::adb::{AdbClient, Result, Target};
use crate::devices::{parse_list, AdbDevice, Connection, DeviceState};
use crate::identity::{identify, DeviceId, Strength};
use crate::naming::{self, DeviceName, FormFactor};
use crate::props::Props;
use crate::scrcpy::{MIN_SDK, MIN_SDK_AUDIO};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    /// Accept the "Allow USB debugging?" prompt on the device.
    NeedsAuthorization,
    Offline,
    /// The PC cannot open the device (driver or permission).
    NoPermission,
    /// Recovery, bootloader, sideload.
    NotAndroidSystem,
    /// Android older than scrcpy supports.
    TooOld {
        sdk: u32,
    },
    /// Authorized, but properties could not be read yet.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndroidDevice {
    pub id: DeviceId,
    pub name: DeviceName,
    /// The ADB entry used for mirroring (USB first).
    pub primary: AdbDevice,
    /// Every transport this device is connected through right now.
    pub transports: Vec<AdbDevice>,
    pub manufacturer: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub android_release: Option<String>,
    pub sdk: Option<u32>,
    pub form_factor: FormFactor,
    pub readiness: Readiness,
    /// Audio capture needs Android 11 (API 30).
    pub audio_supported: bool,
}

impl AndroidDevice {
    pub fn target(&self) -> Target<'_> {
        match self.primary.transport_id {
            Some(id) => Target::TransportId(id),
            None => Target::Serial(&self.primary.serial),
        }
    }
}

fn rank(d: &AdbDevice) -> u8 {
    let auth = if d.state.is_authorized() { 0 } else { 10 };
    let kind = match d.connection {
        Connection::Usb => 0,
        Connection::WifiTls { .. } => 1,
        Connection::WifiTcp { .. } => 2,
        Connection::Emulator => 3,
        Connection::Unknown => 4,
    };
    auth + kind
}

fn readiness(dev: &AdbDevice, props: Option<&Props>, sdk: Option<u32>) -> Readiness {
    match &dev.state {
        DeviceState::Device => match (props, sdk) {
            (None, _) => Readiness::Unknown,
            (Some(_), Some(s)) if s < MIN_SDK => Readiness::TooOld { sdk: s },
            (Some(_), _) => Readiness::Ready,
        },
        DeviceState::Unauthorized | DeviceState::Authorizing => Readiness::NeedsAuthorization,
        DeviceState::Offline | DeviceState::Connecting => Readiness::Offline,
        DeviceState::NoPermissions => Readiness::NoPermission,
        DeviceState::NotAndroidSystem(_) => Readiness::NotAndroidSystem,
        DeviceState::Other(_) => Readiness::Offline,
    }
}

fn describe(dev: AdbDevice, props: Option<Props>) -> AndroidDevice {
    let id = identify(&dev, props.as_ref());
    let get = |k: &str| {
        props
            .as_ref()
            .and_then(|p| p.get(k))
            .and_then(naming::clean)
    };
    let sdk = get("ro.build.version.sdk").and_then(|s| s.parse().ok());
    let name = match (&props, dev.state.is_authorized()) {
        (Some(p), true) => naming::resolve(p),
        (None, true) => dev
            .model
            .as_deref()
            .and_then(naming::from_adb_model)
            .unwrap_or_else(|| naming::placeholder(Some(&dev.serial))),
        _ => naming::placeholder(Some(&dev.serial)),
    };
    AndroidDevice {
        readiness: readiness(&dev, props.as_ref(), sdk),
        audio_supported: sdk.is_some_and(|s| s >= MIN_SDK_AUDIO),
        form_factor: props
            .as_ref()
            .map(naming::form_factor)
            .unwrap_or(FormFactor::Unknown),
        manufacturer: get("ro.product.manufacturer"),
        brand: props.as_ref().and_then(naming::brand),
        model: get("ro.product.model"),
        android_release: get("ro.build.version.release"),
        sdk,
        name,
        id,
        transports: vec![dev.clone()],
        primary: dev,
    }
}

/// Builds the list. `fetch` reads properties; it is called only for
/// authorized entries (never for unauthorized or offline ones).
pub fn build(
    entries: Vec<AdbDevice>,
    mut fetch: impl FnMut(&AdbDevice) -> Option<Props>,
) -> Vec<AndroidDevice> {
    let mut out: Vec<AndroidDevice> = Vec::new();
    for dev in entries {
        let props = if dev.state.is_authorized() {
            fetch(&dev)
        } else {
            None
        };
        let d = describe(dev, props);
        let same = (d.id.strength == Strength::Strong)
            .then(|| out.iter_mut().find(|o| o.id == d.id))
            .flatten();
        match same {
            Some(existing) => {
                let mut transports = std::mem::take(&mut existing.transports);
                transports.extend(d.transports.iter().cloned());
                transports.sort_by_key(rank);
                // Keep the better-described entry (authorized USB beats Wi-Fi or unauthorized).
                if rank(&d.primary) < rank(&existing.primary) {
                    *existing = d;
                }
                existing.transports = transports;
            }
            None => out.push(d),
        }
    }
    out
}

/// Reads the device list from the ADB server and the properties of each
/// authorized device.
pub fn scan(client: &AdbClient) -> Result<Vec<AndroidDevice>> {
    let entries = parse_list(&client.devices_l()?);
    Ok(build(entries, |d| {
        let target = match d.transport_id {
            Some(id) => Target::TransportId(id),
            None => Target::Serial(&d.serial),
        };
        client.getprop(&target).ok()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adb::fake::{frame, FakeAdb, Reply};
    use crate::naming::NameSource;
    use std::collections::HashMap;

    // Synthetic fixture (apps/desktop/tests/fixtures/android holds the recorded stream).
    const LIST: &str = "List of devices attached\n\
        R5CX10ABCDE device usb:1-4 product:a57x model:SM_A576B device:a57x transport_id:3\n\
        0B261FDF6000AB unauthorized usb:1-2 transport_id:4\n\
        192.168.1.40:5555 device product:a57x model:SM_A576B device:a57x transport_id:5\n\
        R5CX10ZZZZZ device usb:1-5 product:a57x model:SM_A576B device:a57x transport_id:6\n\
        adb-0F1E2D3C4B-aB12cD._adb-tls-connect._tcp device product:LH8n model:TECNO_LH8n device:TECNO-LH8n transport_id:7\n\
        emulator-5554 offline transport_id:8\n";

    fn samsung(sn: &str) -> String {
        format!(
            "[ro.product.manufacturer]: [samsung]\n[ro.product.brand]: [samsung]\n[ro.product.model]: [SM-A576B]\n\
             [ro.build.version.release]: [15]\n[ro.build.version.sdk]: [35]\n[ro.build.characteristics]: [phone]\n\
             [ro.serialno]: [{sn}]\n"
        )
    }

    fn tecno() -> String {
        "[ro.product.manufacturer]: [TECNO]\n[ro.product.brand]: [TECNO]\n[ro.product.model]: [TECNO LH8n]\n\
         [ro.product.marketname]: [TECNO POVA 6]\n[ro.build.version.release]: [10]\n[ro.build.version.sdk]: [29]\n\
         [ro.serialno]: [0F1E2D3C4B]\n"
            .to_string()
    }

    fn fake() -> FakeAdb {
        let mut s: HashMap<String, Reply> =
            HashMap::from([("host:devices-l".to_string(), frame(LIST))]);
        for (tid, props) in [
            (3, samsung("R5CX10ABCDE")),
            (5, samsung("R5CX10ABCDE")),
            (6, samsung("R5CX10ZZZZZ")),
            (7, tecno()),
        ] {
            s.insert(format!("host:transport-id:{tid}"), Reply::Okay);
            s.insert(
                format!("shell:getprop@{tid}"),
                Reply::Stream(props.into_bytes()),
            );
        }
        FakeAdb::start(s)
    }

    /// `build` with props taken from the fixture by transport id.
    fn scan_list() -> Vec<AndroidDevice> {
        let props: HashMap<u64, String> = HashMap::from([
            (3, samsung("R5CX10ABCDE")),
            (5, samsung("R5CX10ABCDE")),
            (6, samsung("R5CX10ZZZZZ")),
            (7, tecno()),
        ]);
        build(parse_list(LIST), |d| {
            d.transport_id
                .and_then(|t| props.get(&t))
                .map(|p| Props::parse(p))
        })
    }

    #[test]
    fn usb_and_wifi_of_one_phone_merge() {
        let list = scan_list();
        let a = list
            .iter()
            .find(|d| d.id.key == "android/R5CX10ABCDE")
            .unwrap();
        assert_eq!(a.transports.len(), 2);
        assert_eq!(a.primary.connection, Connection::Usb, "USB preferred");
        assert!(matches!(
            a.transports[1].connection,
            Connection::WifiTcp { .. }
        ));
        assert_eq!(a.name.display, "Samsung SM-A576B");
        assert_eq!(
            (a.sdk, a.android_release.as_deref()),
            (Some(35), Some("15"))
        );
        assert_eq!(a.readiness, Readiness::Ready);
        assert!(a.audio_supported);
        assert_eq!(a.form_factor, FormFactor::Phone);
        assert_eq!(a.target(), Target::TransportId(3));
    }

    #[test]
    fn identical_models_stay_separate() {
        let list = scan_list();
        let same_model: Vec<_> = list
            .iter()
            .filter(|d| d.model.as_deref() == Some("SM-A576B"))
            .collect();
        assert_eq!(same_model.len(), 2);
        assert_ne!(same_model[0].id, same_model[1].id);
        assert_eq!(list.len(), 5, "6 entries, one phone on two transports");
    }

    #[test]
    fn unauthorized_offline_and_wireless_tls() {
        let list = scan_list();
        let u = list
            .iter()
            .find(|d| d.primary.serial == "0B261FDF6000AB")
            .unwrap();
        assert_eq!(u.readiness, Readiness::NeedsAuthorization);
        assert_eq!(u.name.source, NameSource::Placeholder);
        assert_eq!(u.model, None);
        let e = list
            .iter()
            .find(|d| d.primary.connection == Connection::Emulator)
            .unwrap();
        assert_eq!(e.readiness, Readiness::Offline);
        let t = list
            .iter()
            .find(|d| d.id.key == "android/0F1E2D3C4B")
            .unwrap();
        assert_eq!(t.name.display, "TECNO POVA 6");
        assert!(!t.audio_supported, "Android 10 has no audio capture");
        assert_eq!(t.readiness, Readiness::Ready);
    }

    #[test]
    fn props_never_read_for_unauthorized() {
        let mut asked = Vec::new();
        build(parse_list(LIST), |d| {
            asked.push(d.serial.clone());
            None
        });
        assert!(!asked
            .iter()
            .any(|s| s == "0B261FDF6000AB" || s == "emulator-5554"));
        assert_eq!(asked.len(), 4);
    }

    #[test]
    fn authorized_without_props_uses_adb_token() {
        let list = build(parse_list(LIST), |_| None);
        let a = &list[0];
        assert_eq!(a.readiness, Readiness::Unknown);
        assert_eq!(a.name.display, "SM_A576B");
        assert_eq!(a.name.source, NameSource::AdbList);
    }

    #[test]
    fn too_old_android() {
        let list = build(
            parse_list("X1234567 device usb:1-1 transport_id:1\n"),
            |_| {
                Some(Props::parse(
                    "[ro.build.version.sdk]: [19]\n[ro.serialno]: [X1234567]\n",
                ))
            },
        );
        assert_eq!(list[0].readiness, Readiness::TooOld { sdk: 19 });
    }

    #[test]
    fn reconnect_usb_to_wifi_keeps_identity() {
        let usb = build(
            parse_list("R5CX10ABCDE device usb:1-4 transport_id:3\n"),
            |_| Some(Props::parse(&samsung("R5CX10ABCDE"))),
        );
        let wifi = build(
            parse_list("192.168.1.40:5555 device transport_id:9\n"),
            |_| Some(Props::parse(&samsung("R5CX10ABCDE"))),
        );
        assert_eq!(usb[0].id, wifi[0].id);
        assert_eq!(usb[0].name, wifi[0].name);
    }

    #[test]
    fn scan_over_fake_server() {
        let fake = fake();
        let list = scan(&fake.client()).unwrap();
        assert_eq!(
            list,
            scan_list(),
            "same result over the wire as with direct props"
        );
        assert_eq!(list.len(), 5);
        assert!(fake.requests().contains(&"host:devices-l".to_string()));
        assert!(fake.requests().contains(&"host:transport-id:3".to_string()));
        assert!(
            !fake.requests().iter().any(|r| r == "host:transport-id:4"),
            "unauthorized device not queried"
        );
    }
}
