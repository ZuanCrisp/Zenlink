//! Settings, saved as JSON in %APPDATA%\ZenLink-Next\next-prefs.json.
//! The stable v2.2.31 keeps %APPDATA%\ZenLink; the beta never touches it.

use serde_json::{json, Value};

use crate::Prefs;

fn path() -> Option<std::path::PathBuf> {
    // Tests use their own file so they never change the user's settings.
    if let Some(p) = std::env::var_os("ZL_PREFS") {
        return Some(p.into());
    }
    let base = std::env::var_os("APPDATA")?;
    let dir = zenlink_core::data_dir_in(std::path::Path::new(&base));
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("next-prefs.json"))
}

pub fn defaults() -> Prefs {
    Prefs {
        theme: "dark".into(),
        glass_on: true,
        glass: 30.0,
        lens: true,
        lang: "id".into(),
        fit: "contain".into(),
        fit_window: true,
        rotation: "auto".into(),
        crop_l: 0.0,
        crop_t: 0.0,
        crop_r: 0.0,
        crop_b: 0.0,
        audio_out: "pc".into(),
        volume: 100.0,
        lip_sync: true,
        audio_buffer: "low".into(),
        touch: true,
        keyboard: true,
        gestures: true,
        start_with_windows: false,
        close_to_tray: false,
        on_top: false,
        stats: false,
        dots: true,
        platform: "".into(),
        remember_platform: false,
        android_max_size: "1920".into(),
        android_fps: "60".into(),
        android_bitrate: "8".into(),
        android_codec: "auto".into(),
        android_profile: "balanced".into(),
        android_encoder: "".into(),
    }
}

pub fn load() -> Prefs {
    match path()
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| parse(&b))
    {
        Some(v) => from_value(&v),
        None => defaults(),
    }
}

/// Parses the file, also when an editor saved it with a UTF-8 byte order
/// mark (serde_json rejects one, which would silently reset every setting).
fn parse(bytes: &[u8]) -> Option<Value> {
    let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    serde_json::from_slice(b).ok()
}

/// Reads a saved file; unknown or invalid values keep their defaults, so
/// files from older versions load without losing language, theme or others.
pub fn from_value(v: &Value) -> Prefs {
    let mut p = defaults();
    let s = |k: &str, d: &mut slint::SharedString, allowed: &[&str]| {
        if let Some(x) = v.get(k).and_then(Value::as_str) {
            if allowed.contains(&x) {
                *d = x.into();
            }
        }
    };
    let f = |k: &str, d: &mut f32, lo: f32, hi: f32| {
        if let Some(x) = v.get(k).and_then(Value::as_f64) {
            *d = (x as f32).clamp(lo, hi);
        }
    };
    let b = |k: &str, d: &mut bool| {
        if let Some(x) = v.get(k).and_then(Value::as_bool) {
            *d = x;
        }
    };
    s("theme", &mut p.theme, &["light", "dark", "system"]);
    b("glassOn", &mut p.glass_on);
    // Version 2 made 30% the default thickness; older files saved 0%.
    if v.get("version").and_then(Value::as_u64).unwrap_or(1) >= 2 {
        f("glassThick", &mut p.glass, 0.0, 100.0);
    }
    b("lens", &mut p.lens);
    s("lang", &mut p.lang, &["id", "en"]);
    s("fit", &mut p.fit, &["contain", "cover", "fill"]);
    b("fitWindow", &mut p.fit_window);
    s(
        "rotation",
        &mut p.rotation,
        &["auto", "0", "90", "180", "270"],
    );
    f("cropL", &mut p.crop_l, 0.0, 400.0);
    f("cropT", &mut p.crop_t, 0.0, 400.0);
    f("cropR", &mut p.crop_r, 0.0, 400.0);
    f("cropB", &mut p.crop_b, 0.0, 400.0);
    s("audioOut", &mut p.audio_out, &["pc", "device"]);
    f("volume", &mut p.volume, 0.0, 100.0);
    b("lipSync", &mut p.lip_sync);
    s(
        "audioBuffer",
        &mut p.audio_buffer,
        &["low", "normal", "high"],
    );
    b("touch", &mut p.touch);
    b("keyboard", &mut p.keyboard);
    b("gestures", &mut p.gestures);
    // "autoStart" (start when plugged in) was never implemented; not read.
    b("startWithWindows", &mut p.start_with_windows);
    b("closeToTray", &mut p.close_to_tray);
    b("onTop", &mut p.on_top);
    b("stats", &mut p.stats);
    b("dots", &mut p.dots);
    // Device platform (phase A2). Separate from the backend's Apple
    // "Remember my choice" (rememberConnectionMode in settings.json).
    s("platform", &mut p.platform, &["apple", "android"]);
    b("rememberPlatform", &mut p.remember_platform);
    // Android picture (phase A4): scrcpy server options.
    s(
        "androidMaxSize",
        &mut p.android_max_size,
        &["1280", "1920", "2560", "0"],
    );
    s(
        "androidFps",
        &mut p.android_fps,
        &["30", "60", "90", "120", "144"],
    );
    s(
        "androidBitrate",
        &mut p.android_bitrate,
        &["4", "8", "16", "24", "40"],
    );
    s(
        "androidCodec",
        &mut p.android_codec,
        &["auto", "h264", "h265"],
    );
    // Phase A5: video profile and a specific encoder (from the phone's list).
    s(
        "androidProfile",
        &mut p.android_profile,
        &[
            "low-latency",
            "balanced",
            "high-quality",
            "streaming",
            "custom",
        ],
    );
    if v.get("androidProfile").is_none() {
        // Settings from before A5: the profile their values match.
        p.android_profile =
            zenlink_android::profiles::matching(&crate::devprofiles::picture_of(&p))
                .as_str()
                .into();
    }
    if let Some(e) = v.get("androidEncoder").and_then(Value::as_str) {
        if e.is_empty() || zenlink_android::mirror::is_encoder_name(e) {
            p.android_encoder = e.into();
        }
    }
    p
}

pub fn save(p: &Prefs) {
    if let Some(f) = path() {
        let _ = std::fs::write(
            f,
            serde_json::to_vec_pretty(&to_value(p)).unwrap_or_default(),
        );
    }
}

pub fn to_value(p: &Prefs) -> Value {
    json!({
        "version": 2, "theme": p.theme.as_str(), "glassOn": p.glass_on, "glassThick": p.glass, "lens": p.lens, "lang": p.lang.as_str(), "fit": p.fit.as_str(),
        "fitWindow": p.fit_window, "rotation": p.rotation.as_str(),
        "cropL": p.crop_l, "cropT": p.crop_t, "cropR": p.crop_r, "cropB": p.crop_b,
        "audioOut": p.audio_out.as_str(), "volume": p.volume, "lipSync": p.lip_sync,
        "audioBuffer": p.audio_buffer.as_str(), "touch": p.touch, "keyboard": p.keyboard,
        "gestures": p.gestures, "startWithWindows": p.start_with_windows, "closeToTray": p.close_to_tray,
        "onTop": p.on_top, "stats": p.stats, "dots": p.dots,
        "platform": p.platform.as_str(), "rememberPlatform": p.remember_platform,
        "androidMaxSize": p.android_max_size.as_str(), "androidFps": p.android_fps.as_str(),
        "androidBitrate": p.android_bitrate.as_str(), "androidCodec": p.android_codec.as_str(),
        "androidProfile": p.android_profile.as_str(), "androidEncoder": p.android_encoder.as_str(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_with_a_byte_order_mark_still_loads() {
        let v = parse(b"\xEF\xBB\xBF{\"startWithWindows\": true}").unwrap();
        assert!(from_value(&v).start_with_windows);
        assert!(parse(b"{}").is_some());
        assert!(parse(b"not json").is_none());
    }

    #[test]
    fn fresh_install_defaults() {
        let p = defaults();
        assert_eq!(p.lang.as_str(), "id", "Indonesian on a fresh install");
        assert_eq!(p.theme.as_str(), "dark", "Liquid Glass Dark by default");
        assert_eq!(p.platform.as_str(), "");
        assert!(!p.remember_platform);
    }

    #[test]
    fn old_file_keeps_language_and_theme_and_has_no_platform() {
        let old = json!({"version": 2, "theme": "light", "lang": "en", "glassThick": 55.0});
        let p = from_value(&old);
        assert_eq!(
            (p.theme.as_str(), p.lang.as_str(), p.glass),
            ("light", "en", 55.0)
        );
        assert_eq!(p.platform.as_str(), "");
        assert!(!p.remember_platform);
    }

    #[test]
    fn platform_round_trip_and_validation() {
        let mut p = defaults();
        p.platform = "android".into();
        p.remember_platform = true;
        let back = from_value(&to_value(&p));
        assert_eq!(back.platform.as_str(), "android");
        assert!(back.remember_platform);
        let bad = from_value(&json!({"platform": "windows-phone"}));
        assert_eq!(bad.platform.as_str(), "");
    }
}
