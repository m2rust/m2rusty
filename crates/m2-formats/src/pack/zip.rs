//! `ZIP` provider: a `.zip` archive, read in place.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

use zip::ZipArchive;

use super::{Error, Provider, virtual_name};

pub struct ZipPack {
    // Reading an entry seeks the file, so access is serialised.
    archive: Mutex<ZipArchive<File>>,
    files: BTreeMap<Vec<u8>, usize>,
}

impl ZipPack {
    /// Opens the archive and indexes its entries. Directory entries are
    /// skipped; of two entries with the same virtual name the first is kept,
    /// as with `std::map::insert` in C++.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut archive = ZipArchive::new(File::open(path)?)?;

        let mut files = BTreeMap::new();
        for index in 0..archive.len() {
            let name = entry_name(&mut archive, index)?;
            if name.is_empty() || name.ends_with(b"/") || name.ends_with(b"\\") {
                continue;
            }
            files.entry(virtual_name(name)).or_insert(index);
        }

        Ok(Self {
            archive: Mutex::new(archive),
            files,
        })
    }
}

/// The entry name as libzip returns it with `ZIP_FL_ENC_GUESS` (flags 0 in
/// C++): unchanged if it is valid UTF-8, otherwise decoded from CP437 to
/// UTF-8. CP949 names therefore come out as CP437 mojibake in both clients.
fn entry_name(archive: &mut ZipArchive<File>, index: usize) -> Result<Vec<u8>, Error> {
    let raw = archive.by_index_raw(index)?.name_raw().to_vec();
    if std::str::from_utf8(&raw).is_ok() {
        return Ok(raw);
    }
    // Without the UTF-8 flag the zip crate decodes the name as CP437.
    match archive.name_for_index(index) {
        Some(Ok(name)) => Ok(name.as_bytes().to_vec()),
        _ => Ok(raw),
    }
}

impl Provider for ZipPack {
    fn file_names(&self) -> Vec<Vec<u8>> {
        self.files.keys().cloned().collect()
    }

    fn contains(&self, name: &[u8]) -> bool {
        self.files.contains_key(name)
    }

    fn read(&self, name: &[u8]) -> Result<Vec<u8>, Error> {
        let &index = self.files.get(name).ok_or_else(|| Error::NotFound(name.to_vec()))?;
        let mut archive = self.archive.lock().unwrap_or_else(|e| e.into_inner());
        let mut entry = archive.by_index(index)?;
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut data)?;
        Ok(data)
    }
}
