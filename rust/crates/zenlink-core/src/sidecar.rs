//! Stage 1 (ADR-0001): the device protocol layer runs as the Python backend
//! in headless mode. It exits when this process exits (--parent-pid).

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crate::api::Api;

pub struct Sidecar {
    child: Option<Child>,
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Portable build: the backend exe packed into this binary.
#[cfg(feature = "embedded-backend")]
static EMBEDDED: &[u8] = include_bytes!(env!("ZENLINK_BACKEND_EXE"));

/// Unpack the packed backend to %LOCALAPPDATA%\ZenLink\service once per
/// version, and return its path.
#[cfg(feature = "embedded-backend")]
fn unpack_embedded() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let dir = base.join("ZenLink").join("service");
    let path = dir.join(format!("ZenLinkService-{}-{}.exe", env!("CARGO_PKG_VERSION"), EMBEDDED.len()));
    if std::fs::metadata(&path).map(|m| m.len() == EMBEDDED.len() as u64).unwrap_or(false) {
        return Some(path);
    }
    std::fs::create_dir_all(&dir).ok()?;
    // Older versions are no longer needed; one in use stays until next time.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let tmp = dir.join(format!("unpack-{}.tmp", std::process::id()));
    std::fs::write(&tmp, EMBEDDED).ok()?;
    std::fs::rename(&tmp, &path).ok()?;
    Some(path)
}

fn command_for(dir: &Path) -> Option<Command> {
    // Installed layout: the backend (ZenLinkService.exe) next to this executable.
    let exe = dir.join("ZenLinkService.exe");
    if exe.is_file() {
        return Some(Command::new(exe));
    }
    #[cfg(feature = "embedded-backend")]
    if let Some(exe) = unpack_embedded() {
        return Some(Command::new(exe));
    }
    // Source checkout: <repo>/.venv + `python -m zenlink`.
    for up in dir.ancestors() {
        let py = up.join(".venv").join("Scripts").join("python.exe");
        if py.is_file() && up.join("zenlink").join("__main__.py").is_file() {
            let mut c = Command::new(py);
            c.args(["-m", "zenlink"]).current_dir(up);
            return Some(c);
        }
    }
    None
}

impl Sidecar {
    /// Reuse a backend that is already running, otherwise start one and wait
    /// until it answers.
    pub fn ensure(api: &Api, timeout: Duration) -> Result<Self, String> {
        if api.alive() {
            return Ok(Self { child: None });
        }
        let mut cmd = match std::env::var_os("ZENLINK_SIDECAR").map(PathBuf::from) {
            Some(p) => Command::new(p),
            None => {
                let here = std::env::current_exe().map_err(|e| e.to_string())?;
                command_for(here.parent().unwrap_or(Path::new("."))).ok_or("ZenLink backend not found")?
            }
        };
        cmd.args(["--headless", "--parent-pid", &std::process::id().to_string()]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let child = cmd.spawn().map_err(|e| format!("start backend: {e}"))?;
        let mut me = Self { child: Some(child) };
        let t0 = Instant::now();
        while t0.elapsed() < timeout {
            if api.alive() {
                return Ok(me);
            }
            if let Some(c) = me.child.as_mut() {
                if let Ok(Some(status)) = c.try_wait() {
                    return Err(format!("backend exited ({status})"));
                }
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        Err("backend did not start in time".into())
    }

    pub fn owned(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
