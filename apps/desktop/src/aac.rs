//! AAC / AAC-ELD decoding through zlaac.dll (the same minimal FFmpeg the backend uses, see
//! apps/desktop/backend/zenlink/aac.py). Output: interleaved s16le stereo at 48 kHz.

use std::path::{Path, PathBuf};

use windows::core::{s, HSTRING, PCWSTR};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type Open = unsafe extern "C" fn(*const u8, i32, i32, i32, i32) -> *mut std::ffi::c_void;
type Decode = unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, i32, *mut i16, i32) -> i32;
type Close = unsafe extern "C" fn(*mut std::ffi::c_void);

/// Largest output of one access unit in frames (aac.py `_CAP`).
const CAP: usize = 8192;

pub struct Aac {
    h: *mut std::ffi::c_void,
    decode: Decode,
    close: Close,
    buf: Vec<i16>,
}

// The handle is only used by the one task that owns the decoder.
unsafe impl Send for Aac {}

/// zlaac.dll next to ZenLink.exe (packaged), else in a source checkout's backend folder.
fn find_dll() -> Option<PathBuf> {
    find_native("", "zlaac.dll").or_else(unpack_embedded)
}

/// A native DLL next to ZenLink.exe, else in a source checkout's backend/zenlink/`sub`.
pub fn find_native(sub: &str, name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let source = std::env::var_os("ZENLINK_SOURCE")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = exe.parent().map(Path::to_path_buf).into_iter().collect();
    dirs.extend(source.into_iter().chain(exe.ancestors().skip(1).map(Path::to_path_buf)).map(|d| {
        d.join("apps").join("desktop").join("backend").join("zenlink").join(sub)
    }));
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.exists())
}

/// Portable.exe: zlaac.dll is packed in, unpacked to %LOCALAPPDATA%\ZenLink-Next\native once
/// per size (the service folder is emptied on each new version, so not there).
#[cfg(feature = "embedded-backend")]
fn unpack_embedded() -> Option<PathBuf> {
    static DLL: &[u8] = include_bytes!("../backend/zenlink/zlaac.dll");
    let dir = zenlink_core::local_data_dir()?.join("native");
    let path = dir.join(format!("zlaac-{}.dll", DLL.len()));
    if std::fs::metadata(&path).is_ok_and(|m| m.len() == DLL.len() as u64) {
        return Some(path);
    }
    std::fs::create_dir_all(&dir).ok()?;
    let tmp = dir.join(format!("unpack-{}.tmp", std::process::id()));
    std::fs::write(&tmp, DLL).ok()?;
    std::fs::rename(&tmp, &path).ok()?;
    Some(path)
}

#[cfg(not(feature = "embedded-backend"))]
fn unpack_embedded() -> Option<PathBuf> {
    None
}

impl Aac {
    /// A decoder for one stream with this AudioSpecificConfig.
    pub fn new(asc: &[u8], rate: i32) -> Result<Self, String> {
        Self::with_dll(&find_dll().ok_or("zlaac.dll not found")?, asc, rate)
    }

    pub fn with_dll(dll: &Path, asc: &[u8], rate: i32) -> Result<Self, String> {
        unsafe {
            let lib = LoadLibraryW(PCWSTR(HSTRING::from(dll.as_os_str()).as_ptr()))
                .map_err(|e| format!("load {}: {e}", dll.display()))?;
            let open: Open = std::mem::transmute(GetProcAddress(lib, s!("zl_aac_open")).ok_or("zl_aac_open")?);
            let decode: Decode = std::mem::transmute(GetProcAddress(lib, s!("zl_aac_decode")).ok_or("zl_aac_decode")?);
            let close: Close = std::mem::transmute(GetProcAddress(lib, s!("zl_aac_close")).ok_or("zl_aac_close")?);
            let h = open(asc.as_ptr(), asc.len() as i32, rate, 2, 48000);
            if h.is_null() {
                return Err("AAC decoder did not open (bad codec config)".into());
            }
            Ok(Self { h, decode, close, buf: vec![0; CAP * 2] })
        }
    }

    /// s16le stereo bytes of one access unit (empty on a decode error).
    pub fn decode(&mut self, au: &[u8]) -> &[u8] {
        if au.is_empty() {
            return &[];
        }
        let n = unsafe { (self.decode)(self.h, au.as_ptr(), au.len() as i32, self.buf.as_mut_ptr(), CAP as i32) };
        let n = (n.max(0) as usize).min(CAP);
        unsafe { std::slice::from_raw_parts(self.buf.as_ptr().cast::<u8>(), n * 4) }
    }
}

impl Drop for Aac {
    fn drop(&mut self) {
        unsafe { (self.close)(self.h) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_eld_fixture() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("backend");
        let data = std::fs::read(root.join("tests/fixtures/eld_48000.zlau")).unwrap();
        assert_eq!(&data[..4], b"ZLAU");
        let n = data[4] as usize;
        let asc = &data[5..5 + n];
        let mut pos = 9 + n; // ZLAU, length, ASC, u32 rate
        let mut dec = Aac::with_dll(&root.join("zenlink/zlaac.dll"), asc, 48000).unwrap();
        let mut bytes = 0;
        while pos + 2 <= data.len() {
            let size = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            bytes += dec.decode(&data[pos + 2..pos + 2 + size]).len();
            pos += 2 + size;
        }
        assert!(bytes > 48000 * 4 / 10, "only {bytes} bytes of sound");
    }
}
