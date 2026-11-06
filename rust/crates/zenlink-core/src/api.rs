use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::base_url;

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
        let codec = if name.starts_with("avc") { Codec::H264 } else { Codec::Hevc };
        let description = base64::engine::general_purpose::STANDARD.decode(raw.description?).ok()?;
        if description.is_empty() {
            return None;
        }
        Some(Self { codec, description, keyframes_on_request: raw.keyframes_on_request.unwrap_or(true) })
    }
}

/// Short requests (state, input, settings). The video stream uses its own agent.
#[derive(Clone)]
pub struct Api {
    agent: ureq::Agent,
}

impl Default for Api {
    fn default() -> Self {
        Self::new()
    }
}

impl Api {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .http_status_as_error(false)
            .build()
            .into();
        Self { agent }
    }

    pub fn get(&self, path: &str) -> Option<Value> {
        let mut r = self.agent.get(format!("{}{path}", base_url())).call().ok()?;
        if r.status() != 200 {
            return None;
        }
        r.body_mut().read_json::<Value>().ok()
    }

    pub fn post(&self, path: &str, body: &Value) -> Option<Value> {
        let mut r = self.agent.post(format!("{}{path}", base_url())).send_json(body).ok()?;
        if !r.status().is_success() {
            return None;
        }
        Some(r.body_mut().read_json::<Value>().unwrap_or(Value::Null))
    }

    /// POST for work that takes long (driver changes, Wi-Fi setup). The JSON
    /// body comes back also for error statuses: they carry the message.
    pub fn post_slow(&self, path: &str, body: &Value, secs: u64) -> Option<Value> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(secs)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut r = agent.post(format!("{}{path}", base_url())).send_json(body).ok()?;
        r.body_mut().read_json::<Value>().ok()
    }

    pub fn alive(&self) -> bool {
        self.get("/api/state").is_some()
    }

    pub fn state(&self) -> Option<Value> {
        self.get("/api/state")
    }

    pub fn codec(&self) -> Option<CodecInfo> {
        let mut r = self.agent.get(format!("{}/cast/codec", base_url())).call().ok()?;
        if r.status() != 200 {
            return None;
        }
        let bytes = r.body_mut().read_to_vec().ok()?;
        CodecInfo::from_json(&bytes)
    }

    pub fn start_cast(&self) -> bool {
        self.post("/api/cast/start", &json!({})).is_some()
    }

    pub fn stop_cast(&self) -> bool {
        self.post("/api/cast/stop", &json!({})).is_some()
    }

    pub fn request_keyframe(&self) {
        let _ = self.post("/cast/pli", &json!({}));
    }

    /// `x`, `y` are 0..=65535 in the device's touch space.
    pub fn touch(&self, kind: TouchKind, x: u16, y: u16) {
        let t = match kind {
            TouchKind::Contact => "contact",
            TouchKind::Release => "release",
        };
        let _ = self.post("/cast/touch", &json!({ "type": t, "x": x, "y": y }));
    }

    pub fn button(&self, name: &str) {
        let _ = self.post("/cast/button", &json!({ "name": name, "state": "press" }));
    }

    pub fn key(&self, payload: &Value) {
        let _ = self.post("/cast/key", payload);
    }

    pub fn keyboard_action(&self, action: &str) {
        let _ = self.post("/api/keyboard/action", &json!({ "action": action }));
    }

    pub fn audio(&self, settings: &Value) {
        let _ = self.post("/api/audio", settings);
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
        let c = CodecInfo::from_json(br#"{"codec":"hvc1.1.6.L153.B0 ","description":"AQID","keyframesOnRequest":false}"#).unwrap();
        assert_eq!(c.codec, Codec::Hevc);
        assert_eq!(c.description, vec![1, 2, 3]);
        assert!(!c.keyframes_on_request);
        assert_eq!(CodecInfo::from_json(br#"{"codec":"avc1.640028","description":"AQ=="}"#).unwrap().codec, Codec::H264);
    }

    #[test]
    fn rotation_points() {
        assert_eq!(device_point(0.0, 0.0, 0), (0, 0));
        assert_eq!(device_point(1.0, 0.0, 90), (0, 0));
        assert_eq!(device_point(0.0, 0.0, 180), (65535, 65535));
        assert_eq!(device_point(0.0, 1.0, 270), (0, 0));
    }
}
