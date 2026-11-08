//! The scrcpy server ZenLink bundles.
//!
//! The server is started with the client version as its first argument and
//! refuses to run when it does not match exactly; the protocol has no
//! backward or forward compatibility (scrcpy `doc/develop.md`). Bumping the
//! version is a deliberate step: read the upstream changelog and protocol
//! notes, replace `ZenshiroDev/resources/third_party/scrcpy/scrcpy-server-v<VERSION>`, update the
//! checksum here and in `ZenshiroDev/docs/THIRD_PARTY.md`, then run the protocol tests.

/// Official release: https://github.com/Genymobile/scrcpy/releases/tag/v5.0.1
pub const SCRCPY_VERSION: &str = "5.0.1";

/// File name of the bundled server, as published in the release assets.
pub const SERVER_FILE: &str = "scrcpy-server-v5.0.1";

/// SHA-256 of the official `scrcpy-server-v5.0.1` asset (SHA256SUMS.txt,
/// signed by Romain Vimont, key 4569 58E8 5A18 5DD5 C2D1 E4E8 0C82 2B29 8461 FA03).
pub const SERVER_SHA256: &str = "764eb6f79811d5211fe9df341120882ba9994c7a61b897d7bf3fb662e53bc536";

/// Size in bytes of the official asset.
pub const SERVER_SIZE: u64 = 733_930;

/// Where the server is pushed on the device.
pub const DEVICE_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";

/// Oldest Android API level scrcpy supports (Android 5.0).
pub const MIN_SDK: u32 = 21;

/// Oldest Android API level with audio capture (Android 11).
pub const MIN_SDK_AUDIO: u32 = 30;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn server_file_name_matches_version() {
        assert_eq!(SERVER_FILE, format!("scrcpy-server-v{SCRCPY_VERSION}"));
    }

    #[test]
    fn bundled_server_is_present_with_the_pinned_size() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/third_party/scrcpy")
            .join(SERVER_FILE);
        let meta = std::fs::metadata(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!(meta.len(), SERVER_SIZE);
    }

    #[test]
    fn checksum_is_hex_sha256() {
        assert_eq!(SERVER_SHA256.len(), 64);
        assert!(SERVER_SHA256.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
