//! Where the decoder gets its video from. The Apple path reads the backend's
//! stream.bin; Android reads a scrcpy socket. Both hand the decoder the same
//! records (length-prefixed samples and avcC/hvcC configs), so there is one
//! decoder and one renderer for every device type.

use crate::api::{Api, CodecInfo};
use crate::stream::{Record, StreamReader};

/// A stream of decoder records. `None` ends the session.
pub trait RecordStream: Send {
    fn next_record(&mut self) -> Option<Record>;
}

impl RecordStream for StreamReader {
    fn next_record(&mut self) -> Option<Record> {
        StreamReader::next_record(self)
    }
}

/// Opens video sessions for the decoder thread. `open` may block for a short
/// while; `None` means "not ready yet" and the decoder retries after a pause.
pub trait VideoSource: Send {
    fn open(&mut self) -> Option<(CodecInfo, Box<dyn RecordStream>)>;

    /// Ask for a keyframe after a decode error. Only called when the codec
    /// info says the source supports it.
    fn request_keyframe(&self) {}

    /// `true` once the source will never produce another session (a finished
    /// recording, a closed device). The decoder thread then exits.
    fn finished(&self) -> bool {
        false
    }
}

/// The Python backend's cast stream (Apple devices).
pub struct BackendSource(pub Api);

impl VideoSource for BackendSource {
    fn open(&mut self) -> Option<(CodecInfo, Box<dyn RecordStream>)> {
        let info = self.0.codec()?;
        match StreamReader::open() {
            Ok(s) => Some((info, Box::new(s))),
            Err(e) => {
                log::warn!("stream: {e}");
                None
            }
        }
    }

    fn request_keyframe(&self) {
        self.0.request_keyframe();
    }
}
