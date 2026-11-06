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

#[cfg(test)]
mod tests {
    use super::*;

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
