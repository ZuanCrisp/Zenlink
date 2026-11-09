//! avcC / hvcC records and AVCC -> Annex B conversion for Media Foundation.

use crate::api::Codec;

const START: [u8; 4] = [0, 0, 0, 1];

#[derive(Clone, Debug, Default)]
pub struct ParamSets {
    /// SPS/PPS (and VPS for HEVC) as Annex B.
    pub annexb: Vec<u8>,
    /// Size of each NAL length prefix in samples (1, 2 or 4).
    pub length_size: usize,
}

pub fn parse_config(codec: &Codec, rec: &[u8]) -> Option<ParamSets> {
    match codec {
        Codec::H264 => parse_avcc(rec),
        Codec::Hevc => parse_hvcc(rec),
    }
}

fn parse_avcc(r: &[u8]) -> Option<ParamSets> {
    if r.len() < 7 || r[0] != 1 {
        return None;
    }
    let length_size = (r[4] & 3) as usize + 1;
    let mut out = Vec::new();
    let mut i = 5;
    let nsps = (r[i] & 0x1f) as usize;
    i += 1;
    for _ in 0..nsps {
        i = push_ps(r, i, &mut out)?;
    }
    let npps = *r.get(i)? as usize;
    i += 1;
    for _ in 0..npps {
        i = push_ps(r, i, &mut out)?;
    }
    Some(ParamSets {
        annexb: out,
        length_size,
    })
}

fn parse_hvcc(r: &[u8]) -> Option<ParamSets> {
    if r.len() < 23 {
        return None;
    }
    let length_size = (r[21] & 3) as usize + 1;
    let arrays = r[22] as usize;
    let mut out = Vec::new();
    let mut i = 23;
    for _ in 0..arrays {
        i += 1; // array_completeness | NAL type
        let n = u16::from_be_bytes([*r.get(i)?, *r.get(i + 1)?]) as usize;
        i += 2;
        for _ in 0..n {
            i = push_ps(r, i, &mut out)?;
        }
    }
    Some(ParamSets {
        annexb: out,
        length_size,
    })
}

fn push_ps(r: &[u8], i: usize, out: &mut Vec<u8>) -> Option<usize> {
    let len = u16::from_be_bytes([*r.get(i)?, *r.get(i + 1)?]) as usize;
    let nal = r.get(i + 2..i + 2 + len)?;
    out.extend_from_slice(&START);
    out.extend_from_slice(nal);
    Some(i + 2 + len)
}

/// Convert one length-prefixed sample to Annex B. Keyframes get the parameter
/// sets in front, so a decoder can start (or restart) on any keyframe.
pub fn to_annexb(sample: &[u8], ps: &ParamSets, key: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(sample.len() + 64 + if key { ps.annexb.len() } else { 0 });
    if key {
        out.extend_from_slice(&ps.annexb);
    }
    let n = ps.length_size.max(1);
    let mut i = 0;
    while i + n <= sample.len() {
        let mut len = 0usize;
        for b in &sample[i..i + n] {
            len = (len << 8) | *b as usize;
        }
        i += n;
        if len == 0 || i + len > sample.len() {
            break;
        }
        out.extend_from_slice(&START);
        out.extend_from_slice(&sample[i..i + len]);
        i += len;
    }
    out
}

/// NAL units of an Annex B byte stream (3- or 4-byte start codes), without
/// the start codes. Android's MediaCodec (scrcpy) produces Annex B.
pub fn split_annexb(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (n, &s) in starts.iter().enumerate() {
        let mut e = starts.get(n + 1).map_or(data.len(), |&next| next - 3);
        // A 4-byte start code leaves one zero byte at the end of the NAL before it.
        while e > s && n + 1 < starts.len() && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(&data[s..e]);
        }
    }
    out
}

/// Annex B access unit -> one sample with 4-byte NAL lengths (AVCC / HVCC).
pub fn annexb_to_lengths(data: &[u8]) -> Vec<u8> {
    let nals = split_annexb(data);
    let mut out = Vec::with_capacity(data.len() + 4);
    for nal in nals {
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
    out
}

/// avcC record (4-byte lengths) from an H.264 Annex B config packet that
/// holds the SPS (type 7) and PPS (type 8). `None` without an SPS and a PPS.
pub fn avcc_from_annexb(config: &[u8]) -> Option<Vec<u8>> {
    let nals = split_annexb(config);
    let sps: Vec<&[u8]> = nals.iter().copied().filter(|n| n[0] & 0x1f == 7).collect();
    let pps: Vec<&[u8]> = nals.iter().copied().filter(|n| n[0] & 0x1f == 8).collect();
    let first = sps.first()?;
    if pps.is_empty() || first.len() < 4 || sps.len() > 31 || pps.len() > 255 {
        return None;
    }
    let mut r = vec![
        1,
        first[1],
        first[2],
        first[3],
        0xff,
        0xe0 | sps.len() as u8,
    ];
    for s in &sps {
        r.extend_from_slice(&(s.len() as u16).to_be_bytes());
        r.extend_from_slice(s);
    }
    r.push(pps.len() as u8);
    for p in &pps {
        r.extend_from_slice(&(p.len() as u16).to_be_bytes());
        r.extend_from_slice(p);
    }
    Some(r)
}

/// hvcC record (4-byte lengths) from an H.265 Annex B config packet with
/// the VPS (type 32), SPS (33) and PPS (34). The decoder only reads the NAL
/// arrays and the length size; profile, tier and level are copied from the
/// SPS's profile_tier_level so the record stays truthful. `None` without
/// all three.
pub fn hvcc_from_annexb(config: &[u8]) -> Option<Vec<u8>> {
    let nals = split_annexb(config);
    let kind = |n: &[u8]| (n[0] >> 1) & 0x3f;
    let of = |t: u8| -> Vec<&[u8]> {
        nals.iter()
            .copied()
            .filter(|n| n.len() > 2 && kind(n) == t)
            .collect()
    };
    let (vps, sps, pps) = (of(32), of(33), of(34));
    if vps.is_empty() || sps.is_empty() || pps.is_empty() {
        return None;
    }
    let mut r = vec![0u8; 23];
    r[0] = 1; // configurationVersion
              // SPS: 2-byte NAL header, 1 byte (vps id, max sub layers, nesting), then
              // general profile_tier_level: 1 byte profile space/tier/idc, 4 bytes
              // compatibility flags, 6 bytes constraint flags, 1 byte level.
    if let Some(ptl) = sps[0].get(3..15) {
        r[1..13].copy_from_slice(ptl);
    }
    r[13] = 0xf0; // reserved + min_spatial_segmentation_idc (0)
    r[15] = 0xfc; // reserved + parallelismType (0)
    r[16] = 0xfd; // reserved + chroma_format_idc 4:2:0
    r[17] = 0xf8; // reserved + bit_depth_luma_minus8 0
    r[18] = 0xf8; // reserved + bit_depth_chroma_minus8 0
    r[21] = 0x0f; // constantFrameRate 0, numTemporalLayers 1, nesting 1, lengthSizeMinusOne 3
    r[22] = 3; // numOfArrays
    for (t, list) in [(32u8, &vps), (33, &sps), (34, &pps)] {
        r.push(0x80 | t); // array_completeness = 1
        r.extend_from_slice(&(list.len() as u16).to_be_bytes());
        for n in list.iter() {
            r.extend_from_slice(&(n.len() as u16).to_be_bytes());
            r.extend_from_slice(n);
        }
    }
    Some(r)
}

/// Exp-Golomb bit reader over an RBSP (emulation prevention bytes removed).
struct Bits {
    data: Vec<u8>,
    pos: usize,
}

impl Bits {
    fn new(nal: &[u8]) -> Self {
        let mut data = Vec::with_capacity(nal.len());
        let mut zeros = 0;
        for &b in nal {
            if zeros >= 2 && b == 3 {
                zeros = 0;
                continue;
            }
            zeros = if b == 0 { zeros + 1 } else { 0 };
            data.push(b);
        }
        Self { data, pos: 0 }
    }

    fn bit(&mut self) -> Option<u32> {
        let b = self.data.get(self.pos / 8)? >> (7 - self.pos % 8) & 1;
        self.pos += 1;
        Some(u32::from(b))
    }

    fn u(&mut self, n: usize) -> Option<u32> {
        (0..n).try_fold(0, |v, _| Some(v << 1 | self.bit()?))
    }

    fn skip(&mut self, n: usize) {
        self.pos += n;
    }

    fn ue(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while self.bit()? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        Some((1u32 << zeros) - 1 + self.u(zeros)?)
    }
}

/// The visible picture of an HEVC stream (left, top, right, bottom in luma pixels) from the
/// SPS conformance window; `None` without one. Encoders pad the coded size to a multiple of
/// 16 or 32 (an iPhone 17's 1206x2622 screen comes as 1216x2656).
pub fn hevc_visible(annexb: &[u8]) -> Option<[u32; 4]> {
    let sps = split_annexb(annexb)
        .into_iter()
        .find(|n| n.len() > 2 && (n[0] >> 1) & 0x3f == 33)?;
    let mut b = Bits::new(&sps[2..]);
    b.skip(4); // sps_video_parameter_set_id
    let sub_layers = b.u(3)? as usize;
    b.skip(1);
    b.skip(96); // general profile_tier_level
    let mut present = Vec::new();
    for _ in 0..sub_layers {
        present.push((b.bit()?, b.bit()?));
    }
    if sub_layers > 0 {
        b.skip(2 * (8 - sub_layers));
    }
    for (profile, level) in present {
        b.skip(88 * profile as usize + 8 * level as usize);
    }
    b.ue()?; // sps_seq_parameter_set_id
    let chroma = b.ue()?;
    if chroma == 3 {
        b.skip(1);
    }
    let (w, h) = (b.ue()?, b.ue()?);
    if b.bit()? == 0 {
        return None;
    }
    let (sx, sy) = match chroma {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    };
    let (l, r, t, bo) = (b.ue()? * sx, b.ue()? * sx, b.ue()? * sy, b.ue()? * sy);
    (l + r < w && t + bo < h).then_some([l, t, w - r, h - bo])
}

#[cfg(test)]
mod tests {
    #[test]
    fn hevc_visible_reads_the_conformance_window() {
        // A hand-built Main-profile SPS: 1216x2656, window right 5, bottom 17 (chroma units).
        let mut bits = String::new();
        let ue = |v: u32| {
            let n = 32 - (v + 1).leading_zeros() - 1;
            format!("{}{:0width$b}", "0".repeat(n as usize), v + 1, width = n as usize + 1)
        };
        bits += "0000"; // vps id
        bits += "000"; // max_sub_layers_minus1
        bits += "1";
        bits += &"0".repeat(96);
        bits += &ue(0); // sps id
        bits += &ue(1); // 4:2:0
        bits += &ue(1216);
        bits += &ue(2656);
        bits += "1";
        bits += &(ue(0) + &ue(5) + &ue(0) + &ue(17));
        while bits.len() % 8 != 0 {
            bits.push('1');
        }
        let mut au = vec![0, 0, 0, 1, 33 << 1, 1];
        au.extend(bits.as_bytes().chunks(8).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap()));
        assert_eq!(super::hevc_visible(&au), Some([0, 0, 1206, 2622]));
    }

    use super::*;

    #[test]
    fn hvcc_from_scrcpy_config_round_trips() {
        let mut config = vec![0, 0, 0, 1, 0x40, 0x01, 0x0c, 0x01];
        let sps = [
            0x42, 0x01, 0x01, 0x01, 0x60, 0, 0, 0, 0xb0, 0, 0, 0, 0, 0, 0x5d, 0xa0,
        ];
        config.extend_from_slice(&[0, 0, 0, 1]);
        config.extend_from_slice(&sps);
        config.extend_from_slice(&[0, 0, 0, 1, 0x44, 0x01, 0xc1, 0x72]);
        let rec = hvcc_from_annexb(&config).unwrap();
        assert_eq!(rec[1], 0x01, "general profile Main");
        assert_eq!(rec[12], 0x5d, "level 3.1");
        let ps = parse_config(&Codec::Hevc, &rec).unwrap();
        assert_eq!(ps.length_size, 4);
        assert_eq!(ps.annexb, config);
        assert!(
            hvcc_from_annexb(&[0, 0, 0, 1, 0x42, 0x01, 1]).is_none(),
            "SPS only"
        );
    }

    #[test]
    fn annexb_split_and_lengths() {
        // 4-byte start, 3-byte start, 4-byte start.
        let au = [
            0, 0, 0, 1, 0x67, 0x42, 0, 0x1f, 0, 0, 1, 0x68, 0xce, 0, 0, 0, 1, 0x65, 0x88, 0x84,
        ];
        let nals = split_annexb(&au);
        assert_eq!(
            nals,
            vec![
                &[0x67, 0x42, 0, 0x1f][..],
                &[0x68, 0xce][..],
                &[0x65, 0x88, 0x84][..]
            ]
        );
        let s = annexb_to_lengths(&au);
        assert_eq!(&s[..8], &[0, 0, 0, 4, 0x67, 0x42, 0, 0x1f]);
        assert!(split_annexb(&[]).is_empty());
        assert!(split_annexb(&[1, 2, 3]).is_empty());
    }

    #[test]
    fn avcc_from_scrcpy_config_round_trips() {
        let config = [
            0, 0, 0, 1, 0x67, 0x42, 0xc0, 0x1f, 0xda, 0, 0, 0, 1, 0x68, 0xce, 0x3c, 0x80,
        ];
        let rec = avcc_from_annexb(&config).unwrap();
        assert_eq!(&rec[..6], &[1, 0x42, 0xc0, 0x1f, 0xff, 0xe1]);
        let ps = parse_config(&Codec::H264, &rec).unwrap();
        assert_eq!(ps.length_size, 4);
        assert_eq!(ps.annexb, config.to_vec());
        // A frame converted to lengths comes back unchanged through to_annexb.
        let frame = [0, 0, 0, 1, 0x41, 0x9a, 0x01];
        assert_eq!(
            to_annexb(&annexb_to_lengths(&frame), &ps, false),
            frame.to_vec()
        );
        assert!(avcc_from_annexb(&[0, 0, 0, 1, 0x68, 1]).is_none(), "no SPS");
    }

    #[test]
    fn avcc_roundtrip() {
        // version 1, profile, compat, level, 0xFF (4-byte lengths), 1 SPS, 1 PPS
        let rec = [1, 0x64, 0, 0x28, 0xff, 0xe1, 0, 2, 0x67, 1, 1, 0, 1, 0x68];
        let ps = parse_config(&Codec::H264, &rec).unwrap();
        assert_eq!(ps.length_size, 4);
        assert_eq!(ps.annexb, vec![0, 0, 0, 1, 0x67, 1, 0, 0, 0, 1, 0x68]);
        let sample = [0, 0, 0, 2, 0x65, 9, 0, 0, 0, 1, 0x06];
        assert_eq!(
            to_annexb(&sample, &ps, false),
            vec![0, 0, 0, 1, 0x65, 9, 0, 0, 0, 1, 0x06]
        );
        assert_eq!(to_annexb(&sample, &ps, true).len(), ps.annexb.len() + 11);
    }

    #[test]
    fn hvcc_arrays() {
        let mut rec = vec![1u8; 21];
        rec.push(0x0f); // length size 4
        rec.push(1); // one array
        rec.extend([0x20, 0, 1, 0, 3, 0x40, 1, 2]); // VPS x1
        let ps = parse_config(&Codec::Hevc, &rec).unwrap();
        assert_eq!(ps.length_size, 4);
        assert_eq!(ps.annexb, vec![0, 0, 0, 1, 0x40, 1, 2]);
    }
}
