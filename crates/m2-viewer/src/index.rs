//! The pack index as the browser sees it: one [`Entry`] per virtual file, with
//! its type and its display path. Pure data, no Bevy.

use std::path::Path;
use std::sync::Arc;

use m2_formats::pack::{PackManager, load_index};

/// File type groups of the browser filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileKind {
    Gr2,
    Texture,
    Spt,
    Race,
    Effect,
    Text,
    Other,
}

impl FileKind {
    pub const ALL: [FileKind; 7] =
        [FileKind::Gr2, FileKind::Texture, FileKind::Spt, FileKind::Race, FileKind::Effect, FileKind::Text, FileKind::Other];

    pub fn label(self) -> &'static str {
        match self {
            FileKind::Gr2 => "gr2",
            FileKind::Texture => "dds/tga/jpg",
            FileKind::Spt => "spt",
            FileKind::Race => "msm/msa",
            FileKind::Effect => "mse",
            FileKind::Text => "txt/py",
            FileKind::Other => "other",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// Classifies a lowercase extension (without the dot).
    pub fn from_ext(ext: &str) -> FileKind {
        match ext {
            "gr2" => FileKind::Gr2,
            "dds" | "tga" | "jpg" | "jpeg" | "bmp" | "png" => FileKind::Texture,
            "spt" => FileKind::Spt,
            "msm" | "msa" => FileKind::Race,
            "mse" => FileKind::Effect,
            "txt" | "py" => FileKind::Text,
            _ => FileKind::Other,
        }
    }
}

/// Lowercase extension of a virtual path ("" when none).
pub fn extension(name: &str) -> &str {
    let file = name.rsplit('/').next().unwrap_or(name);
    match file.rfind('.') {
        Some(i) if i + 1 < file.len() => &file[i + 1..],
        _ => "",
    }
}

/// `d:/ymir work/x` is shown as `ymir work/x`.
pub fn display_path(name: &str) -> &str {
    match name.as_bytes() {
        [d, b':', b'/', ..] if d.is_ascii_alphabetic() => &name[3..],
        _ => name,
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Virtual name, lowercase, `/`-separated (as the pack manager keys it).
    pub name: String,
    pub kind: FileKind,
}

impl Entry {
    pub fn new(name: String) -> Self {
        let kind = FileKind::from_ext(extension(&name));
        Entry { name, kind }
    }
    pub fn display(&self) -> &str {
        display_path(&self.name)
    }
    pub fn ext(&self) -> &str {
        extension(&self.name)
    }
}

/// Everything the background load produces.
pub struct LoadedPack {
    pub manager: Arc<PackManager>,
    /// Sorted by display path.
    pub entries: Vec<Entry>,
    pub registered: usize,
    pub failed: Vec<String>,
}

/// Builds the sorted entry list from raw virtual names.
pub fn entries_from_names<'a>(names: impl IntoIterator<Item = &'a [u8]>) -> Vec<Entry> {
    let mut v: Vec<Entry> = names.into_iter().map(|n| Entry::new(String::from_utf8_lossy(n).into_owned())).collect();
    v.sort_by(|a, b| a.display().cmp(b.display()));
    v
}

/// `m2-viewer [client dir]`: registers the packs of `<client>/<index>` and lists their files.
pub fn load_pack(client_dir: &Path, index: &str) -> Result<LoadedPack, String> {
    let mut manager = PackManager::new(client_dir);
    let report = load_index(&mut manager, "pack", index).map_err(|e| format!("{}: {e}", client_dir.join(index).display()))?;
    let entries = entries_from_names(manager.file_names());
    Ok(LoadedPack {
        registered: report.registered.len(),
        failed: report.failed.iter().map(|(n, e)| format!("{n}: {e}")).collect(),
        manager: Arc::new(manager),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_and_display() {
        assert_eq!(extension("d:/ymir work/a/b.GR2".to_lowercase().as_str()), "gr2");
        assert_eq!(extension("noext"), "");
        assert_eq!(extension("dir.x/file"), "");
        assert_eq!(display_path("d:/ymir work/a.dds"), "ymir work/a.dds");
        assert_eq!(display_path("locale/x.txt"), "locale/x.txt");
        assert_eq!(FileKind::from_ext("jpg"), FileKind::Texture);
        assert_eq!(FileKind::from_ext("msa"), FileKind::Race);
        assert_eq!(FileKind::from_ext("zzz"), FileKind::Other);
    }

    #[test]
    fn real_index() {
        let dir = std::env::var("M2_CLIENT_DIR").unwrap_or_else(|_| "bin".into());
        if !Path::new(&dir).join("pack/Index.dev").exists() {
            eprintln!("skipping: no client data at {dir}");
            return;
        }
        let p = load_pack(Path::new(&dir), "pack/Index.dev").unwrap();
        assert!(p.entries.len() > 1000, "{}", p.entries.len());
        assert!(p.entries.iter().any(|e| e.kind == FileKind::Gr2));
        assert!(p.entries.iter().any(|e| e.kind == FileKind::Texture));
        eprintln!("{} files, {} packs", p.entries.len(), p.registered);
    }
}
