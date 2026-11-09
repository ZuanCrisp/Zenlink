use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
}

#[derive(Clone, Debug)]
pub struct CodecInfo {
    pub codec: Codec,
    /// avcC / hvcC decoder configuration record.
    pub description: Vec<u8>,
    /// Apple mode answers POST /cast/pli with a keyframe; AirPlay and Studio cannot.
    pub keyframes_on_request: bool,
    /// The picture's real size (width, height) when the encoder pads it and the stream does
    /// not say so (iOS: 1206x2622 coded as 1216x2656 with no SPS window).
    pub display: Option<(u32, u32)>,
}

#[derive(Deserialize)]
struct RawCodec {
    codec: Option<String>,
    description: Option<String>,
    #[serde(rename = "keyframesOnRequest")]
    keyframes_on_request: Option<bool>,
}

impl CodecInfo {
    /// Parse the JSON of GET /cast/codec or of a type-3 stream record.
    pub fn from_json(bytes: &[u8]) -> Option<Self> {
        let raw: RawCodec = serde_json::from_slice(bytes).ok()?;
        let name = raw.codec?.trim().to_ascii_lowercase();
        let codec = if name.starts_with("avc") {
            Codec::H264
        } else {
            Codec::Hevc
        };
        let description = base64::engine::general_purpose::STANDARD
            .decode(raw.description?)
            .ok()?;
        if description.is_empty() {
            return None;
        }
        Some(Self {
            codec,
            description,
            keyframes_on_request: raw.keyframes_on_request.unwrap_or(true),
            display: None,
        })
    }
}

/// The backend's requests, answered in this process by the handler the app registers
/// (status code, JSON body). Before that every request fails.
pub type Handler = fn(method: &str, path: &str, body: Option<&Value>) -> (u16, Value);

static HANDLER: std::sync::OnceLock<Handler> = std::sync::OnceLock::new();

/// Serve the API in this process (the Python sidecar is gone).
pub fn serve(handler: Handler) {
    let _ = HANDLER.set(handler);
}

fn call(method: &str, path: &str, body: Option<&Value>) -> Option<(u16, Value)> {
    HANDLER.get().map(|h| h(method, path, body))
}

/// Short requests (state, input, settings).
#[derive(Clone, Default)]
pub struct Api;

impl Api {
    pub fn new() -> Self {
        Self
    }

    pub fn get(&self, path: &str) -> Option<Value> {
        call("GET", path, None)
            .filter(|(c, _)| *c == 200)
            .map(|(_, v)| v)
    }

    pub fn post(&self, path: &str, body: &Value) -> Option<Value> {
        call("POST", path, Some(body))
            .filter(|(c, _)| (200..300).contains(c))
            .map(|(_, v)| v)
    }

    /// POST for work that takes long (driver changes). The JSON body comes back also for
    /// error statuses: they carry the message.
    pub fn post_slow(&self, path: &str, body: &Value, _secs: u64) -> Option<Value> {
        call("POST", path, Some(body)).map(|(_, v)| v)
    }

    /// GET that may take a few seconds; the body comes back also for error statuses.
    pub fn get_slow(&self, path: &str, _secs: u64) -> Option<Value> {
        call("GET", path, None).map(|(_, v)| v)
    }

    pub fn alive(&self) -> bool {
        self.get("/api/state").is_some()
    }

    pub fn state(&self) -> Option<Value> {
        self.get("/api/state")
    }

    pub fn start_cast(&self) -> bool {
        self.post("/api/cast/start", &json!({})).is_some()
    }

    pub fn stop_cast(&self) -> bool {
        self.post("/api/cast/stop", &json!({})).is_some()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TouchKind {
    Contact,
    Release,
}

/// Map a point on the shown picture (u, v in 0..1, (0,0) top-left as the user
/// sees it) to device touch space for the given rotation (0/90/180/270).
/// Same transform as `devicePoint` in ui/app.js.
pub fn device_point(u: f32, v: f32, rotation: u32) -> (u16, u16) {
    let (x, y) = match rotation {
        90 => (v, 1.0 - u),
        180 => (1.0 - u, 1.0 - v),
        270 => (1.0 - v, u),
        _ => (u, v),
    };
    let c = |n: f32| (n.clamp(0.0, 1.0) * 65535.0).round() as u16;
    (c(x), c(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_json() {
        let c = CodecInfo::from_json(
            br#"{"codec":"hvc1.1.6.L153.B0 ","description":"AQID","keyframesOnRequest":false}"#,
        )
        .unwrap();
        assert_eq!(c.codec, Codec::Hevc);
        assert_eq!(c.description, vec![1, 2, 3]);
        assert!(!c.keyframes_on_request);
        assert_eq!(
            CodecInfo::from_json(br#"{"codec":"avc1.640028","description":"AQ=="}"#)
                .unwrap()
                .codec,
            Codec::H264
        );
    }

    #[test]
    fn rotation_points() {
        assert_eq!(device_point(0.0, 0.0, 0), (0, 0));
        assert_eq!(device_point(1.0, 0.0, 90), (0, 0));
        assert_eq!(device_point(0.0, 0.0, 180), (65535, 65535));
        assert_eq!(device_point(0.0, 1.0, 270), (0, 0));
    }
}
