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
//! - [`control`]: scrcpy control messages (touch, keys, text, clipboard; phase A4).
//! - [`encoders`]: Smart Encoder Detection and the bounded start plan (phase A5).
//! - [`profiles`]: video profiles and per-device settings (phase A5).
//! - [`wireless`]: Android over Wi-Fi: tcpip from USB, Android 11+ pairing (phase A6).
//! - [`apps`]: installed apps (`list_apps`), for app windows on a virtual display (v2.5.1).
//!
//! Everything here is portable Rust; only [`adb::start_server`] touches the platform.

pub mod adb;
pub mod apps;
pub mod control;
pub mod devices;
pub mod discovery;
pub mod encoders;
pub mod identity;
pub mod mirror;
pub mod naming;
pub mod profiles;
pub mod props;
pub mod scrcpy;
pub mod wireless;
