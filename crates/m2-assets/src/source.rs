//! The `m2://` asset source: a Bevy [`AssetReader`] over a shared
//! [`PackManager`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use bevy::asset::io::{AssetReader, AssetReaderError, PathStream, Reader, VecReader};
use bevy::tasks::futures_lite::stream;
use m2_formats::pack::{self, PackManager};

use crate::paths;

/// Pack manager shared between the asset source and the application.
pub type SharedPacks = Arc<PackManager>;

/// Directory structure of every registered file, derived from the virtual
/// names. Only names that are valid UTF-8 can be addressed (D14); the others
/// are counted in [`DirectoryIndex::non_utf8`].
#[derive(Debug, Default)]
pub struct DirectoryIndex {
    /// Directory (`""` is the root) to its entries; `true` marks a directory.
    dirs: BTreeMap<String, BTreeMap<String, bool>>,
    files: usize,
    non_utf8: usize,
}

impl DirectoryIndex {
    /// Builds the index from the manager's virtual names.
    pub fn build(packs: &PackManager) -> Self {
        let mut index = DirectoryIndex::default();
        let mut seen = BTreeSet::new();
        for name in packs.file_names() {
            let Some(path) = paths::asset_path(name) else {
                index.non_utf8 += 1;
                continue;
            };
            if !seen.insert(path.clone()) {
                continue;
            }
            index.files += 1;
            // Register the file and every ancestor directory.
            let mut child = path;
            let mut is_dir = false;
            loop {
                let dir = paths::parent(&child).to_owned();
                let leaf = child[dir.len() + usize::from(!dir.is_empty())..].to_owned();
                let entries = index.dirs.entry(dir.clone()).or_default();
                let known = entries.insert(leaf, is_dir).is_some();
                if (known && is_dir) || dir.is_empty() {
                    // The ancestors of an existing directory are registered already.
                    break;
                }
                child = dir;
                is_dir = true;
            }
        }
        index
    }

    /// Number of addressable files.
    pub fn file_count(&self) -> usize {
        self.files
    }

    /// Number of files whose virtual name is not valid UTF-8 and so cannot be
    /// addressed as an asset path.
    pub fn non_utf8(&self) -> usize {
        self.non_utf8
    }

    /// Entries of directory `dir` (lowercase, no trailing slash): `(name,
    /// is_directory)` in byte order.
    pub fn list(&self, dir: &str) -> Option<impl Iterator<Item = (&str, bool)>> {
        self.dirs.get(dir).map(|e| e.iter().map(|(n, d)| (n.as_str(), *d)))
    }

    /// Whether `dir` is a directory containing at least one file.
    pub fn is_dir(&self, dir: &str) -> bool {
        self.dirs.contains_key(dir)
    }
}

fn clean_dir(path: &Path) -> Option<String> {
    let name = pack::normalize(path.to_str()?);
    let s = std::str::from_utf8(&name).ok()?.trim_matches('/');
    let s = s.strip_prefix("d:").map_or(s, |r| r.trim_start_matches('/'));
    Some(s.to_owned())
}

/// Reads bytes from the packs (and loose files, per the manager's search mode)
/// for `m2://` asset paths. See [`crate::paths`] for the path convention.
#[derive(Clone)]
pub struct PackAssetReader {
    packs: SharedPacks,
    index: Arc<OnceLock<DirectoryIndex>>,
}

impl PackAssetReader {
    pub fn new(packs: SharedPacks) -> Self {
        Self { packs, index: Arc::new(OnceLock::new()) }
    }

    /// The directory index, built on first use (it walks every file name).
    pub fn index(&self) -> &DirectoryIndex {
        self.index.get_or_init(|| DirectoryIndex::build(&self.packs))
    }
}

impl AssetReader for PackAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let not_found = || AssetReaderError::NotFound(path.to_owned());
        let name = paths::virtual_name(path.to_str().ok_or_else(not_found)?);
        match self.packs.get(&name) {
            Ok(bytes) => Ok(VecReader::new(bytes)),
            Err(pack::Error::NotFound(_)) => Err(not_found()),
            Err(pack::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Err(not_found()),
            Err(e) => Err(std::io::Error::other(e).into()),
        }
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // Packs carry no `.meta` files: the default loader settings apply.
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_owned()))
    }

    async fn read_directory<'a>(&'a self, path: &'a Path) -> Result<Box<PathStream>, AssetReaderError> {
        let not_found = || AssetReaderError::NotFound(path.to_owned());
        let dir = clean_dir(path).ok_or_else(not_found)?;
        let entries = self.index().list(&dir).ok_or_else(not_found)?;
        let paths: Vec<PathBuf> = entries.map(|(name, _)| PathBuf::from(paths::join(&dir, name))).collect();
        Ok(Box::new(stream::iter(paths)))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(clean_dir(path).is_some_and(|d| self.index().is_dir(&d)))
    }
}
