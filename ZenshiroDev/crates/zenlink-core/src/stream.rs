//! GET /cast/stream.bin: records of `[u32 BE len][u8 type][len - 1 bytes]`.

use std::io::Read;

use crate::api::CodecInfo;
use crate::base_url;

#[derive(Debug)]
pub enum Record {
    /// Length-prefixed NAL units (AVCC / HVCC sample).
    Frame {
        key: bool,
        rebuild: bool,
        data: Vec<u8>,
    },
    /// New decoder configuration (rotation / size change).
    Config(CodecInfo),
}

pub struct StreamReader {
    reader: Box<dyn Read + Send>,
}

impl StreamReader {
    pub fn open() -> Result<Self, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        let r = agent
            .get(format!("{}/cast/stream.bin", base_url()))
            .call()
            .map_err(|e| e.to_string())?;
        if r.status() != 200 {
            return Err(format!("stream HTTP {}", r.status()));
        }
        Ok(Self {
            reader: Box::new(r.into_body().into_reader()),
        })
    }

    pub fn from_reader(reader: impl Read + Send + 'static) -> Self {
        Self {
            reader: Box::new(reader),
        }
    }

    /// Next record, `None` when the stream ends.
    pub fn next_record(&mut self) -> Option<Record> {
        loop {
            let mut head = [0u8; 4];
            self.reader.read_exact(&mut head).ok()?;
            let len = u32::from_be_bytes(head) as usize;
            if len == 0 || len > 64 << 20 {
                return None;
            }
            let mut buf = vec![0u8; len];
            self.reader.read_exact(&mut buf).ok()?;
            let kind = buf[0];
            let data = buf.split_off(1);
            match kind {
                3 => match CodecInfo::from_json(&data) {
                    Some(c) => return Some(Record::Config(c)),
                    None => continue,
                },
                0 | 2 => {
                    return Some(Record::Frame {
                        key: true,
                        rebuild: kind == 2,
                        data,
                    })
                }
                _ => {
                    return Some(Record::Frame {
                        key: false,
                        rebuild: false,
                        data,
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = ((payload.len() + 1) as u32).to_be_bytes().to_vec();
        v.push(kind);
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn parses_records() {
        let mut bytes = rec(0, &[9, 9]);
        bytes.extend(rec(1, &[7]));
        bytes.extend(rec(3, br#"{"codec":"avc1","description":"AQ=="}"#));
        bytes.extend(rec(2, &[]));
        let mut s = StreamReader::from_reader(std::io::Cursor::new(bytes));
        assert!(
            matches!(s.next_record(), Some(Record::Frame { key: true, rebuild: false, ref data }) if data == &[9, 9])
        );
        assert!(matches!(
            s.next_record(),
            Some(Record::Frame { key: false, .. })
        ));
        assert!(matches!(s.next_record(), Some(Record::Config(_))));
        assert!(matches!(
            s.next_record(),
            Some(Record::Frame {
                key: true,
                rebuild: true,
                ..
            })
        ));
        assert!(s.next_record().is_none());
    }
}
