//! Pack files: port of `neoeter/src/io` (`CEterPackManager`, `Folder`, `ZIP`)
//! and of the `Index` loop in `client/src/ui/UserInterface.cpp`.
//!
//! A pack is a folder or a `.zip` archive. Every file in it gets a *virtual
//! name*: its path inside the pack, ASCII-lowercased, with `/` separators, and
//! `d:/` in front of `ymir work/…`. Lookups normalise the requested name the
//! same way, so `D:\Ymir Work\PC\a.GR2` finds `d:/ymir work/pc/a.gr2`.

mod folder;
mod index;
mod manager;
mod zip;

use std::fmt;
use std::io;

pub use folder::FolderPack;
pub use index::{IndexReport, load_index};
pub use manager::{PackManager, SearchMode};
pub use zip::ZipPack;

/// One registered pack.
pub trait Provider: Send + Sync {
    /// Virtual names of every file, in byte order.
    fn file_names(&self) -> Vec<Vec<u8>>;
    /// Whether `name` (normalised by the caller) is in this pack.
    fn contains(&self, name: &[u8]) -> bool;
    /// Contents of `name` (normalised by the caller).
    fn read(&self, name: &[u8]) -> Result<Vec<u8>, Error>;
}

/// The two container kinds an `Index` line can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Folder,
    Zip,
}

impl Container {
    /// Parses the `Index` spelling (`FOLDER`, `ZIP`); case-sensitive, as in C++.
    pub fn parse(s: &[u8]) -> Result<Self, Error> {
        match s {
            b"FOLDER" => Ok(Container::Folder),
            b"ZIP" => Ok(Container::Zip),
            _ => Err(Error::UnknownContainer(String::from_utf8_lossy(s).into_owned())),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Zip(::zip::result::ZipError),
    NotFound(Vec<u8>),
    UnknownContainer(String),
    /// An `Index` line without exactly two tab-separated fields.
    IndexLine { line: usize, tokens: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Zip(e) => write!(f, "zip: {e}"),
            Error::NotFound(name) => write!(f, "not found: {}", String::from_utf8_lossy(name)),
            Error::UnknownContainer(c) => write!(f, "unexpected container type: {c}"),
            Error::IndexLine { line, tokens } => {
                write!(f, "index line {line}: expected 2 tokens, got {tokens}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Zip(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<::zip::result::ZipError> for Error {
    fn from(e: ::zip::result::ZipError) -> Self {
        Error::Zip(e)
    }
}

/// `CEterPackManager::ConvertFileName`: ASCII lowercase and `\` → `/`.
/// Bytes >= 0x80 are left as they are.
pub fn normalize(name: impl AsRef<[u8]>) -> Vec<u8> {
    name.as_ref()
        .iter()
        .map(|&b| if b == b'\\' { b'/' } else { b.to_ascii_lowercase() })
        .collect()
}

/// Virtual name of a file at `relative` inside a pack: normalised, and
/// `ymir work/…` rooted at `d:/` as the original Windows client saw it.
pub fn virtual_name(relative: impl AsRef<[u8]>) -> Vec<u8> {
    let name = normalize(relative);
    if name.starts_with(b"ymir work/") {
        [&b"d:/"[..], &name].concat()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization() {
        // Cases from neoeter/src/io/tests/test_pack_formats.cpp.
        assert_eq!(normalize("D:\\Ymir Work\\UI\\Game.DDS"), b"d:/ymir work/ui/game.dds");
        assert_eq!(normalize(""), b"");
        assert_eq!(normalize("MIXED/Path\\Name.TXT"), b"mixed/path/name.txt");
        assert_eq!(normalize(b"\xBF\xEB\xB8\xB6"), b"\xBF\xEB\xB8\xB6");
        assert_eq!(
            normalize("D:\\Ymir Work\\PC\\Warrior\\Action.GR2"),
            b"d:/ymir work/pc/warrior/action.gr2"
        );
    }

    #[test]
    fn virtual_names() {
        assert_eq!(virtual_name("Ymir Work/UI/a.dds"), b"d:/ymir work/ui/a.dds");
        assert_eq!(virtual_name("locale/en/x.txt"), b"locale/en/x.txt");
        // Only a leading "ymir work/" is rooted.
        assert_eq!(virtual_name("d/ymir work/a"), b"d/ymir work/a");
    }

    #[test]
    fn containers() {
        assert_eq!(Container::parse(b"FOLDER").unwrap(), Container::Folder);
        assert_eq!(Container::parse(b"ZIP").unwrap(), Container::Zip);
        assert!(Container::parse(b"zip").is_err());
        assert!(Container::parse(b"TAR").is_err());
    }
}
