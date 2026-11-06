//! Android over Wi-Fi (phase A6, master prompt section 4 "Wireless Mode").
//!
//! Two ways, both through the ADB server's own services:
//!
//! - **TCP/IP from USB** (any Android version): with the cable in,
//!   `tcpip:5555` restarts adbd on the phone in TCP mode; ZenLink reads the
//!   phone's Wi-Fi address and sends `host:connect:<ip>:5555`. The cable can
//!   then be removed.
//! - **Wireless debugging** (Android 11+): the phone shows an address and a
//!   six-digit code under Developer options › Wireless debugging › Pair
//!   device with pairing code. `host:pair:<code>:<ip>:<port>` pairs once;
//!   `host:connect:<ip>:<port>` (the address on the Wireless debugging
//!   screen) connects.
//!
//! The device keeps the same stable identity over Wi-Fi (hardware serial,
//! `identity.rs`), so its settings follow it. No secret is stored by
//! ZenLink: the pairing key lives in the ADB server's own key store.

use std::io::Read;

use crate::adb::{AdbClient, AdbError, Target};

/// Default port of `adb tcpip`.
pub const TCPIP_PORT: u16 = 5555;

/// `host` and `port` from user input such as `192.168.1.20:37099`.
/// Accepts IPv4 addresses and plain host names; the port is required.
pub fn parse_endpoint(s: &str) -> Option<(String, u16)> {
    let s = s.trim();
    let (host, port) = s.rsplit_once(':')?;
    let port: u16 = port.trim().parse().ok().filter(|p| *p > 0)?;
    let host = host.trim();
    let ok_host = !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    ok_host.then(|| (host.to_string(), port))
}

/// A pairing code is exactly six digits.
pub fn valid_pair_code(s: &str) -> bool {
    let s = s.trim();
    s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit())
}

/// What the ADB server answered to `host:connect` / `host:pair`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Ok(String),
    Failed(String),
}

/// `host:connect` answers `connected to <ip>:<port>` or `already connected
/// to ...` on success, and a reason (`failed to connect to ...`, `cannot
/// connect to ...`) otherwise; both come with OKAY.
pub fn connect_reply(text: &str) -> Reply {
    let t = text.trim();
    if t.starts_with("connected to") || t.starts_with("already connected to") {
        Reply::Ok(t.to_string())
    } else {
        Reply::Failed(t.to_string())
    }
}

/// `host:pair` answers `Successfully paired to <ip>:<port> [guid=...]` or
/// `Failed: ...`.
pub fn pair_reply(text: &str) -> Reply {
    let t = text.trim();
    if t.starts_with("Successfully paired") {
        Reply::Ok(t.to_string())
    } else {
        Reply::Failed(t.to_string())
    }
}

/// The phone's IPv4 address on Wi-Fi from `ip -f inet addr show wlan0`
/// (`inet 192.168.1.20/24 brd ... scope global wlan0`). Loopback and
/// link-local addresses are ignored.
pub fn wlan_ip(ip_output: &str) -> Option<String> {
    ip_output.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("inet ")?;
        let addr = rest.split(['/', ' ']).next()?;
        let parts: Vec<u8> = addr.split('.').filter_map(|p| p.parse().ok()).collect();
        let usable = parts.len() == 4 && parts[0] != 127 && !(parts[0] == 169 && parts[1] == 254);
        usable.then(|| addr.to_string())
    })
}

impl AdbClient {
    /// `adb connect host:port`.
    pub fn connect_device(&self, host: &str, port: u16) -> Result<Reply, AdbError> {
        // A connect to an unreachable address takes the server several seconds.
        let mut slow = self.clone();
        slow.timeout = self.timeout.max(std::time::Duration::from_secs(15));
        let out = slow.host_query(&format!("host:connect:{host}:{port}"))?;
        Ok(connect_reply(&String::from_utf8_lossy(&out)))
    }

    /// `adb pair host:port code` (Android 11+ wireless debugging).
    pub fn pair(&self, host: &str, port: u16, code: &str) -> Result<Reply, AdbError> {
        let mut slow = self.clone();
        slow.timeout = self.timeout.max(std::time::Duration::from_secs(20));
        let out = slow.host_query(&format!("host:pair:{}:{host}:{port}", code.trim()))?;
        Ok(pair_reply(&String::from_utf8_lossy(&out)))
    }

    /// `adb tcpip <port>`: the phone's adbd restarts listening on TCP. The
    /// USB connection drops for a moment.
    pub fn tcpip(&self, target: &Target, port: u16) -> Result<String, AdbError> {
        let s = self.open_service(target, &format!("tcpip:{port}"))?;
        let mut out = String::new();
        let _ = s.take(4096).read_to_string(&mut out);
        Ok(out.trim().to_string())
    }

    /// The phone's Wi-Fi IPv4 address, `None` when it is not on Wi-Fi.
    pub fn wifi_address(&self, target: &Target) -> Result<Option<String>, AdbError> {
        let out = self.shell(target, "ip -f inet addr show wlan0", 16 * 1024)?;
        Ok(wlan_ip(&String::from_utf8_lossy(&out)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_parsed_and_checked() {
        assert_eq!(
            parse_endpoint("192.168.1.20:37099"),
            Some(("192.168.1.20".into(), 37099))
        );
        assert_eq!(
            parse_endpoint(" 10.0.0.5 : 5555 "),
            Some(("10.0.0.5".into(), 5555))
        );
        assert_eq!(
            parse_endpoint("phone.local:5555"),
            Some(("phone.local".into(), 5555))
        );
        assert_eq!(parse_endpoint("192.168.1.20"), None);
        assert_eq!(parse_endpoint("192.168.1.20:0"), None);
        assert_eq!(parse_endpoint("192.168.1.20:99999"), None);
        assert_eq!(parse_endpoint(":5555"), None);
        assert_eq!(parse_endpoint("a;reboot:5555"), None);
    }

    #[test]
    fn pair_codes_are_six_digits() {
        assert!(valid_pair_code("123456"));
        assert!(valid_pair_code(" 012345 "));
        assert!(!valid_pair_code("12345"));
        assert!(!valid_pair_code("1234567"));
        assert!(!valid_pair_code("12a456"));
    }

    #[test]
    fn server_replies_are_classified() {
        assert!(matches!(
            connect_reply("connected to 192.168.1.20:5555"),
            Reply::Ok(_)
        ));
        assert!(matches!(
            connect_reply("already connected to 192.168.1.20:5555"),
            Reply::Ok(_)
        ));
        assert!(matches!(
            connect_reply("failed to connect to '192.168.1.20:5555': Connection refused"),
            Reply::Failed(_)
        ));
        assert!(matches!(
            pair_reply("Successfully paired to 192.168.1.20:37099 [guid=adb-X]"),
            Reply::Ok(_)
        ));
        assert!(matches!(
            pair_reply("Failed: Wrong password or connection was dropped."),
            Reply::Failed(_)
        ));
    }

    #[test]
    fn wifi_address_from_ip_output() {
        let out = "\
3: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP group default qlen 3000
    inet 192.168.1.20/24 brd 192.168.1.255 scope global wlan0
       valid_lft forever preferred_lft forever
";
        assert_eq!(wlan_ip(out), Some("192.168.1.20".into()));
        assert_eq!(wlan_ip("Device \"wlan0\" does not exist."), None);
        assert_eq!(wlan_ip("    inet 169.254.3.4/16 scope link wlan0"), None);
        assert_eq!(wlan_ip("    inet 127.0.0.1/8 scope host lo"), None);
    }

    #[test]
    fn connect_and_pair_use_the_host_services() {
        use crate::adb::fake::{frame, FakeAdb};
        use std::collections::HashMap;
        let mut script = HashMap::new();
        script.insert(
            "host:connect:192.168.1.20:5555".to_string(),
            frame("connected to 192.168.1.20:5555"),
        );
        script.insert(
            "host:pair:123456:192.168.1.20:37099".to_string(),
            frame("Successfully paired to 192.168.1.20:37099 [guid=adb-X]"),
        );
        let fake = FakeAdb::start(script);
        let c = fake.client();
        assert!(matches!(
            c.connect_device("192.168.1.20", 5555).unwrap(),
            Reply::Ok(_)
        ));
        assert!(matches!(
            c.pair("192.168.1.20", 37099, "123456").unwrap(),
            Reply::Ok(_)
        ));
    }
}
