//! Keyboard shortcuts of the window (B-26), for both platforms.
//!
//! Ported from the classic web UI (`ui/app.js`, `ui/keys.js`) so the Rust
//! window has the set the README and Settings list. Pure logic: `route`
//! decides, `main.rs` performs. F11 / Esc (full screen) stay in Slint.
//!
//! Every app shortcut uses Alt or Ctrl (owner, 2026-11-07): plain keys
//! (Space, M...) always go to the device, so typing never stops the
//! mirroring or mutes the sound by accident.

/// What a key press does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// Alt+M: sound on the PC on / off.
    Mute,
    /// Ctrl+, : open / close Settings.
    Settings,
    /// Alt+H: rotation Auto → 90° → 270°.
    Rotate,
    /// Alt+Left: Back (Apple: swipe from the left edge).
    Back,
    /// Alt+L: lock (Android: power button).
    Lock,
    /// Alt+C: copy the text on the device screen to the PC (Windows OCR).
    CopyText,
    /// Alt+R: forget the copied-text history (the next Alt+C starts over).
    ClearText,
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
            ('m', _) => Some(Shortcut::Mute),
            ('c', _) => Some(Shortcut::CopyText),
            (LEFT, _) => Some(Shortcut::Back),
            ('l', _) => Some(Shortcut::Lock),
            ('u', false) => Some(Shortcut::Apple("wake")),
            ('d', false) => Some(Shortcut::Apple("stealth")),
            ('v', false) => Some(Shortcut::Apple("paste-pc")),
            ('n', true) => Some(Shortcut::Android("notifications")),
            // Like scrcpy: MOD+S = app switch; Quick Settings on Q.
            ('q', true) => Some(Shortcut::Android("quick-settings")),
            ('s', true) => Some(Shortcut::Android("switcher")),
            ('r', _) => Some(Shortcut::ClearText),
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
    let _ = typing; // plain keys are never shortcuts
    None
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

/// A 32-bit top-down BMP (BGRA rows) for Windows OCR, which reads any
/// image Windows can decode; BMP needs no encoder in ZenLink.
pub fn bmp(width: u32, height: u32, bgra: &[u8]) -> Vec<u8> {
    let size = 54 + bgra.len() as u32;
    let mut b = Vec::with_capacity(size as usize);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(width as i32).to_le_bytes());
    b.extend_from_slice(&(-(height as i32)).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    b.extend_from_slice(&(bgra.len() as u32).to_le_bytes());
    b.extend_from_slice(&2835u32.to_le_bytes());
    b.extend_from_slice(&2835u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(bgra);
    b
}

/// Standard base64 (RFC 4648), for the OCR request body.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_keys_are_never_shortcuts_and_alt_m_mutes() {
        for typing in [false, true] {
            for android in [false, true] {
                assert_eq!(route(android, " ", false, false, false, typing), None);
                assert_eq!(route(android, "m", false, false, false, typing), None);
                assert_eq!(
                    route(android, "m", false, false, true, typing),
                    Some(Shortcut::Mute)
                );
                assert_eq!(
                    route(android, "M", false, true, true, typing),
                    Some(Shortcut::Mute)
                );
            }
        }
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
    fn copy_text_on_both_platforms() {
        assert_eq!(
            route(false, "c", false, false, true, true),
            Some(Shortcut::CopyText)
        );
        assert_eq!(
            route(true, "c", false, false, true, true),
            Some(Shortcut::CopyText)
        );
        assert_eq!(
            route(false, "r", false, false, true, true),
            Some(Shortcut::ClearText)
        );
        assert_eq!(
            route(true, "r", false, false, true, true),
            Some(Shortcut::ClearText)
        );
        assert_eq!(
            route(true, "s", false, false, true, true),
            Some(Shortcut::Android("switcher"))
        );
        assert_eq!(
            route(true, "q", false, false, true, true),
            Some(Shortcut::Android("quick-settings"))
        );
    }

    #[test]
    fn bmp_has_the_right_header_and_size() {
        let px = vec![1u8, 2, 3, 255, 4, 5, 6, 255];
        let b = bmp(2, 1, &px);
        assert_eq!(&b[..2], b"BM");
        assert_eq!(
            u32::from_le_bytes(b[2..6].try_into().unwrap()) as usize,
            b.len()
        );
        assert_eq!(b.len(), 54 + 8);
        // Top-down rows: negative height.
        assert_eq!(i32::from_le_bytes(b[22..26].try_into().unwrap()), -1);
        assert_eq!(&b[54..], &px[..]);
    }

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
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
