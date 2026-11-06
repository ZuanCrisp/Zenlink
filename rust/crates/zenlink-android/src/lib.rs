//! ZenLink Android support.
//!
//! - [`scrcpy`]: the pinned scrcpy server version (client and server must match exactly).
//! - [`adb`]: a client for the ADB host protocol (the `adb` server on 127.0.0.1:5037).
//! - [`devices`]: `adb devices -l` / `host:track-devices-l` parsing and connection kinds.
//! - [`props`]: `getprop` output parsing.
//! - [`naming`]: human-readable device names from `ro.product.*` properties.
//! - [`identity`]: a stable per-device identity that never depends on an IP address.
//! - [`discovery`]: combines the above into the device list the UI shows.
//! - [`mirror`]: starts the scrcpy server and reads its video stream (phase A3).
//!
//! Everything here is portable Rust; only [`adb::start_server`] touches the platform.

pub mod adb;
pub mod devices;
pub mod discovery;
pub mod identity;
pub mod mirror;
pub mod naming;
pub mod props;
pub mod scrcpy;
