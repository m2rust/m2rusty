//! `CEterPackManager`: the registry of packs and loose files.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Container, Error, FolderPack, Provider, ZipPack, normalize};

/// Which wins when a name exists both on disk and in a pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Loose files under the root first (development).
    #[default]
    FileFirst,
    /// Packs first (`_DISTRIBUTE` builds).
    PackFirst,
}

/// Every registered file, by virtual name.
///
/// Relative pack and file paths resolve against `root`, where C++ uses the
/// process working directory.
pub struct PackManager {
    root: PathBuf,
    mode: SearchMode,
    files: HashMap<Vec<u8>, Arc<dyn Provider>>,
    packs: HashMap<String, Arc<dyn Provider>>,
}

impl PackManager {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            mode: SearchMode::default(),
            files: HashMap::new(),
            packs: HashMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn search_mode(&self) -> SearchMode {
        self.mode
    }

    pub fn set_search_mode(&mut self, mode: SearchMode) {
        self.mode = mode;
    }

    /// Registers the pack at `name` (relative to the root, `\` allowed). A
    /// later pack overrides the files of earlier ones. Registering a name
    /// twice is a no-op.
    ///
    /// The on-disk name is matched ignoring ASCII case. For a folder, every
    /// sibling directory matching that way is registered (packs extracted on
    /// Windows can be split across `ETC/` and `etc/`), the exact spelling last.
    pub fn register(&mut self, name: &str, container: Container) -> Result<(), Error> {
        if self.packs.contains_key(name) {
            return Ok(());
        }
        let path = self.root.join(name.replace('\\', "/"));

        let pack: Arc<dyn Provider> = match container {
            Container::Folder => {
                let mut dirs = case_insensitive_dirs(&path);
                if dirs.is_empty() {
                    dirs.push(path);
                }
                let mut last = None;
                for dir in dirs {
                    let pack: Arc<dyn Provider> = Arc::new(FolderPack::open(dir)?);
                    self.add_files(&pack);
                    last = Some(pack);
                }
                let pack = last.expect("at least one folder");
                self.packs.insert(name.to_owned(), pack);
                return Ok(());
            }
            Container::Zip => {
                let zip = with_extension(&path, "zip");
                let real = case_insensitive_file(&zip).unwrap_or(zip);
                Arc::new(ZipPack::open(real)?)
            }
        };

        self.add_files(&pack);
        self.packs.insert(name.to_owned(), pack);
        Ok(())
    }

    /// Whether a pack called `name` exists on disk (`PackExists`).
    pub fn pack_exists(&self, name: &str, container: Container) -> bool {
        let path = self.root.join(name.replace('\\', "/"));
        match container {
            Container::Folder => case_insensitive_file(&path).is_some(),
            Container::Zip => case_insensitive_file(&with_extension(&path, "zip")).is_some(),
        }
    }

    /// Virtual names of every registered file, unordered.
    pub fn file_names(&self) -> impl Iterator<Item = &[u8]> {
        self.files.keys().map(Vec::as_slice)
    }

    /// Contents of `name`, from a loose file or a pack per the search mode.
    pub fn get(&self, name: impl AsRef<[u8]>) -> Result<Vec<u8>, Error> {
        let name = name.as_ref();
        match self.mode {
            SearchMode::FileFirst => self.get_from_file(name).or_else(|_| self.get_from_pack(name)),
            SearchMode::PackFirst => self.get_from_pack(name).or_else(|_| self.get_from_file(name)),
        }
    }

    /// Contents of `name` from the packs only.
    pub fn get_from_pack(&self, name: impl AsRef<[u8]>) -> Result<Vec<u8>, Error> {
        let name = normalize(name);
        match self.files.get(&name) {
            Some(pack) => pack.read(&name),
            None => Err(Error::NotFound(name)),
        }
    }

    /// `isExist`: a loose file or a pack entry, per the search mode.
    pub fn exists(&self, name: impl AsRef<[u8]>) -> bool {
        let name = name.as_ref();
        match self.mode {
            SearchMode::FileFirst => self.loose_file(name).is_some() || self.exists_in_pack(name),
            SearchMode::PackFirst => self.exists_in_pack(name) || self.loose_file(name).is_some(),
        }
    }

    /// `isExistInPack`.
    pub fn exists_in_pack(&self, name: impl AsRef<[u8]>) -> bool {
        let name = normalize(name);
        self.files.get(&name).is_some_and(|pack| pack.contains(&name))
    }

    fn add_files(&mut self, pack: &Arc<dyn Provider>) {
        for name in pack.file_names() {
            self.files.insert(name, Arc::clone(pack));
        }
    }

    fn get_from_file(&self, name: &[u8]) -> Result<Vec<u8>, Error> {
        match self.loose_file(name) {
            Some(path) => Ok(fs::read(path)?),
            None => Err(Error::NotFound(name.to_vec())),
        }
    }

    /// The name as given (separators fixed), then lowercased.
    fn loose_file(&self, name: &[u8]) -> Option<PathBuf> {
        let separators: Vec<u8> = name.iter().map(|&b| if b == b'\\' { b'/' } else { b }).collect();
        [separators, normalize(name)]
            .into_iter()
            .map(|n| self.root.join(bytes_path(&n)))
            .find(|p| p.is_file())
    }
}

fn with_extension(path: &Path, ext: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

#[cfg(unix)]
fn bytes_path(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn bytes_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

fn file_name_eq_ignore_case(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    a.as_encoded_bytes().eq_ignore_ascii_case(b.as_encoded_bytes())
}

/// `FindCaseInsensitiveFile`: `path` if it exists, else the first entry of
/// its parent whose name matches ignoring ASCII case.
fn case_insensitive_file(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_owned());
    }
    let wanted = path.file_name()?;
    let mut entries: Vec<_> = fs::read_dir(parent(path)).ok()?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    entries
        .into_iter()
        .find(|e| file_name_eq_ignore_case(&e.file_name(), wanted))
        .map(|e| e.path())
}

/// `FindCaseInsensitiveDirs`: every sibling directory matching `path`'s name
/// ignoring ASCII case, the exact spelling last so its files win.
fn case_insensitive_dirs(path: &Path) -> Vec<PathBuf> {
    let Some(wanted) = path.file_name() else {
        return Vec::new();
    };
    let Ok(read) = fs::read_dir(parent(path)) else {
        return Vec::new();
    };
    let mut dirs: Vec<_> = read
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir() && file_name_eq_ignore_case(&e.file_name(), wanted))
        .map(|e| e.file_name())
        .collect();
    dirs.sort();
    // Stable: the exact spelling moves to the end, the rest keep their order.
    dirs.sort_by_key(|n| n == wanted);
    dirs.into_iter().map(|n| parent(path).join(n)).collect()
}

fn parent(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}
