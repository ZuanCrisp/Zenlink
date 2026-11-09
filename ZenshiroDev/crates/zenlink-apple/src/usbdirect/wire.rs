//! Wire format of Apple's USB screen-capture protocol (the one QuickTime uses). Port of backend
//! usbdirect/wire.py. Every message is `[u32 length][u32 magic][...]` little-endian; values use a
//! small tagged dictionary format and media arrives as serialized CMSampleBuffers. Layout from
//! danielpaulus/quicktime_video_hack (MIT).

pub const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

pub const PING: u32 = fourcc(b"ping");
pub const SYNC: u32 = fourcc(b"sync");
pub const ASYN: u32 = fourcc(b"asyn");
pub const RPLY: u32 = fourcc(b"rply");

pub const CWPA: u32 = fourcc(b"cwpa");
pub const CVRP: u32 = fourcc(b"cvrp");
pub const CLOK: u32 = fourcc(b"clok");
pub const TIME: u32 = fourcc(b"time");
pub const AFMT: u32 = fourcc(b"afmt");
pub const SKEW: u32 = fourcc(b"skew");
pub const OG: u32 = fourcc(b"go! ");
pub const STOP: u32 = fourcc(b"stop");

pub const FEED: u32 = fourcc(b"feed");
pub const EAT: u32 = fourcc(b"eat!");
pub const RELS: u32 = fourcc(b"rels");
pub const HPD1: u32 = fourcc(b"hpd1");
pub const HPA1: u32 = fourcc(b"hpa1");
pub const HPD0: u32 = fourcc(b"hpd0");
pub const HPA0: u32 = fourcc(b"hpa0");
pub const NEED: u32 = fourcc(b"need");

pub const EMPTY_CF: u64 = 1;

const KEYV: u32 = fourcc(b"keyv");
const STRK: u32 = fourcc(b"strk");
const IDXK: u32 = fourcc(b"idxk");
const BULV: u32 = fourcc(b"bulv");
const DICT: u32 = fourcc(b"dict");
const DATV: u32 = fourcc(b"datv");
const STRV: u32 = fourcc(b"strv");
const NMBV: u32 = fourcc(b"nmbv");
const FDSC: u32 = fourcc(b"fdsc");
const SBUF: u32 = fourcc(b"sbuf");
const OPTS: u32 = fourcc(b"opts");
const SDAT: u32 = fourcc(b"sdat");
const MDIA: u32 = fourcc(b"mdia");
const VDIM: u32 = fourcc(b"vdim");
const CODC: u32 = fourcc(b"codc");
const EXTN: u32 = fourcc(b"extn");
const MEDIA_SOUND: u32 = fourcc(b"soun");
const CODEC_HEVC: u32 = fourcc(b"hvc1");
const CODEC_HEVC_ALT: u32 = fourcc(b"hev1");

pub fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

pub fn u64_at(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

fn tag(len: usize, magic: u32) -> Vec<u8> {
    let mut v = (len as u32).to_le_bytes().to_vec();
    v.extend_from_slice(&magic.to_le_bytes());
    v
}

/// CMTime: value / scale seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CmTime {
    pub value: u64,
    pub scale: u32,
    pub flags: u32,
    pub epoch: u64,
}

impl CmTime {
    pub fn ns(value: u64) -> Self {
        Self {
            value,
            scale: 1_000_000_000,
            flags: 1,
            epoch: 0,
        }
    }

    fn parse(d: &[u8], at: usize) -> Option<Self> {
        Some(Self {
            value: u64_at(d, at)?,
            scale: u32_at(d, at + 8)?,
            flags: u32_at(d, at + 12)?,
            epoch: u64_at(d, at + 16)?,
        })
    }

    fn bytes(&self) -> Vec<u8> {
        let mut v = self.value.to_le_bytes().to_vec();
        v.extend_from_slice(&self.scale.to_le_bytes());
        v.extend_from_slice(&self.flags.to_le_bytes());
        v.extend_from_slice(&self.epoch.to_le_bytes());
        v
    }

    pub fn seconds(&self) -> f64 {
        if self.scale == 0 {
            0.0
        } else {
            self.value as f64 / self.scale as f64
        }
    }
}

/// A value in an outgoing string-key dictionary.
pub enum Value {
    Bool(bool),
    F64(f64),
    U32(u32),
    Str(&'static str),
    Data(Vec<u8>),
    Dict(Vec<(&'static str, Value)>),
}

fn value_bytes(v: &Value) -> Vec<u8> {
    let body = |magic, b: &[u8]| [tag(b.len() + 8, magic), b.to_vec()].concat();
    match v {
        Value::Bool(b) => body(BULV, &[*b as u8]),
        Value::F64(f) => body(NMBV, &[&[6u8][..], &f.to_le_bytes()].concat()),
        Value::U32(n) => body(NMBV, &[&[3u8][..], &n.to_le_bytes()].concat()),
        Value::Str(s) => body(STRV, s.as_bytes()),
        Value::Data(d) => body(DATV, d),
        Value::Dict(d) => dict_bytes(d),
    }
}

pub fn dict_bytes(entries: &[(&str, Value)]) -> Vec<u8> {
    let mut pairs = Vec::new();
    for (k, v) in entries {
        let mut body = tag(k.len() + 8, STRK);
        body.extend_from_slice(k.as_bytes());
        body.extend(value_bytes(v));
        pairs.extend(tag(body.len() + 8, KEYV));
        pairs.extend(body);
    }
    [tag(pairs.len() + 8, DICT), pairs].concat()
}

/// Index-key dictionary values we need from a format description extension: data by key.
fn index_dict(d: &[u8], outer: u32) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    let (Some(len), Some(magic)) = (u32_at(d, 0), u32_at(d, 4)) else {
        return out;
    };
    if magic != outer {
        return out;
    }
    let end = (len as usize).min(d.len());
    let mut pos = 8;
    while let Some(plen) = u32_at(d, pos).map(|n| n as usize) {
        if plen < 8 || pos + plen > end || u32_at(d, pos + 4) != Some(KEYV) {
            break;
        }
        let pair = &d[pos + 8..pos + plen];
        if let (Some(klen), Some(IDXK)) = (u32_at(pair, 0).map(|n| n as usize), u32_at(pair, 4)) {
            if let (Some(k), Some(v)) = (pair.get(8..10), pair.get(klen..)) {
                out.push((u16::from_le_bytes([k[0], k[1]]), v));
            }
        }
        pos += plen;
    }
    out
}

/// A tagged value's body if it is `magic`.
fn body_of(v: &[u8], magic: u32) -> Option<&[u8]> {
    let len = u32_at(v, 0)? as usize;
    (u32_at(v, 4)? == magic).then(|| v.get(8..len)).flatten()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Format {
    pub sound: bool,
    pub width: u32,
    pub height: u32,
    pub codec: u32,
    /// avcC / hvcC decoder configuration record.
    pub config: Vec<u8>,
    pub channels: u32,
    pub bits: u32,
    pub rate: f64,
}

impl Format {
    fn parse(d: &[u8]) -> Option<Self> {
        if u32_at(d, 4)? != FDSC || u32_at(d, 12)? != MDIA {
            return None;
        }
        let media = u32_at(d, 16)?;
        let mut pos = 8 + u32_at(d, 8)? as usize;
        if media == MEDIA_SOUND {
            let a = d.get(pos + 8..)?;
            return Some(Self {
                sound: true,
                rate: f64::from_le_bytes(a.get(..8)?.try_into().ok()?),
                channels: u32_at(a, 28)?,
                bits: u32_at(a, 32)?,
                ..Self::default()
            });
        }
        let mut f = Self::default();
        let end = (u32_at(d, 0)? as usize).min(d.len());
        while pos + 8 <= end {
            let len = u32_at(d, pos)? as usize;
            if len < 8 {
                return None;
            }
            match u32_at(d, pos + 4)? {
                VDIM => {
                    f.width = u32_at(d, pos + 8)?;
                    f.height = u32_at(d, pos + 12)?;
                }
                CODC => f.codec = u32_at(d, pos + 8)?,
                EXTN => {
                    let ext = index_dict(d.get(pos..pos + len)?, EXTN);
                    // 49: SampleDescriptionExtensionAtoms; 105 in it is avcC, else any record.
                    if let Some(atoms) = ext.iter().find(|(k, _)| *k == 49).map(|(_, v)| *v) {
                        let atoms = index_dict(atoms, DICT);
                        let datas: Vec<(u16, &[u8])> = atoms
                            .iter()
                            .filter_map(|(k, v)| Some((*k, body_of(v, DATV)?)))
                            .collect();
                        f.config = datas
                            .iter()
                            .find(|(k, _)| *k == 105)
                            .or_else(|| datas.iter().find(|(_, v)| v.len() > 6))
                            .map(|(_, v)| v.to_vec())
                            .unwrap_or_default();
                    }
                }
                _ => {}
            }
            pos += len;
        }
        Some(f)
    }

    pub fn is_hevc(&self) -> bool {
        if self.codec == CODEC_HEVC || self.codec == CODEC_HEVC_ALT {
            return true;
        }
        let c = &self.config;
        self.codec == 0 && c.len() >= 23 && c[0] == 1 && (c[21] & 3) == 3 && c[22] < 16
    }
}

#[derive(Debug, Default)]
pub struct Sample {
    pub pts: CmTime,
    pub data: Vec<u8>,
    pub format: Option<Format>,
}

impl Sample {
    pub fn parse(d: &[u8]) -> Option<Self> {
        if u32_at(d, 4)? != SBUF {
            return None;
        }
        let mut s = Self::default();
        let end = (u32_at(d, 0)? as usize).min(d.len());
        let mut pos = 8;
        while pos < end {
            let len = u32_at(d, pos)? as usize;
            if len < 8 {
                return None;
            }
            match u32_at(d, pos + 4)? {
                OPTS => s.pts = CmTime::parse(d, pos + 8)?,
                SDAT => s.data = d.get(pos + 8..pos + len)?.to_vec(),
                FDSC => s.format = Format::parse(d.get(pos..pos + len)?),
                _ => {}
            }
            pos += len;
        }
        Some(s)
    }
}

fn head(len: usize, kind: u32, a: u64, b: u32) -> Vec<u8> {
    let mut v = tag(len, kind);
    v.extend_from_slice(&a.to_le_bytes());
    v.extend_from_slice(&b.to_le_bytes());
    v
}

/// Reply to the device's PING; `active`: host-initiated after silence.
pub fn ping(active: bool) -> Vec<u8> {
    let mut v = tag(16, PING);
    v.extend_from_slice(&(0x1_0000_0000u64 | active as u64).to_le_bytes());
    v
}

pub fn reply_clock(corr: u64, clock: u64) -> Vec<u8> {
    let mut v = head(28, RPLY, corr, 0);
    v.extend_from_slice(&clock.to_le_bytes());
    v
}

pub fn reply_empty(corr: u64) -> Vec<u8> {
    let mut v = head(24, RPLY, corr, 0);
    v.extend_from_slice(&[0; 4]);
    v
}

pub fn reply_time(corr: u64, t: CmTime) -> Vec<u8> {
    [head(44, RPLY, corr, 0), t.bytes()].concat()
}

pub fn reply_skew(corr: u64, skew: f64) -> Vec<u8> {
    [head(28, RPLY, corr, 0), skew.to_le_bytes().to_vec()].concat()
}

pub fn reply_dict(corr: u64, entries: &[(&str, Value)]) -> Vec<u8> {
    let body = dict_bytes(entries);
    [head(body.len() + 20, RPLY, corr, 0), body].concat()
}

pub fn asyn(sub: u32, clock: u64, body: &[u8]) -> Vec<u8> {
    [head(body.len() + 20, ASYN, clock, sub), body.to_vec()].concat()
}

/// Ask for video; DisplaySize is the receiver's preferred picture size.
pub fn hpd1(w: u32, h: u32) -> Vec<u8> {
    let d = dict_bytes(&[
        ("Valeria", Value::Bool(true)),
        ("HEVCDecoderSupports444", Value::Bool(true)),
        (
            "DisplaySize",
            Value::Dict(vec![
                ("Width", Value::F64(w as f64)),
                ("Height", Value::F64(h as f64)),
            ]),
        ),
    ]);
    asyn(HPD1, EMPTY_CF, &d)
}

/// Ask for audio as 48 kHz stereo 16-bit PCM.
pub fn hpa1(clock: u64) -> Vec<u8> {
    let rate = 48000f64.to_le_bytes();
    let mut asbd = rate.to_vec();
    for n in [fourcc(b"lpcm"), 12, 4, 1, 4, 2, 16, 0] {
        asbd.extend_from_slice(&n.to_le_bytes());
    }
    asbd.extend_from_slice(&rate); // 56 bytes: the ASBD followed by the rate twice, as the host sends it
    asbd.extend_from_slice(&rate);
    let d = dict_bytes(&[
        ("BufferAheadInterval", Value::F64(0.073)),
        ("deviceUID", Value::Str("ZenLink")),
        ("ScreenLatency", Value::F64(0.04)),
        ("formats", Value::Data(asbd)),
        ("EDIDAC3Support", Value::U32(0)),
        ("deviceName", Value::Str("ZenLink")),
    ]);
    asyn(HPA1, clock, &d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn outgoing_packets_match_wire_py() {
        // wire.hpd1(1920, 1080) / wire.hpa1(0x1122) from the Python backend.
        assert_eq!(hpd1(1920, 1080), hex("db0000006e797361010000000000000031647068c700000074636964200000007679656b0f0000006b72747356616c6572696109000000766c7562012f0000007679656b1e0000006b727473484556434465636f646572537570706f72747334343409000000766c756201700000007679656b130000006b727473446973706c617953697a655500000074636964260000007679656b0d0000006b72747357696474681100000076626d6e060000000000009e40270000007679656b0e0000006b7274734865696768741100000076626d6e060000000000e09040"));
        assert_eq!(hpa1(0x1122), hex("510100006e7973612211000000000000316170683d01000074636964340000007679656b1b0000006b7274734275666665724168656164496e74657276616c1100000076626d6e06e3a59bc420b0b23f280000007679656b110000006b7274736465766963655549440f000000767274735a656e4c696e6b2e0000007679656b150000006b72747353637265656e4c6174656e63791100000076626d6e067b14ae47e17aa43f570000007679656b0f0000006b727473666f726d6174734000000076746164000000000070e7406d63706c0c000000040000000100000004000000020000001000000000000000000000000070e740000000000070e7402b0000007679656b160000006b72747345444944414333537570706f72740d00000076626d6e0300000000290000007679656b120000006b7274736465766963654e616d650f000000767274735a656e4c696e6b"));
        assert_eq!(ping(false).len(), 16);
        assert_eq!(reply_empty(5).len(), 24);
        assert_eq!(reply_time(5, CmTime::ns(1)).len(), 44);
    }

    #[test]
    fn parses_a_video_sample() {
        // sbuf { opts(CMTime), fdsc { mdia vide, vdim, codc avc1, extn { 49: { 105: avcC } } }, sdat }
        let avcc = vec![1, 0x64, 0, 0x1F, 0xFF, 0xE1, 0, 0];
        let data_v = [tag(avcc.len() + 8, DATV), avcc.clone()].concat();
        let idx = |k: u16, v: &[u8]| {
            let key = [tag(10, IDXK), k.to_le_bytes().to_vec()].concat();
            let body = [key, v.to_vec()].concat();
            [tag(body.len() + 8, KEYV), body].concat()
        };
        let atoms = idx(105, &data_v);
        let atoms = [tag(atoms.len() + 8, DICT), atoms].concat();
        let ext = idx(49, &atoms);
        let ext = [tag(ext.len() + 8, EXTN), ext].concat();
        let mdia = [tag(12, MDIA), fourcc(b"vide").to_le_bytes().to_vec()].concat();
        let vdim = [
            tag(16, VDIM),
            1170u32.to_le_bytes().to_vec(),
            2532u32.to_le_bytes().to_vec(),
        ]
        .concat();
        let codc = [tag(12, CODC), fourcc(b"avc1").to_le_bytes().to_vec()].concat();
        let inner = [mdia, vdim, codc, ext].concat();
        let fdsc = [tag(inner.len() + 8, FDSC), inner].concat();
        let opts = [tag(32, OPTS), CmTime::ns(42).bytes()].concat();
        let sdat = [tag(13, SDAT), vec![0, 0, 0, 1, 0x65]].concat();
        let inner = [opts, fdsc, sdat].concat();
        let sbuf = [tag(inner.len() + 8, SBUF), inner].concat();
        let s = Sample::parse(&sbuf).unwrap();
        assert_eq!(s.pts.value, 42);
        assert_eq!(s.data, vec![0, 0, 0, 1, 0x65]);
        let f = s.format.unwrap();
        assert_eq!((f.width, f.height, f.config.clone()), (1170, 2532, avcc));
        assert!(!f.is_hevc());
        let _ = Sample::parse(&sbuf[..20]); // truncated: no panic
    }
}
