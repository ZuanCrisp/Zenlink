//! Smart Encoder Detection (master prompt §5): list the phone's encoders with
//! the scrcpy server, decide which codecs ZenLink can really play, and plan a
//! short, bounded list of start attempts.
//!
//! Reference: scrcpy v5.0.1 `server/.../util/LogUtils.java`
//! `buildEncoderListMessage`. With `list_encoders=true` the server prints,
//! through `Ln.i` (prefix `[server] INFO: `), and exits without mirroring:
//!
//! ```text
//! [server] INFO: List of video encoders:
//!     --video-codec=h264 --video-encoder=c2.qti.avc.encoder        (hw) [vendor]
//!     --video-codec=h264 --video-encoder=OMX.google.h264.encoder   (sw) (alias for c2.android.avc.encoder)
//! [server] INFO: List of audio encoders:
//!     --audio-codec=opus --audio-encoder=c2.android.opus.encoder   (sw)
//! ```
//!
//! The `(hw|sw|hybrid)`, `[vendor]` and `(alias for …)` parts exist only on
//! Android 10+ (API 29). Older phones list just the codec and the name.

use crate::adb::{AdbClient, AdbError, Target};
use crate::scrcpy::{DEVICE_SERVER_PATH, SCRCPY_VERSION};

/// Video or audio encoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Video,
    Audio,
}

/// How the encoder runs, as Android reports it (API 29+).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Accel {
    /// `isHardwareAccelerated()`.
    Hardware,
    /// Neither software-only nor hardware-accelerated.
    Hybrid,
    /// `isSoftwareOnly()`.
    Software,
    /// Android 9 and older do not say.
    Unknown,
}

impl Accel {
    pub fn as_str(self) -> &'static str {
        match self {
            Accel::Hardware => "hw",
            Accel::Hybrid => "hybrid",
            Accel::Software => "sw",
            Accel::Unknown => "",
        }
    }

    /// Order for automatic choice: hardware first, software last. Unknown
    /// sits before software: on old phones the vendor encoder is listed
    /// first and is hardware in practice, but ZenLink never labels it so.
    fn rank(self) -> u8 {
        match self {
            Accel::Hardware => 0,
            Accel::Hybrid => 1,
            Accel::Unknown => 2,
            Accel::Software => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoder {
    pub kind: Kind,
    /// scrcpy codec name: `h264`, `h265`, `av1`, `vp8`, `vp9`, `opus`, `aac`, `flac`, `raw`.
    pub codec: String,
    /// MediaCodec name, e.g. `c2.qti.avc.encoder`.
    pub name: String,
    pub accel: Accel,
    pub vendor: bool,
    /// Canonical name when this entry is an alias of another encoder.
    pub alias_for: Option<String>,
}

/// Parses the server's encoder list (see the module docs). Unknown lines are
/// skipped, so a future format change degrades to "nothing detected".
pub fn parse_list(text: &str) -> Vec<Encoder> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("--") else {
            continue;
        };
        let (kind, rest) = if let Some(r) = rest.strip_prefix("video-codec=") {
            (Kind::Video, r)
        } else if let Some(r) = rest.strip_prefix("audio-codec=") {
            (Kind::Audio, r)
        } else {
            continue;
        };
        let Some((codec, rest)) = rest.split_once(' ') else {
            continue;
        };
        let flag = match kind {
            Kind::Video => "--video-encoder=",
            Kind::Audio => "--audio-encoder=",
        };
        let Some(rest) = rest.trim_start().strip_prefix(flag) else {
            continue;
        };
        let (name, tail) = match rest.split_once(char::is_whitespace) {
            Some((n, t)) => (n, t.trim()),
            None => (rest, ""),
        };
        if name.is_empty() || codec.is_empty() {
            continue;
        }
        let accel = if tail.starts_with("(hw)") {
            Accel::Hardware
        } else if tail.starts_with("(sw)") {
            Accel::Software
        } else if tail.starts_with("(hybrid)") {
            Accel::Hybrid
        } else {
            Accel::Unknown
        };
        let alias_for = tail
            .split_once("(alias for ")
            .and_then(|(_, a)| a.split_once(')'))
            .map(|(a, _)| a.trim().to_string())
            .filter(|a| !a.is_empty());
        out.push(Encoder {
            kind,
            codec: codec.to_string(),
            name: name.to_string(),
            accel,
            vendor: tail.contains("[vendor]"),
            alias_for,
        });
    }
    out
}

/// The `adb shell` command that makes the server print its encoder list.
pub fn list_command() -> String {
    format!(
        "CLASSPATH={DEVICE_SERVER_PATH} app_process / com.genymobile.scrcpy.Server {SCRCPY_VERSION} \
         log_level=info list_encoders=true"
    )
}

/// Pushes the server and asks it for the encoder list (about a second).
pub fn query(client: &AdbClient, target: &Target, server: &[u8]) -> Result<Vec<Encoder>, AdbError> {
    client.push(target, server, DEVICE_SERVER_PATH, 0o100644)?;
    let out = client.shell(target, &list_command(), 256 * 1024)?;
    Ok(parse_list(&String::from_utf8_lossy(&out)))
}

/// What the PC can decode (ZenLink's Media Foundation decoder).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decoders {
    pub h265: bool,
    /// No AV1 path in ZenLink's decoder yet: always false for now.
    pub av1: bool,
}

/// Video codecs ZenLink can play end to end: an encoder on the phone and a
/// decoder on the PC. H.264 first: it is the low-latency default and every
/// Android device has it. VP8/VP9 are never offered (no decoder path).
pub fn usable_codecs(list: &[Encoder], dec: Decoders) -> Vec<&'static str> {
    let has = |c: &str| list.iter().any(|e| e.kind == Kind::Video && e.codec == c);
    let mut out = vec!["h264"];
    if dec.h265 && has("h265") {
        out.push("h265");
    }
    if dec.av1 && has("av1") {
        out.push("av1");
    }
    out
}

/// Video encoders for `codec`, best first, without aliases (an alias is the
/// same encoder under an old name).
pub fn video_encoders<'a>(list: &'a [Encoder], codec: &str) -> Vec<&'a Encoder> {
    let mut v: Vec<&Encoder> = list
        .iter()
        .filter(|e| e.kind == Kind::Video && e.codec == codec && e.alias_for.is_none())
        .collect();
    // Stable sort: the server's order (MediaCodecList's, vendor first) breaks ties.
    v.sort_by_key(|e| e.accel.rank());
    v
}

/// One start attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub codec: &'static str,
    /// `None` = let the server pick its default encoder for the codec.
    pub encoder: Option<String>,
}

/// Most start attempts per session start (master prompt §5: no endless
/// retry loop). The server itself also downsizes on an encoder error
/// (`downsize_on_error`, on by default), which covers "reduce resolution".
pub const MAX_ATTEMPTS: usize = 4;

/// The user's choice in Settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preference {
    /// `auto`, `h264`, `h265` or `av1`.
    pub codec: String,
    /// A MediaCodec name, or empty for automatic.
    pub encoder: String,
}

/// Ordered start attempts for a preference: the chosen encoder, another
/// hardware encoder of the same codec, then H.264. `list` is `None` when the
/// encoder list could not be read; then only the codec is used.
///
/// `auto` means H.264 (master prompt §5: the low-latency default; newer is
/// not assumed faster). The user's preference itself is never changed: the
/// next session starts from it again.
pub fn plan(pref: &Preference, list: Option<&[Encoder]>, dec: Decoders) -> Vec<Attempt> {
    let wanted: &'static str = match pref.codec.as_str() {
        "h265" if dec.h265 => "h265",
        "av1" if dec.av1 => "av1",
        _ => "h264",
    };
    let mut out: Vec<Attempt> = Vec::new();
    let mut add = |a: Attempt| {
        if out.len() < MAX_ATTEMPTS && !out.contains(&a) {
            out.push(a);
        }
    };
    match list {
        Some(list) => {
            let codecs = usable_codecs(list, dec);
            let codec_order: Vec<&'static str> = if codecs.contains(&wanted) {
                let mut v = vec![wanted];
                if wanted != "h264" {
                    v.push("h264");
                }
                v
            } else {
                vec!["h264"]
            };
            for (n, codec) in codec_order.iter().enumerate() {
                let encs = video_encoders(list, codec);
                // The user's encoder first, when it belongs to this codec.
                if n == 0 && !pref.encoder.is_empty() {
                    if let Some(e) = encs.iter().find(|e| e.name == pref.encoder) {
                        add(Attempt {
                            codec,
                            encoder: Some(e.name.clone()),
                        });
                    }
                }
                // Then up to two non-software encoders, best first.
                for e in encs.iter().filter(|e| e.accel != Accel::Software).take(2) {
                    add(Attempt {
                        codec,
                        encoder: Some(e.name.clone()),
                    });
                }
                // The server's own default for the codec.
                add(Attempt {
                    codec,
                    encoder: None,
                });
            }
        }
        None => {
            add(Attempt {
                codec: wanted,
                encoder: None,
            });
            add(Attempt {
                codec: "h264",
                encoder: None,
            });
        }
    }
    // H.264 with the server default is always the last resort.
    let last = Attempt {
        codec: "h264",
        encoder: None,
    };
    if !out.contains(&last) {
        if out.len() == MAX_ATTEMPTS {
            out.pop();
        }
        out.push(last);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shape copied from the v5.0.1 format (LogUtils.java); names are typical
    // Codec2 / OMX names, not a claim about any one phone.
    const LIST_API29: &str = "\
[server] INFO: Device: [samsung] samsung SM-X000 (Android 14)
[server] INFO: List of video encoders:
    --video-codec=h264 --video-encoder=c2.exynos.h264.encoder             (hw) [vendor]
    --video-codec=h264 --video-encoder=OMX.google.h264.encoder            (sw) (alias for c2.android.avc.encoder)
    --video-codec=h264 --video-encoder=c2.android.avc.encoder             (sw)
    --video-codec=h265 --video-encoder=c2.exynos.hevc.encoder             (hw) [vendor]
    --video-codec=h265 --video-encoder=c2.android.hevc.encoder            (sw)
    --video-codec=av1 --video-encoder=c2.android.av1.encoder              (sw)
    --video-codec=vp8 --video-encoder=c2.android.vp8.encoder              (sw)
[server] INFO: List of audio encoders:
    --audio-codec=opus --audio-encoder=c2.android.opus.encoder            (sw)
    --audio-codec=aac --audio-encoder=c2.android.aac.encoder              (sw)
";

    const LIST_OLD: &str = "\
[server] INFO: List of video encoders:
    --video-codec=h264 --video-encoder=OMX.qcom.video.encoder.avc
    --video-codec=h264 --video-encoder=OMX.google.h264.encoder
[server] INFO: List of audio encoders:
";

    fn dec(h265: bool) -> Decoders {
        Decoders { h265, av1: false }
    }

    #[test]
    fn parses_the_api29_format() {
        let l = parse_list(LIST_API29);
        assert_eq!(l.len(), 9);
        assert_eq!(
            l[0],
            Encoder {
                kind: Kind::Video,
                codec: "h264".into(),
                name: "c2.exynos.h264.encoder".into(),
                accel: Accel::Hardware,
                vendor: true,
                alias_for: None,
            }
        );
        assert_eq!(l[1].accel, Accel::Software);
        assert_eq!(l[1].alias_for.as_deref(), Some("c2.android.avc.encoder"));
        assert!(!l[2].vendor);
        assert_eq!(l[7].kind, Kind::Audio);
        assert_eq!(l[7].codec, "opus");
    }

    #[test]
    fn parses_the_old_format_without_accel() {
        let l = parse_list(LIST_OLD);
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|e| e.accel == Accel::Unknown && !e.vendor));
        assert_eq!(l[0].name, "OMX.qcom.video.encoder.avc");
    }

    #[test]
    fn garbage_and_errors_give_nothing() {
        assert!(parse_list("").is_empty());
        assert!(
            parse_list("[server] ERROR: Could not list encoders\njava.lang.Exception").is_empty()
        );
        assert!(parse_list("    --video-codec=h264 --video-encoder=").is_empty());
        assert!(parse_list("    --video-codec=h264").is_empty());
    }

    #[test]
    fn list_command_uses_the_pinned_version() {
        let c = list_command();
        assert!(c.contains("com.genymobile.scrcpy.Server 5.0.1 "));
        assert!(c.ends_with("list_encoders=true"));
    }

    #[test]
    fn usable_codecs_need_both_ends() {
        let l = parse_list(LIST_API29);
        assert_eq!(usable_codecs(&l, dec(false)), vec!["h264"]);
        assert_eq!(usable_codecs(&l, dec(true)), vec!["h264", "h265"]);
        // AV1 encoder present, but no AV1 decoder in ZenLink: never offered.
        assert!(!usable_codecs(
            &l,
            Decoders {
                h265: true,
                av1: false
            }
        )
        .contains(&"av1"));
        // VP8 never.
        assert!(!usable_codecs(&l, dec(true)).contains(&"vp8"));
        // A phone without HEVC.
        assert_eq!(
            usable_codecs(&parse_list(LIST_OLD), dec(true)),
            vec!["h264"]
        );
    }

    #[test]
    fn encoders_sorted_hardware_first_without_aliases() {
        let l = parse_list(LIST_API29);
        let v: Vec<&str> = video_encoders(&l, "h264")
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(v, vec!["c2.exynos.h264.encoder", "c2.android.avc.encoder"]);
    }

    #[test]
    fn auto_plans_h264_hardware_then_default() {
        let l = parse_list(LIST_API29);
        let p = plan(
            &Preference {
                codec: "auto".into(),
                encoder: String::new(),
            },
            Some(&l),
            dec(true),
        );
        assert_eq!(
            p,
            vec![
                Attempt {
                    codec: "h264",
                    encoder: Some("c2.exynos.h264.encoder".into())
                },
                Attempt {
                    codec: "h264",
                    encoder: None
                },
            ]
        );
    }

    #[test]
    fn h265_falls_back_to_h264_within_the_bound() {
        let l = parse_list(LIST_API29);
        let p = plan(
            &Preference {
                codec: "h265".into(),
                encoder: String::new(),
            },
            Some(&l),
            dec(true),
        );
        assert_eq!(
            p[0],
            Attempt {
                codec: "h265",
                encoder: Some("c2.exynos.hevc.encoder".into())
            }
        );
        assert_eq!(
            p[1],
            Attempt {
                codec: "h265",
                encoder: None
            }
        );
        assert!(p.len() <= MAX_ATTEMPTS);
        assert_eq!(
            p.last().unwrap(),
            &Attempt {
                codec: "h264",
                encoder: None
            }
        );
        assert!(p.iter().any(|a| a.codec == "h264" && a.encoder.is_some()));
    }

    #[test]
    fn h265_without_a_decoder_or_encoder_is_h264() {
        let l = parse_list(LIST_API29);
        let p = plan(
            &Preference {
                codec: "h265".into(),
                encoder: String::new(),
            },
            Some(&l),
            dec(false),
        );
        assert!(p.iter().all(|a| a.codec == "h264"));
        let old = parse_list(LIST_OLD);
        let p = plan(
            &Preference {
                codec: "h265".into(),
                encoder: String::new(),
            },
            Some(&old),
            dec(true),
        );
        assert!(p.iter().all(|a| a.codec == "h264"));
    }

    #[test]
    fn chosen_software_encoder_is_tried_first() {
        let l = parse_list(LIST_API29);
        let p = plan(
            &Preference {
                codec: "h264".into(),
                encoder: "c2.android.avc.encoder".into(),
            },
            Some(&l),
            dec(true),
        );
        assert_eq!(p[0].encoder.as_deref(), Some("c2.android.avc.encoder"));
        assert_eq!(p[1].encoder.as_deref(), Some("c2.exynos.h264.encoder"));
    }

    #[test]
    fn unknown_encoder_name_is_ignored() {
        let l = parse_list(LIST_API29);
        let p = plan(
            &Preference {
                codec: "h264".into(),
                encoder: "gone.encoder".into(),
            },
            Some(&l),
            dec(true),
        );
        assert!(p
            .iter()
            .all(|a| a.encoder.as_deref() != Some("gone.encoder")));
    }

    #[test]
    fn without_a_list_only_codecs_are_tried() {
        let p = plan(
            &Preference {
                codec: "h265".into(),
                encoder: "x".into(),
            },
            None,
            dec(true),
        );
        assert_eq!(
            p,
            vec![
                Attempt {
                    codec: "h265",
                    encoder: None
                },
                Attempt {
                    codec: "h264",
                    encoder: None
                }
            ]
        );
        let p = plan(&Preference::default(), None, dec(true));
        assert_eq!(
            p,
            vec![Attempt {
                codec: "h264",
                encoder: None
            }]
        );
    }

    #[test]
    fn plan_is_always_bounded_and_ends_with_h264_default() {
        let mut text = String::from("List of video encoders:\n");
        for i in 0..10 {
            text.push_str(&format!(
                "    --video-codec=h265 --video-encoder=hw{i}  (hw)\n"
            ));
            text.push_str(&format!(
                "    --video-codec=h264 --video-encoder=a{i}  (hw)\n"
            ));
        }
        let l = parse_list(&text);
        let p = plan(
            &Preference {
                codec: "h265".into(),
                encoder: "hw7".into(),
            },
            Some(&l),
            dec(true),
        );
        assert_eq!(p.len(), MAX_ATTEMPTS);
        assert_eq!(p[0].encoder.as_deref(), Some("hw7"));
        assert_eq!(
            p.last().unwrap(),
            &Attempt {
                codec: "h264",
                encoder: None
            }
        );
    }
}
