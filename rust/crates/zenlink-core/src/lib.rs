//! Client side of the ZenLink backend protocol (docs/rust/AUDIT.md section 2).
//!
//! Stage 1 of ADR-0001: the backend is the Python sidecar on 127.0.0.1:47421
//! (the beta's port; the stable v2.2.31 keeps 47321, so both can run side by side).
//! Later stages replace the sidecar with Rust crates; this API stays.

use std::path::{Path, PathBuf};

pub mod api;
pub mod nal;
pub mod sidecar;
pub mod stream;

pub const HOST: &str = "127.0.0.1";

/// Backend port. The stable v2.2.31 uses 47321; the beta must never share it.
pub const PORT: u16 = 47421;

/// Product name for window titles and messages.
pub const APP_NAME: &str = "ZenLink Beta";

/// Folder name under %APPDATA% and %LOCALAPPDATA% for prefs, settings and the
/// unpacked backend. The stable build uses "ZenLink"; the beta never touches it.
pub const DATA_DIR_NAME: &str = "ZenLink-Beta";

/// Log file in %TEMP%. The stable build writes zenlink-next.log.
pub const LOG_FILE_NAME: &str = "zenlink-beta.log";

/// Backend URL. `ZENLINK_PORT` overrides the port (tests, a second instance).
pub fn base_url() -> String {
    let port = std::env::var("ZENLINK_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(PORT);
    format!("http://{HOST}:{port}")
}

/// The beta's data folder inside a base folder such as %APPDATA% or %LOCALAPPDATA%.
pub fn data_dir_in(base: &Path) -> PathBuf {
    base.join(DATA_DIR_NAME)
}

/// `%LOCALAPPDATA%\ZenLink-Beta`, when LOCALAPPDATA is set.
pub fn local_data_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|b| data_dir_in(Path::new(&b)))
}

#[cfg(test)]
mod isolation_tests {
    use super::*;

    #[test]
    fn beta_port_differs_from_stable() {
        assert_eq!(PORT, 47421);
        assert_ne!(PORT, 47321, "47321 belongs to the stable v2.2.31");
    }

    #[test]
    fn beta_names_carry_beta() {
        assert!(APP_NAME.contains("Beta"));
        assert!(DATA_DIR_NAME.contains("Beta"));
        assert!(LOG_FILE_NAME.contains("beta"));
        assert_ne!(LOG_FILE_NAME, "zenlink-next.log");
    }

    #[test]
    fn data_dir_is_beta_folder() {
        let base = Path::new("base");
        assert_eq!(data_dir_in(base), base.join("ZenLink-Beta"));
        assert_ne!(data_dir_in(base), base.join("ZenLink"));
    }
}
