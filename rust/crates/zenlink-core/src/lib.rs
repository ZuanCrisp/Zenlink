//! Client side of the ZenLink backend protocol (docs/rust/AUDIT.md section 2).
//!
//! Stage 1 of ADR-0001: the backend is the Python sidecar on 127.0.0.1:47321.
//! Later stages replace the sidecar with Rust crates; this API stays.

pub mod api;
pub mod nal;
pub mod sidecar;
pub mod stream;

pub const HOST: &str = "127.0.0.1";
pub const PORT: u16 = 47321;

/// Backend URL. `ZENLINK_PORT` overrides the port (tests, a second instance).
pub fn base_url() -> String {
    let port = std::env::var("ZENLINK_PORT").ok().and_then(|p| p.parse::<u16>().ok()).unwrap_or(PORT);
    format!("http://{HOST}:{port}")
}
