//! Developer Disk Image: which kind a device needs and where its files are.
//!
//! Same choice as pymobiledevice3 11.50 (`auto_mount_personalized`):
//! - iOS 27 and later: the Cryptex1 DDI, installed over `cryptexd` (needs the RSD tunnel).
//! - iOS 17 to 26: the Personalized DMG, mounted over the image mounter.
//!
//! The files come from github.com/doronz88/DeveloperDiskImage (`PersonalizedImages/...`), as in
//! pymobiledevice3. ZenLink keeps them in `%LOCALAPPDATA%\ZenLink-Next\ddi\<kind>`; a copy that
//! pymobiledevice3 already downloaded (`~\.pymobiledevice3\<kind>`) is used too.

use std::path::{Path, PathBuf};

/// The DDI kind a device needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// iOS 17 to 26.
    Personalized,
    /// iOS 27 and later.
    Cryptex,
}

impl Kind {
    /// `None` below iOS 17 (signed DMG, not supported by ZenLink's Apple Driver mode).
    pub fn for_version(product_version: &str) -> Option<Self> {
        let major: u32 = product_version.split('.').next()?.trim().parse().ok()?;
        match major {
            0..=16 => None,
            17..=26 => Some(Kind::Personalized),
            _ => Some(Kind::Cryptex),
        }
    }

    /// Folder name, the same as in the DeveloperDiskImage repository and pymobiledevice3's cache.
    pub fn folder(self) -> &'static str {
        match self {
            Kind::Personalized => "Xcode_iOS_DDI_Personalized",
            Kind::Cryptex => "Xcode_iOS_DDI_Cryptex",
        }
    }

    /// Files of this kind, relative to its folder.
    pub fn files(self) -> &'static [&'static str] {
        match self {
            Kind::Personalized => &["BuildManifest.plist", "Image.dmg", "Image.dmg.trustcache"],
            Kind::Cryptex => &[
                "BuildManifest.plist",
                "Image.dmg",
                "Image.dmg.trustcache",
                "Image.dmg.cryptex_info",
                "Image.dmg.root_hash",
            ],
        }
    }
}

/// Folders to look in, first match wins.
pub fn search_dirs(kind: Kind) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(
            PathBuf::from(local)
                .join("ZenLink-Next")
                .join("ddi")
                .join(kind.folder()),
        );
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        dirs.push(
            PathBuf::from(home)
                .join(".pymobiledevice3")
                .join(kind.folder()),
        );
    }
    dirs
}

/// The first folder in `dirs` that has every file of `kind`.
pub fn find(kind: Kind, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter()
        .find(|d| kind.files().iter().all(|f| d.join(f).is_file()))
        .cloned()
}

/// Raw files of a found DDI folder.
pub struct Files {
    pub build_manifest: Vec<u8>,
    pub image: Vec<u8>,
    pub trustcache: Vec<u8>,
    /// Cryptex only.
    pub cryptex_info: Vec<u8>,
    /// Cryptex only.
    pub root_hash: Vec<u8>,
}

pub fn read(kind: Kind, dir: &Path) -> std::io::Result<Files> {
    let r = |name: &str| std::fs::read(dir.join(name));
    let cryptex = kind == Kind::Cryptex;
    Ok(Files {
        build_manifest: r("BuildManifest.plist")?,
        image: r("Image.dmg")?,
        trustcache: r("Image.dmg.trustcache")?,
        cryptex_info: if cryptex {
            r("Image.dmg.cryptex_info")?
        } else {
            Vec::new()
        },
        root_hash: if cryptex {
            r("Image.dmg.root_hash")?
        } else {
            Vec::new()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_follows_pymobiledevice3() {
        assert_eq!(Kind::for_version("16.7.10"), None);
        assert_eq!(Kind::for_version("17.0"), Some(Kind::Personalized));
        assert_eq!(Kind::for_version("26.4.1"), Some(Kind::Personalized));
        assert_eq!(Kind::for_version("27.0"), Some(Kind::Cryptex));
        assert_eq!(Kind::for_version("28.1"), Some(Kind::Cryptex));
        assert_eq!(Kind::for_version(""), None);
    }

    #[test]
    fn find_needs_every_file() {
        let base = std::env::temp_dir().join(format!("zl-ddi-{}", std::process::id()));
        let full = base.join("full");
        let part = base.join("part");
        for d in [&full, &part] {
            std::fs::create_dir_all(d).unwrap();
        }
        for f in Kind::Cryptex.files() {
            std::fs::write(full.join(f), b"x").unwrap();
        }
        std::fs::write(part.join("Image.dmg"), b"x").unwrap();
        assert_eq!(
            find(Kind::Cryptex, &[part.clone(), full.clone()]),
            Some(full.clone())
        );
        assert_eq!(find(Kind::Cryptex, &[part]), None);
        let files = read(Kind::Cryptex, &full).unwrap();
        assert_eq!(files.root_hash, b"x");
        std::fs::remove_dir_all(base).unwrap();
    }
}
