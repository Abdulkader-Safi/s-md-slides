//! Assets come only from the deck's folder. Absolute paths and anything that escapes
//! the folder after resolving symlinks are refused (spec: Security).

use s_md_slides::{Asset, AssetResolver};
use std::path::{Path, PathBuf};

pub struct Folder {
    root: PathBuf,
}

impl Folder {
    pub fn for_deck(deck: &Path) -> Folder {
        let parent = deck
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Folder {
            root: parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file for a relative path, if it exists inside the folder.
    pub fn locate(&self, rel: &str) -> Option<PathBuf> {
        let rel = rel.trim_start_matches("./");
        if rel.is_empty() || Path::new(rel).is_absolute() || rel.contains('\0') {
            return None;
        }
        let full = self.root.join(rel).canonicalize().ok()?;
        (full.starts_with(&self.root) && full.is_file()).then_some(full)
    }
}

impl AssetResolver for Folder {
    fn resolve(&self, path: &str) -> Option<Asset> {
        let full = self.locate(path)?;
        Some(Asset {
            bytes: std::fs::read(full).ok()?,
            mime: String::new(),
        })
    }
}
