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

fn command_for(dir: &Path) -> Option<Command> {
    // Installed layout: the v2 backend (ZenLink.exe) next to this executable.
    let exe = dir.join("ZenLink.exe");
    if exe.is_file() {
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
