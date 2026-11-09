//! What the receiver says about itself: `GET /info` and the mDNS TXT records.

use plist::{Dictionary, Value};

use crate::identity::Identity;

/// Port 7100 (not 7000), so it runs next to the stable v2.2.31; the mDNS records carry it.
pub const PORT: u16 = 7100;
pub const MODEL: &str = "AppleTV3,2";
pub const SOURCE_VERSION: &str = "220.68";
pub const PI: &str = "2e388006-13ba-4041-9a67-25dd4a43d536";
/// The screen offered: iOS mirrors at its own size up to this.
pub const DISPLAY: (u32, u32) = (3840, 2160);

/// Apple TV 3 mirroring with legacy pairing; bit 42 (SupportsScreenMultiCodec) lets iOS send HEVC.
pub fn features(hevc: bool) -> (u32, u32) {
    (0x5A7F_FEE6, if hevc { 0x400 } else { 0 })
}

pub struct Receiver<'a> {
    pub id: &'a Identity,
    pub name: &'a str,
    pub hevc: bool,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Receiver<'_> {
    fn ft(&self) -> String {
        let (lo, hi) = features(self.hevc);
        format!("0x{lo:X},0x{hi:X}")
    }

    pub fn txt_airplay(&self) -> Vec<(&'static str, String)> {
        vec![
            ("deviceid", self.id.mac.clone()),
            ("features", self.ft()),
            ("flags", "0x4".into()),
            ("model", MODEL.into()),
            ("pk", hex(&self.id.pk())),
            ("pi", PI.into()),
            ("srcvers", SOURCE_VERSION.into()),
            ("vv", "2".into()),
        ]
    }

    pub fn txt_raop(&self) -> Vec<(&'static str, String)> {
        let s = |v: &str| v.to_string();
        vec![
            ("ch", s("2")),
            ("cn", s("0,1,2,3")),
            ("da", s("true")),
            ("et", s("0,3,5")),
            ("vv", s("2")),
            ("ft", self.ft()),
            ("am", s(MODEL)),
            ("md", s("0,1,2")),
            ("rhd", s("5.6.0.0")),
            ("pw", s("false")),
            ("sf", s("0x4")),
            ("sr", s("44100")),
            ("ss", s("16")),
            ("sv", s("false")),
            ("tp", s("UDP")),
            ("txtvers", s("1")),
            ("vs", s(SOURCE_VERSION)),
            ("vn", s("65537")),
            ("pk", hex(&self.id.pk())),
        ]
    }

    fn txt_bytes(txt: &[(&str, String)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (k, v) in txt {
            let item = format!("{k}={v}");
            out.push(item.len() as u8);
            out.extend_from_slice(item.as_bytes());
        }
        out
    }

    /// The `GET /info` reply (binary plist). With a plist body (`qualifier`), iOS asks only
    /// for one TXT record.
    pub fn info(&self, content_type: Option<&str>, body: &[u8]) -> Vec<u8> {
        let mut res = Dictionary::new();
        let qualifier = content_type
            .filter(|t| t.contains("binary-plist"))
            .and_then(|_| Value::from_reader(std::io::Cursor::new(body)).ok())
            .and_then(|v| {
                v.as_dictionary()?
                    .get("qualifier")?
                    .as_array()?
                    .first()?
                    .as_string()
                    .map(str::to_owned)
            });
        match qualifier.as_deref() {
            Some("txtAirPlay") => {
                res.insert("txtAirPlay".into(), Value::Data(Self::txt_bytes(&self.txt_airplay())));
            }
            Some("txtRAOP") => {
                res.insert("txtRAOP".into(), Value::Data(Self::txt_bytes(&self.txt_raop())));
            }
            _ => {}
        }
        if content_type.is_some_and(|t| !t.is_empty()) {
            return bplist(res);
        }
        let (lo, hi) = features(self.hevc);
        let (w, h) = DISPLAY;
        let int = |n: u64| Value::Integer(n.into());
        let dict = |items: Vec<(&str, Value)>| {
            Value::Dictionary(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
        };
        let latency = |t: u64| {
            dict(vec![
                ("type", int(t)),
                ("audioType", "default".into()),
                ("inputLatencyMicros", int(0)),
                ("outputLatencyMicros", false.into()),
            ])
        };
        let format = |t: u64| {
            dict(vec![
                ("type", int(t)),
                ("audioInputFormats", int(0x3FF_FFFC)),
                ("audioOutputFormats", int(0x3FF_FFFC)),
            ])
        };
        let items: Vec<(&str, Value)> = vec![
            ("deviceID", self.id.mac.clone().into()),
            ("macAddress", self.id.mac.clone().into()),
            ("pk", Value::Data(self.id.pk().to_vec())),
            ("features", int(((hi as u64) << 32) | lo as u64)),
            ("name", self.name.into()),
            ("pi", PI.into()),
            ("vv", int(2)),
            ("statusFlags", int(68)),
            ("keepAliveLowPower", true.into()),
            ("sourceVersion", SOURCE_VERSION.into()),
            ("keepAliveSendStatsAsBody", true.into()),
            ("model", MODEL.into()),
            ("initialVolume", Value::Real(0.0)),
            ("audioLatencies", Value::Array(vec![latency(100), latency(101)])),
            ("audioFormats", Value::Array(vec![format(100), format(101)])),
            (
                "displays",
                Value::Array(vec![dict(vec![
                    ("uuid", "e0ff8a27-6738-3d56-8a16-cc53aacee925".into()),
                    ("widthPhysical", int(0)),
                    ("heightPhysical", int(0)),
                    ("width", int(w as u64)),
                    ("height", int(h as u64)),
                    ("widthPixels", int(w as u64)),
                    ("heightPixels", int(h as u64)),
                    ("rotation", false.into()),
                    ("refreshRate", Value::Real(1.0 / 60.0)),
                    ("maxFPS", int(60)),
                    ("overscanned", false.into()),
                    ("features", int(14)),
                ])]),
            ),
        ];
        for (k, v) in items {
            res.insert(k.into(), v);
        }
        bplist(res)
    }
}

fn bplist(d: Dictionary) -> Vec<u8> {
    let mut out = Vec::new();
    plist::to_writer_binary(&mut out, &Value::Dictionary(d)).expect("plist to memory");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same values as the Python receiver's replies (testdata/, made by scratch ap_info.py with
    /// seed 00..1f and MAC 0A:11:22:33:44:55).
    #[test]
    fn info_matches_python() {
        let id = Identity {
            key: ed25519_dalek::SigningKey::from_bytes(&std::array::from_fn(|i| i as u8)),
            mac: "0A:11:22:33:44:55".into(),
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
        let read = |n: &str| Value::from_file(dir.join(n)).unwrap();
        let bp = "application/x-apple-binary-plist";
        for (hevc, tag) in [(true, "hevc"), (false, "h264")] {
            let r = Receiver { id: &id, name: "ZenLinkStudio", hevc };
            let got = |b: Vec<u8>| Value::from_reader(std::io::Cursor::new(b)).unwrap();
            assert_eq!(got(r.info(None, b"")), read(&format!("info_{tag}.bplist")), "{tag}");
            for q in ["txtAirPlay", "txtRAOP"] {
                let mut d = Dictionary::new();
                d.insert("qualifier".into(), Value::Array(vec![q.into()]));
                let body = bplist(d);
                assert_eq!(got(r.info(Some(bp), &body)), read(&format!("info_{tag}_{q}.bplist")), "{tag} {q}");
            }
        }
    }
}
