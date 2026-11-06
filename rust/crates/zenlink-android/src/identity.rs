//! Stable device identity for per-device profiles and nicknames.
//!
//! Never based on an IP address, a transport id or a display name (master
//! prompt section 4, item 8). The hardware serial (`ro.serialno`) is the same
//! over USB and Wi-Fi, so a phone keeps its profile when it switches transport.
//! Over USB the ADB serial normally equals it too; wireless debugging exposes
//! it inside the mDNS service name.

use crate::devices::{AdbDevice, Connection};
use crate::props::Props;

/// Serials that vendors ship on many units; they cannot identify one device.
const SHARED_SERIALS: &[&str] = &[
    "0123456789abcdef",
    "0123456789abcdef0",
    "0000000000000000",
    "000000000000",
    "1234567890abcdef",
    "abcdef0123456789",
    "unknown",
    "null",
    "none",
    "serial",
    "android",
];

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Strength {
    /// Derived from the device's hardware serial: stable across transports.
    Strong,
    /// No usable serial: model + ADB serial. Stable only on the same transport.
    Weak,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeviceId {
    /// Storage key, e.g. `android/R5CX10ABCDE`.
    pub key: String,
    pub strength: Strength,
}

/// A serial good enough to identify one device.
pub fn usable_serial(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.len() < 6 || s.len() > 64 {
        return None;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return None;
    }
    let lower = s.to_lowercase();
    if SHARED_SERIALS.contains(&lower.as_str()) {
        return None;
    }
    let first = lower.chars().next()?;
    if lower.chars().all(|c| c == first) {
        return None;
    }
    Some(s.to_string())
}

/// The hardware serial the transport itself reveals (USB serial or the one
/// embedded in a wireless-debugging mDNS name). `None` for `ip:port` and emulators.
pub fn transport_serial(dev: &AdbDevice) -> Option<String> {
    match &dev.connection {
        Connection::Usb => usable_serial(&dev.serial),
        Connection::WifiTls { serialno, .. } => serialno.as_deref().and_then(usable_serial),
        _ => None,
    }
}

/// Identity of a device. `props` is `None` when the device is not authorized.
pub fn identify(dev: &AdbDevice, props: Option<&Props>) -> DeviceId {
    let from_props = props.and_then(|p| {
        ["ro.serialno", "ro.boot.serialno"]
            .iter()
            .filter_map(|k| p.get(k))
            .find_map(usable_serial)
    });
    if let Some(sn) = from_props.or_else(|| transport_serial(dev)) {
        return DeviceId {
            key: format!("android/{sn}"),
            strength: Strength::Strong,
        };
    }
    let model = props
        .and_then(|p| p.get("ro.product.model"))
        .map(str::to_string)
        .or_else(|| dev.model.clone())
        .unwrap_or_default();
    let model: String = model
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let transport = match &dev.connection {
        // Never key on an address. Two serial-less devices of the same model on
        // `adb connect` share this key; the live list still tells them apart by
        // ADB serial, but their saved profile is shared (shown as "not portable").
        Connection::WifiTcp { .. } => "wifi".to_string(),
        _ => dev.serial.clone(),
    };
    DeviceId {
        key: format!("android-weak/{model}/{transport}"),
        strength: Strength::Weak,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::parse_line;

    fn props(sn: &str) -> Props {
        Props::from_pairs([("ro.serialno", sn), ("ro.product.model", "SM-A576B")])
    }

    #[test]
    fn same_identity_over_usb_and_wifi() {
        let usb = parse_line("R5CX10ABCDE device usb:1-4 model:SM_A576B transport_id:3").unwrap();
        let tcp = parse_line("192.168.1.40:5555 device model:SM_A576B transport_id:9").unwrap();
        let tls = parse_line("adb-R5CX10ABCDE-Ab12Cd._adb-tls-connect._tcp device transport_id:11")
            .unwrap();
        let p = props("R5CX10ABCDE");
        let a = identify(&usb, Some(&p));
        assert_eq!(a, identify(&tcp, Some(&p)));
        assert_eq!(a, identify(&tls, Some(&p)));
        assert_eq!(
            a,
            DeviceId {
                key: "android/R5CX10ABCDE".into(),
                strength: Strength::Strong
            }
        );
        // Even before properties are read, USB and TLS agree.
        assert_eq!(identify(&usb, None), identify(&tls, None));
    }

    #[test]
    fn identity_never_contains_ip_when_serial_known() {
        let tcp = parse_line("10.0.0.7:5555 device transport_id:2").unwrap();
        let id = identify(&tcp, Some(&props("HT7A1B2C3")));
        assert!(!id.key.contains("10.0.0.7"));
        assert_eq!(id.strength, Strength::Strong);
    }

    #[test]
    fn identical_models_get_different_identities() {
        let a = parse_line("R5CX10AAAAA device usb:1-1 model:SM_A576B").unwrap();
        let b = parse_line("R5CX10BBBBB device usb:1-2 model:SM_A576B").unwrap();
        assert_ne!(
            identify(&a, Some(&props("R5CX10AAAAA"))),
            identify(&b, Some(&props("R5CX10BBBBB")))
        );
    }

    #[test]
    fn shared_and_blank_serials_are_rejected() {
        for bad in [
            "",
            "  ",
            "0123456789ABCDEF",
            "0000000000000000",
            "unknown",
            "abc",
            "AAAAAAAA",
            "has space",
        ] {
            assert_eq!(usable_serial(bad), None, "{bad:?}");
        }
        assert_eq!(usable_serial(" 9a3f12c4 ").as_deref(), Some("9a3f12c4"));
        // A cheap device that reports a shared serial over USB falls back to a weak key.
        let d = parse_line("0123456789ABCDEF device usb:1-3 model:Tab_X").unwrap();
        let id = identify(
            &d,
            Some(&Props::from_pairs([
                ("ro.serialno", "0123456789ABCDEF"),
                ("ro.product.model", "Tab X"),
            ])),
        );
        assert_eq!(id.strength, Strength::Weak);
        assert_eq!(id.key, "android-weak/TabX/0123456789ABCDEF");
    }

    #[test]
    fn weak_wifi_key_has_no_address() {
        let d = parse_line("192.168.1.9:5555 device model:Tab_X").unwrap();
        let id = identify(&d, None);
        assert_eq!(
            id,
            DeviceId {
                key: "android-weak/TabX/wifi".into(),
                strength: Strength::Weak
            }
        );
    }

    #[test]
    fn ro_boot_serialno_is_a_fallback() {
        let d = parse_line("192.168.1.9:5555 device").unwrap();
        let p = Props::from_pairs([("ro.serialno", ""), ("ro.boot.serialno", "ZY22K4ABCD")]);
        assert_eq!(identify(&d, Some(&p)).key, "android/ZY22K4ABCD");
    }

    #[test]
    fn unauthorized_usb_still_has_identity() {
        let d = parse_line("0B261FDF6000AB unauthorized usb:1-2 transport_id:4").unwrap();
        assert_eq!(
            identify(&d, None),
            DeviceId {
                key: "android/0B261FDF6000AB".into(),
                strength: Strength::Strong
            }
        );
    }
}
