//! Send files to an iPhone/iPad (port of backend/zenlink/apple/files.py).
//!
//! iOS has no shared Download folder a PC can write to. Finder's "File Sharing" uses
//! house_arrest: an app with `UIFileSharingEnabled` exposes its Documents folder over AFC,
//! and the file then shows in that app and in Files > On My iPhone. Needs Apple Mobile
//! Device Service (usbmux) and a trusted device; never starts a pairing.

use std::path::Path;

use idevice::{
    afc::opcode::AfcFopenMode, house_arrest::HouseArrestClient,
    installation_proxy::InstallationProxyClient, IdeviceService,
};
use tokio::io::AsyncReadExt;

use crate::link::provider;

const DOCUMENTS: &str = "/Documents";
/// Per AFC write: smooth progress, bounded memory.
const CHUNK: usize = 1 << 20;

fn err(what: &str) -> impl Fn(idevice::IdeviceError) -> String + '_ {
    move |e| format!("{what}: {e:?}")
}

/// A file name without folders or characters iOS/AFC would refuse; at most 200 bytes.
pub fn safe_name(name: &str) -> Option<String> {
    let clean: String = name
        .chars()
        .map(|c| {
            if (c as u32) < 32 || "\"*/:<>?\\|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut clean = clean.trim().trim_end_matches('.').to_string();
    if clean.is_empty() || clean.chars().all(|c| c == '.' || c == '_') {
        return None;
    }
    if clean.len() > 200 {
        let (mut stem, ext) = split_ext(&clean);
        let ext = if ext.len() <= 10 { ext } else { String::new() };
        while stem.len() + ext.len() > 200 {
            stem.pop();
        }
        clean = stem + &ext;
    }
    Some(clean)
}

/// ("name", ".ext"); a leading dot alone is no extension (".bashrc").
fn split_ext(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_string(), name[i..].to_string()),
        _ => (name.to_string(), String::new()),
    }
}

/// "a (n).txt"
pub fn numbered(name: &str, n: u32) -> String {
    let (stem, ext) = split_ext(name);
    format!("{stem} ({n}){ext}")
}

/// Apps whose Documents folder is shared: (bundle id, name), sorted by name.
pub async fn sharing_apps(udid: &str) -> Result<Vec<(String, String)>, String> {
    let p = provider(udid).await?;
    let mut proxy = InstallationProxyClient::connect(&p)
        .await
        .map_err(err("installation_proxy"))?;
    let apps = proxy
        .get_apps(Some("User"), None)
        .await
        .map_err(err("app list"))?;
    let mut out: Vec<(String, String)> = apps
        .into_iter()
        .filter_map(|(bundle, info)| {
            let d = info.as_dictionary()?;
            d.get("UIFileSharingEnabled")?.as_boolean().filter(|b| *b)?;
            let s = |k: &str| d.get(k).and_then(|v| v.as_string()).map(str::to_owned);
            let name = s("CFBundleDisplayName")
                .or_else(|| s("CFBundleName"))
                .unwrap_or(bundle.clone());
            Some((bundle, name))
        })
        .collect();
    out.sort_by_key(|(_, n)| n.to_lowercase());
    Ok(out)
}

/// Streams `local` into the app's Documents folder; `progress(sent, total)` after each
/// chunk. Returns the name used (numbered when the name is taken).
pub async fn send(
    udid: &str,
    bundle: &str,
    local: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<String, String> {
    let name = local
        .file_name()
        .and_then(|n| safe_name(&n.to_string_lossy()))
        .ok_or("unusable file name")?;
    let mut file = tokio::fs::File::open(local)
        .await
        .map_err(|e| e.to_string())?;
    let total = file.metadata().await.map_err(|e| e.to_string())?.len();
    let p = provider(udid).await?;
    let mut afc = HouseArrestClient::connect(&p)
        .await
        .map_err(err("house_arrest"))?
        .vend_documents(bundle)
        .await
        .map_err(err("app folder"))?;
    let mut remote = name.clone();
    let mut n = 0;
    while afc
        .get_file_info(format!("{DOCUMENTS}/{remote}"))
        .await
        .is_ok()
    {
        n += 1;
        if n > 999 {
            return Err("no free file name".into());
        }
        remote = numbered(&name, n);
    }
    let mut fd = afc
        .open(format!("{DOCUMENTS}/{remote}"), AfcFopenMode::WrOnly)
        .await
        .map_err(err("open"))?;
    let mut buf = vec![0u8; CHUNK];
    let mut sent = 0u64;
    progress(0, total);
    loop {
        let k = file.read(&mut buf).await.map_err(|e| e.to_string())?;
        if k == 0 {
            break;
        }
        fd.write_entire(&buf[..k]).await.map_err(err("write"))?;
        sent += k as u64;
        progress(sent, total);
    }
    fd.close().await.map_err(err("close"))?;
    Ok(remote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(safe_name("a/b:c.txt").as_deref(), Some("a_b_c.txt"));
        assert_eq!(safe_name(" x. ").as_deref(), Some("x"));
        assert_eq!(safe_name("..."), None);
        assert_eq!(safe_name("_._"), None);
        let long = format!("{}.mp4", "é".repeat(150));
        let s = safe_name(&long).unwrap();
        assert!(s.len() <= 200 && s.ends_with(".mp4"), "{s}");
        assert_eq!(numbered("a.txt", 2), "a (2).txt");
        assert_eq!(numbered(".bashrc", 1), ".bashrc (1)");
    }
}
