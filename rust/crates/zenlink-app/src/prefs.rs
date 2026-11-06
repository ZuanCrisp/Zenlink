//! Settings, saved as JSON in %APPDATA%\ZenLink-Beta\next-prefs.json.
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
        auto_start: false,
        close_to_tray: false,
        on_top: false,
        stats: false,
        dots: true,
    }
}

pub fn load() -> Prefs {
    let mut p = defaults();
    let Some(v) = path().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
        return p;
    };
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
    s("rotation", &mut p.rotation, &["auto", "0", "90", "180", "270"]);
    f("cropL", &mut p.crop_l, 0.0, 400.0);
    f("cropT", &mut p.crop_t, 0.0, 400.0);
    f("cropR", &mut p.crop_r, 0.0, 400.0);
    f("cropB", &mut p.crop_b, 0.0, 400.0);
    s("audioOut", &mut p.audio_out, &["pc", "device"]);
    f("volume", &mut p.volume, 0.0, 100.0);
    b("lipSync", &mut p.lip_sync);
    s("audioBuffer", &mut p.audio_buffer, &["low", "normal", "high"]);
    b("touch", &mut p.touch);
    b("keyboard", &mut p.keyboard);
    b("gestures", &mut p.gestures);
    b("autoStart", &mut p.auto_start);
    b("closeToTray", &mut p.close_to_tray);
    b("onTop", &mut p.on_top);
    b("stats", &mut p.stats);
    b("dots", &mut p.dots);
    p
}

pub fn save(p: &Prefs) {
    let v = json!({
        "version": 2, "theme": p.theme.as_str(), "glassOn": p.glass_on, "glassThick": p.glass, "lens": p.lens, "lang": p.lang.as_str(), "fit": p.fit.as_str(),
        "fitWindow": p.fit_window, "rotation": p.rotation.as_str(),
        "cropL": p.crop_l, "cropT": p.crop_t, "cropR": p.crop_r, "cropB": p.crop_b,
        "audioOut": p.audio_out.as_str(), "volume": p.volume, "lipSync": p.lip_sync,
        "audioBuffer": p.audio_buffer.as_str(), "touch": p.touch, "keyboard": p.keyboard,
        "gestures": p.gestures, "autoStart": p.auto_start, "closeToTray": p.close_to_tray,
        "onTop": p.on_top, "stats": p.stats, "dots": p.dots,
    });
    if let Some(f) = path() {
        let _ = std::fs::write(f, serde_json::to_vec_pretty(&v).unwrap_or_default());
    }
}
