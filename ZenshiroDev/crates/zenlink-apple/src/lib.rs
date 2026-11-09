//! Apple Driver mode on [idevice] (Rust), replacing pymobiledevice3 step by step.
//! See `docs/RUST_TRANSITION_AUDIT.md` in the workspace.

pub mod ddi;
pub mod files;
pub mod hid;
pub mod link;
pub mod mirror;
pub mod models;
pub mod usbdirect;
pub mod watch;
pub mod wifi;
