//! `adb devices -l` (and `host:track-devices-l`) output.
//!
//! One line per device: the serial, the state, then `key:value` tokens, e.g.
//! `R5CX123 device usb:1-4 product:x model:SM_A576B device:y transport_id:3`.
//! The state "no permissions" contains a space, and is followed by a hint.

use std::collections::BTreeMap;

/// What ADB reports about a device's connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceState {
    /// Authorized and online: properties can be read and mirroring can start.
    Device,
    /// USB debugging is on but the RSA key was not accepted on the device yet.
    Unauthorized,
    /// The device is waiting for the user to accept the RSA prompt.
    Authorizing,
    /// ADB lost contact (cable, sleep, or a stale Wi-Fi connection).
    Offline,
    /// The transport is being set up.
    Connecting,
    /// The PC lacks the permission or driver to open the device.
    NoPermissions,
    /// Recovery, sideload, rescue, bootloader: not an Android system to mirror.
    NotAndroidSystem(String),
    Other(String),
}

impl DeviceState {
    pub fn parse(s: &str) -> Self {
        match s {
            "device" => Self::Device,
            "unauthorized" => Self::Unauthorized,
            "authorizing" => Self::Authorizing,
            "offline" => Self::Offline,
            "connecting" => Self::Connecting,
            "recovery" | "sideload" | "rescue" | "bootloader" | "host" => {
                Self::NotAndroidSystem(s.to_string())
            }
            s if s.starts_with("no permissions") => Self::NoPermissions,
            other => Self::Other(other.to_string()),
        }
    }

    /// Properties (and so names and identity) are only trustworthy in this state.
    pub fn is_authorized(&self) -> bool {
        matches!(self, Self::Device)
    }
}

/// How the device is attached, derived from the ADB serial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    Usb,
    /// `adb connect <ip>:<port>` (classic TCP/IP).
    WifiTcp {
        host: String,
        port: u16,
    },
    /// Android 11+ wireless debugging, seen through mDNS:
    /// `adb-<serialno>-<suffix>._adb-tls-connect._tcp`.
    WifiTls {
        service: String,
        serialno: Option<String>,
    },
    Emulator,
    Unknown,
}

impl Connection {
    pub fn is_wireless(&self) -> bool {
        matches!(self, Self::WifiTcp { .. } | Self::WifiTls { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdbDevice {
    /// The serial ADB uses to address the device (`host:transport:<serial>`).
    /// Not a stable identity: for Wi-Fi it is an address or an mDNS name.
    pub serial: String,
    pub state: DeviceState,
    pub connection: Connection,
    /// `model:` token (spaces replaced by `_` by ADB). Only present when authorized.
    pub model: Option<String>,
    pub product: Option<String>,
    pub device: Option<String>,
    pub transport_id: Option<u64>,
    /// `usb:` port path, e.g. `1-4`.
    pub usb_port: Option<String>,
}

fn connection_for(serial: &str) -> Connection {
    if let Some(service) = serial.strip_suffix("._adb-tls-connect._tcp") {
        let serialno = service
            .strip_prefix("adb-")
            .and_then(|s| s.rsplit_once('-'))
            .map(|(sn, _)| sn.to_string())
            .filter(|s| !s.is_empty());
        return Connection::WifiTls {
            service: serial.to_string(),
            serialno,
        };
    }
    if serial.starts_with("emulator-") {
        return Connection::Emulator;
    }
    if let Some((host, port)) = serial.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            let host = host.trim_start_matches('[').trim_end_matches(']');
            if host.parse::<std::net::IpAddr>().is_ok() || host.contains('.') {
                return Connection::WifiTcp {
                    host: host.to_string(),
                    port,
                };
            }
        }
    }
    // `adb devices` without `-l` has no usb: token; any other plain serial is USB in practice.
    Connection::Usb
}

/// Parses one line; `None` for headers, daemon messages and blank lines.
pub fn parse_line(line: &str) -> Option<AdbDevice> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.trim().is_empty() || line.starts_with("List of devices") || line.starts_with('*') {
        return None;
    }
    let (serial, rest) = match line.split_once(['\t', ' ']) {
        Some((s, r)) => (s.trim(), r.trim_start()),
        None => return None,
    };
    if serial.is_empty() || rest.is_empty() {
        return None;
    }
    let (state, tail) = if let Some(after) = rest.strip_prefix("no permissions") {
        // "no permissions (reason); see [url]" up to the first key:value token.
        let tail_start = after
            .find(" usb:")
            .or_else(|| after.find(" transport_id:"))
            .unwrap_or(after.len());
        (DeviceState::NoPermissions, &after[tail_start..])
    } else {
        let (s, t) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        (DeviceState::parse(s), t)
    };
    let mut kv = BTreeMap::new();
    for tok in tail.split_whitespace() {
        if let Some((k, v)) = tok.split_once(':') {
            kv.insert(k, v);
        }
    }
    let get = |k: &str| kv.get(k).map(|v| v.to_string()).filter(|v| !v.is_empty());
    Some(AdbDevice {
        serial: serial.to_string(),
        connection: connection_for(serial),
        state,
        model: get("model"),
        product: get("product"),
        device: get("device"),
        transport_id: kv.get("transport_id").and_then(|v| v.parse().ok()),
        usb_port: get("usb"),
    })
}

/// Parses a whole `adb devices -l` listing.
pub fn parse_list(text: &str) -> Vec<AdbDevice> {
    text.lines().filter_map(parse_line).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "List of devices attached\n\
        R5CX10ABCDE            device usb:1-4 product:a57xnseea model:SM_A576B device:a57x transport_id:3\n\
        0B261FDF6000AB         unauthorized usb:1-2 transport_id:4\n\
        192.168.1.40:5555      device product:LH8n model:TECNO_LH8n device:TECNO-LH8n transport_id:5\n\
        adb-9a3f12c4-XyZ12a._adb-tls-connect._tcp device product:garnet_global model:23117RA68G device:garnet transport_id:6\n\
        emulator-5554          offline transport_id:7\n\
        3a1b2c3d               no permissions (missing udev rules? user is in the plugdev group); see [http://developer.android.com/tools/device.html] usb:3-1 transport_id:8\n\
        HT7A1B2C3              recovery usb:1-1 transport_id:9\n\
        \n";

    #[test]
    fn parses_every_state() {
        let d = parse_list(LIST);
        assert_eq!(d.len(), 7);
        assert_eq!(d[0].state, DeviceState::Device);
        assert_eq!(d[0].model.as_deref(), Some("SM_A576B"));
        assert_eq!(d[0].usb_port.as_deref(), Some("1-4"));
        assert_eq!(d[0].transport_id, Some(3));
        assert_eq!(d[0].connection, Connection::Usb);
        assert_eq!(d[1].state, DeviceState::Unauthorized);
        assert_eq!(d[1].model, None, "unauthorized devices report no model");
        assert_eq!(d[4].state, DeviceState::Offline);
        assert_eq!(d[4].connection, Connection::Emulator);
        assert_eq!(d[5].state, DeviceState::NoPermissions);
        assert_eq!(d[5].usb_port.as_deref(), Some("3-1"));
        assert_eq!(d[5].transport_id, Some(8));
        assert_eq!(d[6].state, DeviceState::NotAndroidSystem("recovery".into()));
        assert!(!d[6].state.is_authorized());
    }

    #[test]
    fn wireless_kinds() {
        let d = parse_list(LIST);
        assert_eq!(
            d[2].connection,
            Connection::WifiTcp {
                host: "192.168.1.40".into(),
                port: 5555
            }
        );
        match &d[3].connection {
            Connection::WifiTls { serialno, .. } => {
                assert_eq!(serialno.as_deref(), Some("9a3f12c4"))
            }
            c => panic!("{c:?}"),
        }
        assert!(d[2].connection.is_wireless() && d[3].connection.is_wireless());
        let v6 = parse_line("[fe80::1]:37099 device transport_id:1").unwrap();
        assert_eq!(
            v6.connection,
            Connection::WifiTcp {
                host: "fe80::1".into(),
                port: 37099
            }
        );
    }

    #[test]
    fn mdns_serial_with_dashes() {
        let d =
            parse_line("adb-ABC-123-4ab9Zq._adb-tls-connect._tcp device transport_id:2").unwrap();
        match d.connection {
            Connection::WifiTls { serialno, .. } => {
                assert_eq!(serialno.as_deref(), Some("ABC-123"))
            }
            c => panic!("{c:?}"),
        }
    }

    #[test]
    fn plain_list_and_noise() {
        let d = parse_list("* daemon not running; starting now at tcp:5037\r\n* daemon started successfully\r\nList of devices attached\r\nX1\tdevice\r\nX2\toffline\r\n\r\n");
        assert_eq!(d.len(), 2);
        assert_eq!(
            (d[0].serial.as_str(), &d[0].state),
            ("X1", &DeviceState::Device)
        );
        assert!(parse_list("").is_empty());
        assert!(parse_line("lonely").is_none());
    }
}
