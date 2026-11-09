//! Low-latency H.264 for the decoder (port of backend usbdirect/h264.py). Apple's capture stream
//! has no B-frames but its SPS doesn't say so, so a hardware decoder holds a full DPB before
//! output (~170-245 ms, bursts). The SPS is rewritten with bitstream_restriction_flag = 1,
//! max_num_reorder_frames = 0, max_dec_frame_buffering = max(1, max_num_ref_frames).

const HIGH_PROFILES: [u32; 13] = [100, 110, 122, 244, 44, 83, 86, 118, 128, 138, 139, 134, 135];

fn unescape(nal: &[u8]) -> Vec<u8> {
    let (mut out, mut zeros) = (Vec::with_capacity(nal.len()), 0);
    for &b in nal {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        out.push(b);
        zeros = if b == 0 { zeros + 1 } else { 0 };
    }
    out
}

fn escape(rbsp: &[u8]) -> Vec<u8> {
    let (mut out, mut zeros) = (Vec::with_capacity(rbsp.len() + 4), 0);
    for &b in rbsp {
        if zeros >= 2 && b <= 3 {
            out.push(3);
            zeros = 0;
        }
        out.push(b);
        zeros = if b == 0 { zeros + 1 } else { 0 };
    }
    out
}

struct Reader<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn bit(&self, p: usize) -> Option<u32> {
        Some(((*self.d.get(p >> 3)? >> (7 - (p & 7))) & 1) as u32)
    }

    fn u(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit(self.pos)?;
            self.pos += 1;
        }
        Some(v)
    }

    fn ue(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while self.u(1)? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        Some(((1u64 << zeros) - 1) as u32 + if zeros > 0 { self.u(zeros)? } else { 0 })
    }

    fn se(&mut self) -> Option<i32> {
        let k = self.ue()? as i64;
        Some(if k & 1 == 1 {
            ((k + 1) / 2) as i32
        } else {
            -(k / 2) as i32
        })
    }
}

#[derive(Default)]
struct Writer {
    bits: Vec<u8>,
}

impl Writer {
    fn u(&mut self, n: u32, v: u32) {
        for i in (0..n).rev() {
            self.bits.push(((v >> i) & 1) as u8);
        }
    }

    fn ue(&mut self, v: u32) {
        let v = v + 1;
        let n = 32 - v.leading_zeros();
        self.u(n - 1, 0);
        self.u(n, v);
    }

    fn copy(&mut self, r: &Reader, end: usize) -> Option<()> {
        for p in 0..end {
            self.bits.push(r.bit(p)? as u8);
        }
        Some(())
    }

    fn trailing(mut self) -> Vec<u8> {
        self.bits.push(1);
        while self.bits.len() % 8 != 0 {
            self.bits.push(0);
        }
        self.bits
            .chunks(8)
            .map(|c| c.iter().fold(0u8, |a, b| (a << 1) | b))
            .collect()
    }
}

fn scaling_list(r: &mut Reader, size: usize) -> Option<()> {
    let (mut last, mut next) = (8i32, 8i32);
    for _ in 0..size {
        if next != 0 {
            next = (last + r.se()? + 256).rem_euclid(256);
        }
        if next != 0 {
            last = next;
        }
    }
    Some(())
}

fn hrd(r: &mut Reader) -> Option<()> {
    let cnt = r.ue()? + 1;
    r.u(8)?;
    for _ in 0..cnt {
        r.ue()?;
        r.ue()?;
        r.u(1)?;
    }
    r.u(20)?;
    Some(())
}

struct Sps {
    max_refs: u32,
    vui: bool,
    reorder: Option<u32>,
    vui_flag: usize,
    restriction_flag: usize,
}

fn parse(r: &mut Reader) -> Option<Sps> {
    let profile = r.u(8)?;
    r.u(16)?;
    r.ue()?;
    if HIGH_PROFILES.contains(&profile) {
        let chroma = r.ue()?;
        if chroma == 3 {
            r.u(1)?;
        }
        r.ue()?;
        r.ue()?;
        r.u(1)?;
        if r.u(1)? == 1 {
            for i in 0..if chroma == 3 { 12 } else { 8 } {
                if r.u(1)? == 1 {
                    scaling_list(r, if i < 6 { 16 } else { 64 })?;
                }
            }
        }
    }
    r.ue()?;
    match r.ue()? {
        0 => {
            r.ue()?;
        }
        1 => {
            r.u(1)?;
            r.se()?;
            r.se()?;
            for _ in 0..r.ue()? {
                r.se()?;
            }
        }
        _ => {}
    }
    let max_refs = r.ue()?;
    r.u(1)?;
    r.ue()?;
    r.ue()?;
    if r.u(1)? == 0 {
        r.u(1)?;
    }
    r.u(1)?;
    if r.u(1)? == 1 {
        for _ in 0..4 {
            r.ue()?;
        }
    }
    let mut s = Sps {
        max_refs,
        vui: false,
        reorder: None,
        vui_flag: r.pos,
        restriction_flag: 0,
    };
    if r.u(1)? == 1 {
        s.vui = true;
        if r.u(1)? == 1 && r.u(8)? == 255 {
            r.u(32)?;
        }
        if r.u(1)? == 1 {
            r.u(1)?;
        }
        if r.u(1)? == 1 {
            r.u(4)?;
            if r.u(1)? == 1 {
                r.u(24)?;
            }
        }
        if r.u(1)? == 1 {
            r.ue()?;
            r.ue()?;
        }
        if r.u(1)? == 1 {
            r.u(32)?;
            r.u(32)?;
            r.u(1)?;
        }
        let nal = r.u(1)? == 1;
        if nal {
            hrd(r)?;
        }
        let vcl = r.u(1)? == 1;
        if vcl {
            hrd(r)?;
        }
        if nal || vcl {
            r.u(1)?;
        }
        r.u(1)?;
        s.restriction_flag = r.pos;
        if r.u(1)? == 1 {
            r.u(1)?;
            for _ in 0..4 {
                r.ue()?;
            }
            s.reorder = Some(r.ue()?);
            r.ue()?;
        }
    }
    Some(s)
}

/// The SPS with max_num_reorder_frames = 0 (unchanged if it already says so or does not parse).
pub fn low_latency_sps(nal: &[u8]) -> Vec<u8> {
    if nal.first().map(|b| b & 0x1F) != Some(7) {
        return nal.to_vec();
    }
    let rbsp = unescape(&nal[1..]);
    let mut r = Reader { d: &rbsp, pos: 0 };
    let Some(s) = parse(&mut r) else {
        return nal.to_vec();
    };
    if s.reorder == Some(0) {
        return nal.to_vec();
    }
    let mut w = Writer::default();
    if !s.vui {
        w.copy(&r, s.vui_flag);
        w.u(1, 1); // vui_parameters_present_flag
        w.u(8, 0); // aspect, overscan, video signal, chroma loc, timing, nal/vcl hrd, pic_struct
    } else {
        w.copy(&r, s.restriction_flag);
    }
    w.u(1, 1); // bitstream_restriction_flag
    w.u(1, 1); // motion_vectors_over_pic_boundaries_flag
    w.ue(2);
    w.ue(1);
    w.ue(16);
    w.ue(16);
    w.ue(0); // max_num_reorder_frames
    w.ue(s.max_refs.max(1)); // max_dec_frame_buffering
    [vec![nal[0]], escape(&w.trailing())].concat()
}

/// avcC with every SPS made low-latency.
pub fn rewrite_avcc(avcc: &[u8]) -> Vec<u8> {
    if avcc.len() < 7 || avcc[0] != 1 {
        return avcc.to_vec();
    }
    let mut out = avcc[..6].to_vec();
    let mut pos = 6;
    for _ in 0..(avcc[5] & 0x1F) {
        let Some(size) = avcc
            .get(pos..pos + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
        else {
            return avcc.to_vec();
        };
        let Some(sps) = avcc.get(pos + 2..pos + 2 + size) else {
            return avcc.to_vec();
        };
        pos += 2 + size;
        let sps = low_latency_sps(sps);
        out.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        out.extend(sps);
    }
    out.extend_from_slice(&avcc[pos..]); // PPS and any high-profile extension
    out
}

/// In-band SPS NAL units of a length-prefixed access unit rewritten.
pub fn rewrite_access_unit(au: &[u8]) -> Option<Vec<u8>> {
    let (mut pos, mut out, mut changed) = (0, Vec::with_capacity(au.len() + 8), false);
    while pos + 5 <= au.len() {
        let size = u32::from_be_bytes(au[pos..pos + 4].try_into().ok()?) as usize;
        let end = pos + 4 + size;
        if size == 0 || end > au.len() {
            return None;
        }
        let mut nal = au[pos + 4..end].to_vec();
        if nal[0] & 0x1F == 7 {
            let new = low_latency_sps(&nal);
            changed |= new != nal;
            nal = new;
        }
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend(nal);
        pos = end;
    }
    changed.then_some(out)
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
    fn matches_h264_py() {
        let sps = hex("6764001facd00f0044c8");
        let want = hex("6764001facd00f0044c900da0884603c20");
        assert_eq!(low_latency_sps(&sps), want);
        assert_eq!(low_latency_sps(&want), want); // already low latency
        let avcc = [&hex("0164001fffe1000a")[..], &sps, &hex("01000468ee3c80")].concat();
        assert_eq!(
            rewrite_avcc(&avcc),
            hex("0164001fffe100116764001facd00f0044c900da0884603c2001000468ee3c80")
        );
        assert_eq!(low_latency_sps(&[0x67, 0x64]), vec![0x67, 0x64]); // truncated: unchanged
    }
}
