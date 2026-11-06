//! Video profiles (master prompt §6) and per-device settings (§7).
//!
//! Profiles are starting points, not guarantees: they fill in resolution,
//! FPS, bitrate and codec; the phone still caps FPS at its refresh rate and
//! the encoder plan (`encoders::plan`) still falls back when a codec fails.
//!
//! Per-device settings are keyed by the stable device identity
//! (`identity::DeviceId::key`, built from the hardware serial), never by the
//! ADB serial or IP address, so a phone keeps its settings when it moves
//! between USB and Wi-Fi. Storage (JSON) lives in the app; this module is the
//! portable model.

/// A video profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    LowLatency,
    Balanced,
    HighQuality,
    Streaming,
    Custom,
}

pub const PROFILES: [Profile; 5] = [
    Profile::LowLatency,
    Profile::Balanced,
    Profile::HighQuality,
    Profile::Streaming,
    Profile::Custom,
];

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::LowLatency => "low-latency",
            Profile::Balanced => "balanced",
            Profile::HighQuality => "high-quality",
            Profile::Streaming => "streaming",
            Profile::Custom => "custom",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        PROFILES.into_iter().find(|p| p.as_str() == s)
    }
}

/// The picture settings a profile decides. Strings as stored in the prefs
/// (`androidMaxSize` etc.): max size in pixels (`0` = native), FPS, bitrate
/// in Mbps, codec (`auto`, `h264`, `h265`), encoder (empty = automatic).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub max_size: String,
    pub fps: String,
    pub bitrate: String,
    pub codec: String,
    pub encoder: String,
}

impl Picture {
    fn new(max_size: &str, fps: &str, bitrate: &str, codec: &str) -> Self {
        Self {
            max_size: max_size.into(),
            fps: fps.into(),
            bitrate: bitrate.into(),
            codec: codec.into(),
            encoder: String::new(),
        }
    }
}

/// The values a profile sets. `None` for Custom (the user's own values).
///
/// - Low Latency: H.264 (lowest encode and decode delay), 1080p, 60 fps,
///   8 Mbps; a smaller frame keeps the encoder quick.
/// - Balanced (default): automatic codec (H.264 unless the user picks),
///   1080p, 60 fps, 8 Mbps.
/// - High Quality: H.265 when both ends support it (the plan falls back to
///   H.264 otherwise), native resolution, 60 fps, 24 Mbps.
/// - Streaming / OBS: H.264, 1080p, a steady 60 fps (OBS's usual rate) and
///   16 Mbps for fewer compression artefacts in a re-encoded stream.
pub fn picture(p: Profile) -> Option<Picture> {
    Some(match p {
        Profile::LowLatency => Picture::new("1920", "60", "8", "h264"),
        Profile::Balanced => Picture::new("1920", "60", "8", "auto"),
        Profile::HighQuality => Picture::new("0", "60", "24", "h265"),
        Profile::Streaming => Picture::new("1920", "60", "16", "h264"),
        Profile::Custom => return None,
    })
}

/// The profile whose values `pic` matches, Custom when none does (a manual
/// change turns the selection into Custom).
pub fn matching(pic: &Picture) -> Profile {
    PROFILES
        .into_iter()
        .find(|p| picture(*p).as_ref() == Some(pic))
        .unwrap_or(Profile::Custom)
}

/// The last start that worked: what to report and what "Automatic
/// optimization" can reuse. Never replaces the user's preference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LastGood {
    pub codec: String,
    pub encoder: String,
    pub width: u32,
    pub height: u32,
}

/// Everything ZenLink remembers about one Android device. No ADB keys, no
/// IP addresses, no account data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceProfile {
    /// `identity::DeviceId::key`.
    pub key: String,
    /// Optional nickname from the user (empty = use the detected name).
    pub nickname: String,
    /// Detected display name and how it was found (`naming` source).
    pub name: String,
    pub manufacturer: String,
    pub model: String,
    pub android: String,
    /// `usb` or `wifi`: how it was last mirrored.
    pub connection: String,
    pub profile: Profile,
    pub picture: Picture,
    pub last_good: Option<LastGood>,
}

impl DeviceProfile {
    /// A device seen for the first time: the Balanced profile.
    pub fn new(key: &str) -> Self {
        Self {
            key: key.into(),
            nickname: String::new(),
            name: String::new(),
            manufacturer: String::new(),
            model: String::new(),
            android: String::new(),
            connection: String::new(),
            profile: Profile::Balanced,
            picture: picture(Profile::Balanced).unwrap(),
            last_good: None,
        }
    }

    /// Switch profile; Custom keeps the current values.
    pub fn set_profile(&mut self, p: Profile) {
        self.profile = p;
        if let Some(pic) = picture(p) {
            self.picture = pic;
        }
    }

    /// Store manually edited values; the profile follows them.
    pub fn set_picture(&mut self, pic: Picture) {
        self.profile = matching(&pic);
        self.picture = pic;
    }

    /// "Reset this device": back to Balanced, nickname and history kept off.
    pub fn reset(&mut self) {
        let key = std::mem::take(&mut self.key);
        let keep = (
            std::mem::take(&mut self.name),
            std::mem::take(&mut self.manufacturer),
            std::mem::take(&mut self.model),
            std::mem::take(&mut self.android),
        );
        *self = Self::new(&key);
        (self.name, self.manufacturer, self.model, self.android) = keep;
    }
}

/// Every known device, in first-seen order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    pub devices: Vec<DeviceProfile>,
}

/// Most devices kept; the oldest unused ones go first.
pub const MAX_DEVICES: usize = 64;

impl Store {
    pub fn get(&self, key: &str) -> Option<&DeviceProfile> {
        self.devices.iter().find(|d| d.key == key)
    }

    /// The device's profile, created (Balanced) when it is new. A used
    /// device moves to the end, so trimming drops the longest-unused.
    pub fn entry(&mut self, key: &str) -> &mut DeviceProfile {
        let d = match self.devices.iter().position(|d| d.key == key) {
            Some(i) => self.devices.remove(i),
            None => DeviceProfile::new(key),
        };
        self.devices.push(d);
        while self.devices.len() > MAX_DEVICES {
            self.devices.remove(0);
        }
        self.devices.last_mut().unwrap()
    }

    pub fn remove(&mut self, key: &str) {
        self.devices.retain(|d| d.key != key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_names_round_trip() {
        for p in PROFILES {
            assert_eq!(Profile::parse(p.as_str()), Some(p));
        }
        assert_eq!(Profile::parse("turbo"), None);
    }

    #[test]
    fn every_fixed_profile_is_recognised_from_its_values() {
        for p in PROFILES {
            if let Some(pic) = picture(p) {
                assert_eq!(matching(&pic), p);
            }
        }
        // The profiles are distinct.
        let pics: Vec<_> = PROFILES.iter().filter_map(|p| picture(*p)).collect();
        for (i, a) in pics.iter().enumerate() {
            assert!(pics[i + 1..].iter().all(|b| a != b));
        }
    }

    #[test]
    fn low_latency_and_streaming_use_h264() {
        assert_eq!(picture(Profile::LowLatency).unwrap().codec, "h264");
        assert_eq!(picture(Profile::Streaming).unwrap().codec, "h264");
        assert_eq!(picture(Profile::Balanced).unwrap().codec, "auto");
    }

    #[test]
    fn manual_change_becomes_custom() {
        let mut d = DeviceProfile::new("android/ABC123456");
        assert_eq!(d.profile, Profile::Balanced);
        let mut pic = d.picture.clone();
        pic.bitrate = "40".into();
        d.set_picture(pic.clone());
        assert_eq!(d.profile, Profile::Custom);
        assert_eq!(d.picture, pic);
        // Custom keeps the values.
        d.set_profile(Profile::Custom);
        assert_eq!(d.picture, pic);
        // Picking a value set that equals a profile selects it.
        d.set_picture(picture(Profile::Streaming).unwrap());
        assert_eq!(d.profile, Profile::Streaming);
    }

    #[test]
    fn devices_do_not_overwrite_each_other() {
        let mut s = Store::default();
        s.entry("android/AAA111222")
            .set_profile(Profile::HighQuality);
        s.entry("android/BBB333444")
            .set_profile(Profile::LowLatency);
        assert_eq!(
            s.get("android/AAA111222").unwrap().profile,
            Profile::HighQuality
        );
        assert_eq!(
            s.get("android/BBB333444").unwrap().profile,
            Profile::LowLatency
        );
        assert_eq!(s.devices.len(), 2);
    }

    #[test]
    fn same_key_from_usb_and_wifi_shares_one_profile() {
        // identity::identify gives the same key on both transports (tested in
        // identity.rs); here the store must reuse the entry, not add one.
        let mut s = Store::default();
        let e = s.entry("android/R5CX10ABCDE");
        e.connection = "usb".into();
        e.set_profile(Profile::Streaming);
        let e = s.entry("android/R5CX10ABCDE");
        e.connection = "wifi".into();
        assert_eq!(e.profile, Profile::Streaming);
        assert_eq!(s.devices.len(), 1);
    }

    #[test]
    fn store_is_bounded_and_drops_the_longest_unused() {
        let mut s = Store::default();
        for i in 0..MAX_DEVICES + 5 {
            s.entry(&format!("android/DEV{i:06}"));
            if i == 2 {
                continue;
            }
            // Keep device 0 in use.
            s.entry("android/DEV000000");
        }
        assert_eq!(s.devices.len(), MAX_DEVICES);
        assert!(s.get("android/DEV000000").is_some());
        assert!(s.get("android/DEV000001").is_none());
    }

    #[test]
    fn reset_goes_back_to_balanced_but_keeps_the_detected_name() {
        let mut d = DeviceProfile::new("android/ABC123456");
        d.name = "Pixel 8".into();
        d.nickname = "Mine".into();
        d.set_profile(Profile::HighQuality);
        d.last_good = Some(LastGood::default());
        d.reset();
        assert_eq!(d.key, "android/ABC123456");
        assert_eq!(d.name, "Pixel 8");
        assert_eq!(d.nickname, "");
        assert_eq!(d.profile, Profile::Balanced);
        assert_eq!(d.last_good, None);
    }
}
