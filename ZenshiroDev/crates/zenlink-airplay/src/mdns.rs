//! The `_airplay._tcp` and `_raop._tcp` adverts, so iOS lists the receiver in Screen Mirroring.

use std::net::IpAddr;

use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::info::Receiver;

/// IPv4 addresses of real LAN adapters (no virtual switches, VPN taps, WSL, loopback).
pub fn lan_ipv4() -> Vec<IpAddr> {
    const SKIP: [&str; 7] = ["vethernet", "virtual", "vmware", "hyper-v", "loopback", "wsl", "tap"];
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| {
            let n = i.name.to_lowercase();
            !SKIP.iter().any(|s| n.contains(s))
        })
        .map(|i| i.ip())
        .filter(|ip| matches!(ip, IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local()))
        .collect()
}

/// Registered adverts; dropping it withdraws them.
pub struct Advert {
    daemon: ServiceDaemon,
    names: Vec<String>,
}

impl Advert {
    pub fn start(r: &Receiver, port: u16) -> Result<Self, String> {
        let ips = lan_ipv4();
        let daemon = ServiceDaemon::new().map_err(|e| format!("mDNS: {e}"))?;
        let mac = r.id.mac.replace(':', "");
        let host = format!("ZenLink-{}.local.", &mac[mac.len() - 6..]);
        let raop = format!("{mac}@{}", r.name);
        let mut names = Vec::new();
        for (ty, name, txt) in [
            ("_airplay._tcp.local.", r.name.to_string(), r.txt_airplay()),
            ("_raop._tcp.local.", raop, r.txt_raop()),
        ] {
            let props: Vec<(&str, &str)> = txt.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let info = ServiceInfo::new(ty, &name, &host, &ips[..], port, &props[..])
                .map_err(|e| format!("mDNS {ty}: {e}"))?;
            names.push(info.get_fullname().to_string());
            daemon.register(info).map_err(|e| format!("mDNS {ty}: {e}"))?;
        }
        log::info!("airplay: advertised \"{}\" on {ips:?}", r.name);
        Ok(Self { daemon, names })
    }
}

impl Drop for Advert {
    fn drop(&mut self) {
        for n in &self.names {
            let _ = self.daemon.unregister(n);
        }
        let _ = self.daemon.shutdown();
    }
}
