//! Mapping between Bevy asset paths and pack *virtual names*.
//!
//! A pack virtual name (see [`m2_formats::pack`]) is the path inside the pack,
//! ASCII-lowercased, with `/` separators, and `d:/` in front of `ymir work/…`
//! (the Windows client's data drive). Asset paths are UTF-8 strings that must
//! not contain `:` (Bevy parses `source://path`, and `d:` would be a drive
//! prefix on Windows), so the convention is:
//!
//! | asset path (`m2://…`)             | virtual name                    |
//! |-----------------------------------|---------------------------------|
//! | `ymir work/pc/warrior/a.gr2`      | `d:/ymir work/pc/warrior/a.gr2` |
//! | `YMIR WORK\PC\Warrior\A.GR2`      | `d:/ymir work/pc/warrior/a.gr2` |
//! | `d:/ymir work/pc/warrior/a.gr2`   | `d:/ymir work/pc/warrior/a.gr2` |
//! | `locale/en/locale_game.txt`       | `locale/en/locale_game.txt`     |
//!
//! Lookups are case-insensitive and accept `\`, exactly like
//! [`m2_formats::pack::normalize`]. Bevy itself compares asset paths with case
//! sensitivity, so `A.dds` and `a.dds` are two handles for the same bytes; use
//! [`canonical_asset_path`] to build handles from game data.

use m2_formats::pack;

/// Prefix of the virtual names of `ymir work/…` files.
const DRIVE: &str = "d:/";
const YMIR_WORK: &str = "ymir work/";

/// The virtual name (normalised bytes) for an asset path.
///
/// Accepts mixed case, `\` separators, a leading `/` or `./`, and an explicit
/// `d:/` prefix. See the module documentation for the convention.
pub fn virtual_name(asset_path: &str) -> Vec<u8> {
    let normalized = pack::normalize(asset_path);
    let mut name: &[u8] = &normalized;
    while let Some(rest) = name.strip_prefix(b"./").or_else(|| name.strip_prefix(b"/")) {
        name = rest;
    }
    pack::virtual_name(name.strip_prefix(DRIVE.as_bytes()).unwrap_or(name))
}

/// The asset path for a virtual name: `d:/` is dropped. `None` when the name is
/// not valid UTF-8 (about 31 files in the shipped packs have CP949 names; they
/// cannot be addressed through the asset system).
pub fn asset_path(virtual_name: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(virtual_name).ok()?;
    Some(s.strip_prefix(DRIVE).unwrap_or(s).to_owned())
}

/// Lowercase, `/`-separated, drive-less spelling of any path (`D:\Ymir
/// Work\PC\A.GR2` becomes `ymir work/pc/a.gr2`): the one spelling to use when
/// building asset handles from names found inside game data, so that the same
/// file is never loaded twice under two spellings.
pub fn canonical_asset_path(path: &str) -> String {
    asset_path(&virtual_name(path)).unwrap_or_else(|| path.to_owned())
}

/// Canonical client spelling, retaining the `d:/` prefix used by the C++
/// client for files rooted under `ymir work/`.
pub fn canonical_game_path(path: &str) -> String {
    let name = virtual_name(path);
    std::str::from_utf8(&name).map(str::to_owned).unwrap_or_else(|_| path.to_owned())
}

/// Bevy source URI for a client path. Bevy's `AssetPath` syntax requires
/// `source://path`; a literal `d:/path` is a normal filesystem path, so the
/// `m2://` adapter is kept at this boundary while game-facing paths retain
/// their original `d:/` spelling.
pub fn asset_source_uri(path: &str) -> String {
    format!("m2://{}", canonical_asset_path(path))
}

/// Whether `asset_path` has a `ymir work/` root, i.e. maps to a `d:/` name.
pub fn is_ymir_work(path: &str) -> bool {
    canonical_asset_path(path).starts_with(YMIR_WORK)
}

/// Directory part of a canonical asset path (`""` for a top-level file).
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Joins a directory and a file name of canonical paths.
pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_path_to_virtual_name() {
        assert_eq!(virtual_name("ymir work/pc/warrior/a.gr2"), b"d:/ymir work/pc/warrior/a.gr2");
        assert_eq!(virtual_name("YMIR WORK\\PC\\Warrior\\A.GR2"), b"d:/ymir work/pc/warrior/a.gr2");
        assert_eq!(virtual_name("d:/ymir work/pc/a.gr2"), b"d:/ymir work/pc/a.gr2");
        assert_eq!(virtual_name("D:\\Ymir Work\\pc\\a.gr2"), b"d:/ymir work/pc/a.gr2");
        assert_eq!(virtual_name("locale/en/Locale_Game.txt"), b"locale/en/locale_game.txt");
        assert_eq!(virtual_name("./ymir work/a"), b"d:/ymir work/a");
        assert_eq!(virtual_name("/ymir work/a"), b"d:/ymir work/a");
        // Only a leading "ymir work/" is rooted.
        assert_eq!(virtual_name("foo/ymir work/a"), b"foo/ymir work/a");
    }

    #[test]
    fn virtual_name_to_asset_path() {
        assert_eq!(asset_path(b"d:/ymir work/pc/a.gr2").as_deref(), Some("ymir work/pc/a.gr2"));
        assert_eq!(asset_path(b"locale/en/x.txt").as_deref(), Some("locale/en/x.txt"));
        assert_eq!(asset_path(b"\xBF\xEB\xB8\xB6.dds"), None);
    }

    #[test]
    fn round_trip_and_canonical() {
        for p in ["ymir work/ui/game.dds", "locale/en/x.txt", "bgm/a.mp3"] {
            assert_eq!(asset_path(&virtual_name(p)).unwrap(), p);
        }
        assert_eq!(canonical_asset_path("D:\\Ymir Work\\PC\\A.GR2"), "ymir work/pc/a.gr2");
        assert_eq!(canonical_game_path("D:\\Ymir Work\\PC\\A.GR2"), "d:/ymir work/pc/a.gr2");
        assert_eq!(asset_source_uri("D:\\Ymir Work\\PC\\A.GR2"), "m2://ymir work/pc/a.gr2");
        assert!(is_ymir_work("d:/Ymir Work/x"));
        assert!(!is_ymir_work("locale/x"));
        assert_eq!(parent("ymir work/pc/a.gr2"), "ymir work/pc");
        assert_eq!(parent("a.gr2"), "");
        assert_eq!(join("", "a"), "a");
        assert_eq!(join("x/y", "a"), "x/y/a");
    }
}
