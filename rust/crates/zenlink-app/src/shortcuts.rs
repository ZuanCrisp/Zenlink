//! Keyboard shortcuts of the window (B-26), for both platforms.
//!
//! Ported from the classic web UI (`ui/app.js`, `ui/keys.js`) so the Rust
//! window has the set the README and Settings list. Pure logic: `route`
//! decides, `main.rs` performs. F11 / Esc (full screen) stay in Slint.
//!
//! Rule from the classic UI: while the PC keyboard types on a live device,
//! plain keys (Space, M, G...) go to the device; only Alt / Ctrl shortcuts
//! stay with the app.

/// What a key press does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// Space: start / stop mirroring.
    StartStop,
    /// M: sound on the PC on / off.
    Mute,
    /// Ctrl+, : open / close Settings.
    Settings,
    /// Alt+H: rotation Auto → 90° → 270°.
    Rotate,
    /// Alt+Left: Back (Apple: swipe from the left edge).
    Back,
    /// Alt+L: lock (Android: power button).
    Lock,
    /// Apple Driver mode actions (`/api/keyboard/action`).
    Apple(&'static str),
    /// Android buttons (`android::Shared::action`).
    Android(&'static str),
}

/// Slint key codes (private-use characters).
const LEFT: char = '\u{F702}';
const UP: char = '\u{F700}';
const DOWN: char = '\u{F701}';

/// `typing`: the PC keyboard currently types on a live device.
pub fn route(
    android: bool,
    text: &str,
    ctrl: bool,
    shift: bool,
    alt: bool,
    typing: bool,
) -> Option<Shortcut> {
    let mut chars = text.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return None;
    };
    let lower = c.to_ascii_lowercase();
    if alt && !ctrl {
        return match (lower, android) {
            ('h', _) => Some(Shortcut::Rotate),
            (LEFT, _) => Some(Shortcut::Back),
            ('l', _) => Some(Shortcut::Lock),
            ('u', false) => Some(Shortcut::Apple("wake")),
            ('d', false) => Some(Shortcut::Apple("stealth")),
            ('v', false) => Some(Shortcut::Apple("paste-pc")),
            ('n', true) => Some(Shortcut::Android("notifications")),
            ('s', true) => Some(Shortcut::Android("quick-settings")),
            ('r', true) => Some(Shortcut::Android("switcher")),
            (UP, true) => Some(Shortcut::Android("vol-up")),
            (DOWN, true) => Some(Shortcut::Android("vol-down")),
            _ => None,
        };
    }
    if ctrl && !alt {
        if c == ',' {
            return Some(Shortcut::Settings);
        }
        // Apple: Ctrl+letter = Cmd+letter on the device; Ctrl+K keyboard.
        // (Android maps Ctrl+letter itself in `android::key_input`.)
        if !android && !shift {
            return match lower {
                'k' => Some(Shortcut::Apple("toggle-keyboard")),
                'c' => Some(Shortcut::Apple("copy")),
                'v' => Some(Shortcut::Apple("paste")),
                'x' => Some(Shortcut::Apple("cut")),
                'a' => Some(Shortcut::Apple("select-all")),
                'z' => Some(Shortcut::Apple("undo")),
                _ => None,
            };
        }
        return None;
    }
    if ctrl || alt || typing {
        return None;
    }
    match lower {
        ' ' => Some(Shortcut::StartStop),
        'm' => Some(Shortcut::Mute),
        _ => None,
    }
}

/// HID keyboard usage (and whether Shift is needed) for a typed character,
/// US layout, for the Apple hardware-keyboard path (`/cast/key`). The
/// iPhone's own layout setting decides the final character.
pub fn hid_usage(c: char) -> Option<(u8, bool)> {
    const SHIFTED_DIGITS: &str = "!@#$%^&*()";
    Some(match c {
        'a'..='z' => (0x04 + (c as u8 - b'a'), false),
        'A'..='Z' => (0x04 + (c as u8 - b'A'), true),
        '1'..='9' => (0x1e + (c as u8 - b'1'), false),
        '0' => (0x27, false),
        '\n' | '\r' => (0x28, false),
        '\u{1b}' => (0x29, false),
        '\u{8}' => (0x2a, false),
        '\t' => (0x2b, false),
        ' ' => (0x2c, false),
        '-' => (0x2d, false),
        '_' => (0x2d, true),
        '=' => (0x2e, false),
        '+' => (0x2e, true),
        '[' => (0x2f, false),
        '{' => (0x2f, true),
        ']' => (0x30, false),
        '}' => (0x30, true),
        '\\' => (0x31, false),
        '|' => (0x31, true),
        ';' => (0x33, false),
        ':' => (0x33, true),
        '\'' => (0x34, false),
        '"' => (0x34, true),
        '`' => (0x35, false),
        '~' => (0x35, true),
        ',' => (0x36, false),
        '<' => (0x36, true),
        '.' => (0x37, false),
        '>' => (0x37, true),
        '/' => (0x38, false),
        '?' => (0x38, true),
        '\u{7f}' => (0x4c, false),
        '\u{F729}' => (0x4a, false),
        '\u{F72B}' => (0x4d, false),
        '\u{F72C}' => (0x4b, false),
        '\u{F72D}' => (0x4e, false),
        '\u{F703}' => (0x4f, false),
        LEFT => (0x50, false),
        DOWN => (0x51, false),
        UP => (0x52, false),
        c => match SHIFTED_DIGITS.find(c) {
            // ! = Shift+1 ... ) = Shift+0
            Some(i) => (if i == 9 { 0x27 } else { 0x1e + i as u8 }, true),
            None => return None,
        },
    })
}

/// Left Shift HID usage.
pub const HID_SHIFT: u8 = 0xe1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_keys_are_app_shortcuts_only_when_not_typing() {
        assert_eq!(
            route(false, " ", false, false, false, false),
            Some(Shortcut::StartStop)
        );
        assert_eq!(
            route(true, "m", false, false, false, false),
            Some(Shortcut::Mute)
        );
        assert_eq!(route(true, " ", false, false, false, true), None);
        assert_eq!(route(false, "m", false, false, false, true), None);
    }

    #[test]
    fn alt_shortcuts_work_while_typing() {
        assert_eq!(
            route(false, "h", false, false, true, true),
            Some(Shortcut::Rotate)
        );
        assert_eq!(
            route(true, "H", false, true, true, true),
            Some(Shortcut::Rotate)
        );
        assert_eq!(
            route(true, "\u{F702}", false, false, true, true),
            Some(Shortcut::Back)
        );
        assert_eq!(
            route(false, "l", false, false, true, true),
            Some(Shortcut::Lock)
        );
        assert_eq!(
            route(true, "l", false, false, true, true),
            Some(Shortcut::Lock)
        );
        assert_eq!(
            route(true, ",", true, false, false, true),
            Some(Shortcut::Settings)
        );
    }

    #[test]
    fn platform_specific_shortcuts() {
        assert_eq!(
            route(false, "u", false, false, true, true),
            Some(Shortcut::Apple("wake"))
        );
        assert_eq!(route(true, "u", false, false, true, true), None);
        assert_eq!(
            route(true, "n", false, false, true, true),
            Some(Shortcut::Android("notifications"))
        );
        assert_eq!(route(false, "n", false, false, true, true), None);
        assert_eq!(
            route(false, "k", true, false, false, true),
            Some(Shortcut::Apple("toggle-keyboard"))
        );
        // Android Ctrl+letter goes to android::key_input, not here.
        assert_eq!(route(true, "c", true, false, false, true), None);
        assert_eq!(
            route(false, "c", true, false, false, true),
            Some(Shortcut::Apple("copy"))
        );
    }

    #[test]
    fn hid_usages_match_the_classic_table() {
        assert_eq!(hid_usage('a'), Some((0x04, false)));
        assert_eq!(hid_usage('Z'), Some((0x1d, true)));
        assert_eq!(hid_usage('1'), Some((0x1e, false)));
        assert_eq!(hid_usage('0'), Some((0x27, false)));
        assert_eq!(hid_usage('!'), Some((0x1e, true)));
        assert_eq!(hid_usage(')'), Some((0x27, true)));
        assert_eq!(hid_usage(' '), Some((0x2c, false)));
        assert_eq!(hid_usage('\u{8}'), Some((0x2a, false)));
        assert_eq!(hid_usage('?'), Some((0x38, true)));
        assert_eq!(hid_usage('\u{F702}'), Some((0x50, false)));
        assert_eq!(hid_usage('é'), None);
    }
}
