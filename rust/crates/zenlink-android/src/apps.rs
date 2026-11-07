//! Installed Android apps, read with the scrcpy server (`list_apps=true`).
//!
//! The server prints `LogUtils.buildAppListMessage` (v5.0.1):
//!
//! ```text
//! [server] INFO: List of apps:
//!  * Settings                       com.android.settings
//!  - VLC                            org.videolan.vlc
//!  - A name of thirty characters or more
//!                                   com.example.long
//! ```
//!
//! `*` is a system app, `-` a user app. The name is padded to 30 columns and
//! followed by one space and the package; a name of 30 characters or more is
//! followed by a new line and the package indented past the column.
//!
//! An app is started on a display with the `START_APP` control message
//! ([`crate::control::Control::StartApp`]), by package name.

use crate::adb::{AdbClient, AdbError, Target};
use crate::scrcpy::{DEVICE_SERVER_PATH, SCRCPY_VERSION};

/// Oldest Android API level with a new virtual display (`new_display`):
/// the server refuses it below Android 10 (`Server.java`).
pub const MIN_SDK_NEW_DISPLAY: u32 = 29;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceApp {
    /// Label shown in the launcher.
    pub name: String,
    pub package: String,
    pub system: bool,
}

/// A Java package name (`com.example.app`): letters, digits, `_` and dots,
/// at least two segments, each starting with a letter. Only such names are
/// sent to the device.
pub fn is_package_name(s: &str) -> bool {
    s.len() <= 255
        && s.split('.').count() >= 2
        && s.split('.').all(|seg| {
            seg.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && seg.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
}

/// Parses the server's app list. Lines that do not fit the format are
/// skipped; a long name waits for its package on the next line.
pub fn parse_list(text: &str) -> Vec<DeviceApp> {
    let mut out = Vec::new();
    // A long name seen on the previous line, waiting for its package.
    let mut pending: Option<(String, bool)> = None;
    for line in text.lines() {
        let line = line.trim_end();
        let body = line.trim_start();
        let entry = body
            .strip_prefix("* ")
            .map(|r| (r, true))
            .or_else(|| body.strip_prefix("- ").map(|r| (r, false)));
        // Entries start with exactly one space (`"\n "` + marker).
        if let (Some((rest, system)), true) = (entry, line.starts_with(' ')) {
            pending = None;
            // Name padding is at least one space, plus the separator.
            match rest.rsplit_once("  ") {
                Some((name, pkg)) if is_package_name(pkg.trim()) && !name.trim().is_empty() => out
                    .push(DeviceApp {
                        name: name.trim().to_string(),
                        package: pkg.trim().to_string(),
                        system,
                    }),
                _ if !rest.trim().is_empty() => pending = Some((rest.trim().to_string(), system)),
                _ => {}
            }
            continue;
        }
        if let Some((name, system)) = pending.take() {
            if line.starts_with("    ") && is_package_name(body) {
                out.push(DeviceApp {
                    name,
                    package: body.to_string(),
                    system,
                });
            }
        }
    }
    out
}

/// The `adb shell` command that makes the server print the app list.
pub fn list_command() -> String {
    format!(
        "CLASSPATH={DEVICE_SERVER_PATH} app_process / com.genymobile.scrcpy.Server {SCRCPY_VERSION} \
         log_level=info list_apps=true"
    )
}

/// Pushes the server and asks it for the installed apps (a few seconds on
/// a phone with many apps).
pub fn query(
    client: &AdbClient,
    target: &Target,
    server: &[u8],
) -> Result<Vec<DeviceApp>, AdbError> {
    client.push(target, server, DEVICE_SERVER_PATH, 0o100644)?;
    let out = client.shell(target, &list_command(), 1 << 20)?;
    Ok(parse_list(&String::from_utf8_lossy(&out)))
}

/// Apps worth showing first: the user's own apps by name, then system apps
/// by name (the server lists system apps first).
pub fn sorted_for_picker(mut apps: Vec<DeviceApp>) -> Vec<DeviceApp> {
    apps.sort_by(|a, b| {
        a.system
            .cmp(&b.system)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.package.cmp(&b.package))
    });
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Built exactly like LogUtils.buildAppListMessage (column 30).
    fn line(name: &str, pkg: &str, system: bool) -> String {
        let mut s = String::from("\n ");
        s.push_str(if system { "* " } else { "- " });
        s.push_str(name);
        let padding = 30i32 - name.chars().count() as i32;
        if padding > 0 {
            s.push_str(&" ".repeat(padding as usize));
        } else {
            s.push_str("\n   ");
            s.push_str(&" ".repeat(30));
        }
        s.push(' ');
        s.push_str(pkg);
        s
    }

    fn sample() -> String {
        let mut s = String::from("[server] INFO: Device: [TECNO] TECNO TECNO LI7 (Android 14)\n");
        s.push_str("[server] INFO: List of apps:");
        s.push_str(&line("Settings", "com.android.settings", true));
        s.push_str(&line("Chrome", "com.android.chrome", true));
        s.push_str(&line("VLC", "org.videolan.vlc", false));
        s.push_str(&line(
            "A name with exactly thirty ch",
            "com.example.edge",
            false,
        ));
        s.push_str(&line(
            "An app name that is much too long",
            "com.example.long_name",
            false,
        ));
        s.push_str(&line("Ñame with ünïcode", "com.example.uni", false));
        s.push('\n');
        s
    }

    #[test]
    fn parses_short_long_and_unicode_names() {
        let apps = parse_list(&sample());
        let got: Vec<(&str, &str, bool)> = apps
            .iter()
            .map(|a| (a.name.as_str(), a.package.as_str(), a.system))
            .collect();
        assert_eq!(
            got,
            [
                ("Settings", "com.android.settings", true),
                ("Chrome", "com.android.chrome", true),
                ("VLC", "org.videolan.vlc", false),
                ("A name with exactly thirty ch", "com.example.edge", false),
                (
                    "An app name that is much too long",
                    "com.example.long_name",
                    false
                ),
                ("Ñame with ünïcode", "com.example.uni", false),
            ]
        );
    }

    #[test]
    fn name_of_exactly_thirty_moves_the_package_down() {
        let name = "x".repeat(30);
        let apps = parse_list(&format!("List of apps:{}", line(&name, "a.b", false)));
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, name);
        assert_eq!(apps[0].package, "a.b");
    }

    #[test]
    fn noise_and_errors_give_nothing() {
        assert!(parse_list("").is_empty());
        assert!(parse_list("Aborted\nERROR: Could not invoke method\n").is_empty());
        assert!(parse_list(" - Broken entry\nnot indented com.x.y\n").is_empty());
        // A package that could not be a package is not sent anywhere.
        assert!(parse_list(" - Evil                           com.x;reboot\n").is_empty());
    }

    #[test]
    fn package_names() {
        assert!(is_package_name("com.android.settings"));
        assert!(is_package_name("org.videolan.vlc"));
        assert!(is_package_name("com.example.long_name2"));
        for bad in [
            "", "settings", "com..x", "com.1x", "com.x y", "com.x;rm", ".com.x", "com.x.",
        ] {
            assert!(!is_package_name(bad), "{bad}");
        }
    }

    #[test]
    fn list_command_uses_the_pinned_version() {
        let c = list_command();
        assert!(c.contains(&format!(" {SCRCPY_VERSION} ")));
        assert!(c.ends_with("list_apps=true"));
    }

    #[test]
    fn picker_shows_user_apps_first() {
        let apps = sorted_for_picker(parse_list(&sample()));
        assert!(!apps[0].system);
        assert_eq!(apps[0].name, "A name with exactly thirty ch");
        assert!(apps.last().unwrap().system);
    }
}
