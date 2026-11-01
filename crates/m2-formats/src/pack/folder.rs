//! `Folder` provider: a directory tree on disk.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{Error, Provider, virtual_name};

pub struct FolderPack {
    root: PathBuf,
    files: BTreeMap<Vec<u8>, PathBuf>,
}

impl FolderPack {
    /// Lists every file under `root`. Fails if `root` is not a directory.
    ///
    /// Two files whose virtual names collide (`A.dds` and `a.dds`) keep the
    /// one visited last. C++ visits in directory order, which is unspecified;
    /// this visits in byte order of the on-disk names.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        if !root.is_dir() {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("failed to open directory: {}", root.display()),
            )));
        }

        let mut files = BTreeMap::new();
        list(&root, &mut Vec::new(), &mut files)?;
        Ok(Self { root, files })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn list(dir: &Path, relative: &mut Vec<u8>, files: &mut BTreeMap<Vec<u8>, PathBuf>) -> Result<(), Error> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name();
        let len = relative.len();
        relative.extend_from_slice(os_bytes(&name));

        let path = entry.path();
        // Follows symlinks, like std::filesystem::directory_entry::is_directory.
        if path.is_dir() {
            relative.push(b'/');
            list(&path, relative, files)?;
        } else {
            files.insert(virtual_name(&relative[..]), path);
        }
        relative.truncate(len);
    }
    Ok(())
}

#[cfg(unix)]
fn os_bytes(s: &std::ffi::OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(s: &std::ffi::OsStr) -> &[u8] {
    s.to_str().map(str::as_bytes).unwrap_or_default()
}

impl Provider for FolderPack {
    fn file_names(&self) -> Vec<Vec<u8>> {
        self.files.keys().cloned().collect()
    }

    fn contains(&self, name: &[u8]) -> bool {
        self.files.contains_key(name)
    }

    fn read(&self, name: &[u8]) -> Result<Vec<u8>, Error> {
        let path = self.files.get(name).ok_or_else(|| Error::NotFound(name.to_vec()))?;
        Ok(fs::read(path)?)
    }
}
