//! UTF-8 at the boundary (D14): legacy code pages to `str` and back.
//!
//! The Phase 1 readers stay byte-oriented because they are proven byte for
//! byte against C++. Everything that wants to show or compare text calls the
//! functions here, once, with the code page of the data's locale.
//!
//! The conversion is `encoding_rs` (pure Rust, wasm/mobile-safe, D16). On the
//! real corpus it gives the same text as the glibc `iconv` port in
//! [`crate::locale`] (feature `glibc-tables`); the evidence is in
//! `tests/utf8_corpus.rs` and `rust_port.md`.
//!
//! Code pages are the ids of [`crate::locale::code_page`]. `0` (`CP_ACP`),
//! `65001` and unknown ids are UTF-8, as in C++. Malformed input never fails:
//! bytes that map to nothing become U+FFFD.

use std::borrow::Cow;

use encoding_rs::{BIG5, EUC_KR, Encoding, GBK, SHIFT_JIS, UTF_8};
use encoding_rs::{
    WINDOWS_874, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256,
    WINDOWS_1257, WINDOWS_1258,
};

use crate::locale::code_page;

/// The `encoding_rs` encoding that stands for a Windows code page.
///
/// `949` is `EUC_KR`, which in the WHATWG sense is the full Unified Hangul
/// Code (CP949). `None` for UTF-8, `CP_ACP` and unknown ids (use UTF-8).
pub fn encoding(code_page: u32) -> Option<&'static Encoding> {
    Some(match code_page {
        code_page::THAI => WINDOWS_874,
        code_page::JAPANESE => SHIFT_JIS,
        code_page::CHINESE_SIMPLE => GBK,
        code_page::HANGUL => EUC_KR,
        code_page::CHINESE_TRAD => BIG5,
        code_page::EASTEUROPE => WINDOWS_1250,
        code_page::CYRILLIC => WINDOWS_1251,
        code_page::LATIN => WINDOWS_1252,
        code_page::GREEK => WINDOWS_1253,
        code_page::TURKISH => WINDOWS_1254,
        code_page::HEBREW => WINDOWS_1255,
        code_page::ARABIC => WINDOWS_1256,
        code_page::BALTIC => WINDOWS_1257,
        code_page::VIETNAMESE => WINDOWS_1258,
        _ => return None,
    })
}

/// The code page of a locale pack name (`locale/<name>/`), as the client's
/// `gs_stLocaleData` (`client/src/ui/Locale.cpp`) lists them, except `cz`: the client says 1252 but the
/// shipped Czech pack is CP1250 (see `tests/utf8_corpus.rs`). `gr` is the
/// shipped Greek pack, which the client calls `tr` with 1253.
pub fn locale_code_page(locale: &str) -> Option<u32> {
    Some(match locale {
        "de" | "en" | "us" | "es" | "it" | "fr" | "pt" | "dk" | "mx" | "nl" | "ca" | "sg" | "br" => {
            code_page::LATIN
        }
        "pl" | "hu" | "ro" | "cz" => code_page::EASTEUROPE,
        "tr" => code_page::TURKISH,
        "gr" => code_page::GREEK,
        "ru" => code_page::CYRILLIC,
        "ae" => code_page::ARABIC,
        "japan" => code_page::JAPANESE,
        "hongkong" => code_page::CHINESE_TRAD,
        "newcibn" => code_page::CHINESE_SIMPLE,
        "ymir" | "kr" | "english" => code_page::HANGUL,
        _ => return None,
    })
}

/// The locale packs in `bin/pack` and their code pages.
pub const SHIPPED_LOCALES: [(&str, u32); 15] = [
    ("cz", code_page::EASTEUROPE),
    ("de", code_page::LATIN),
    ("dk", code_page::LATIN),
    ("en", code_page::LATIN),
    ("es", code_page::LATIN),
    ("fr", code_page::LATIN),
    ("gr", code_page::GREEK),
    ("hu", code_page::EASTEUROPE),
    ("it", code_page::LATIN),
    ("nl", code_page::LATIN),
    ("pl", code_page::EASTEUROPE),
    ("pt", code_page::LATIN),
    ("ro", code_page::EASTEUROPE),
    ("ru", code_page::CYRILLIC),
    ("tr", code_page::TURKISH),
];

/// Code page used for the root pack's own tables (`936mob_proto`, ...) and
/// the client's Korean data.
pub const DEFAULT_NAME_CODE_PAGE: u32 = code_page::HANGUL;

/// `MultiByteToWideChar` into a `str`: the whole of `input` in `code_page`,
/// with U+FFFD for bytes that map to nothing. A byte order mark is not
/// special. Borrows when `input` is already valid text of the same bytes
/// (ASCII, or UTF-8 input).
pub fn decode(code_page: u32, input: &[u8]) -> Cow<'_, str> {
    encoding(code_page).unwrap_or(UTF_8).decode_without_bom_handling(input).0
}

/// [`decode`], or `None` when some byte maps to nothing.
pub fn decode_strict(code_page: u32, input: &[u8]) -> Option<Cow<'_, str>> {
    encoding(code_page).unwrap_or(UTF_8).decode_without_bom_handling_and_without_replacement(input)
}

/// [`decode`] of a NUL-terminated buffer (`char name[N]` in the proto
/// tables): the text up to the first NUL.
pub fn decode_cstr(code_page: u32, input: &[u8]) -> Cow<'_, str> {
    decode(code_page, crate::proto::cstr(input))
}

/// `WideCharToMultiByte` from a `str`: characters the code page cannot hold
/// become `?`, as an unconvertible character does with a default character.
/// UTF-8 and unknown code pages return the string's own bytes.
pub fn encode(code_page: u32, s: &str) -> Cow<'_, [u8]> {
    let Some(enc) = encoding(code_page) else {
        return Cow::Borrowed(s.as_bytes());
    };
    let mut encoder = enc.new_encoder();
    let mut out = Vec::with_capacity(s.len());
    let mut rest = s;
    loop {
        let mut buf = [0u8; 256];
        let (result, read, written) = encoder.encode_from_utf8_without_replacement(rest, &mut buf, true);
        out.extend_from_slice(&buf[..written]);
        rest = &rest[read..];
        match result {
            encoding_rs::EncoderResult::InputEmpty => break,
            encoding_rs::EncoderResult::OutputFull => {}
            encoding_rs::EncoderResult::Unmappable(_) => out.push(b'?'),
        }
    }
    Cow::Owned(out)
}

/// How a pack virtual name was turned into text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameSource {
    /// Valid UTF-8 (including plain ASCII), taken as is.
    Utf8,
    /// Not valid UTF-8: decoded from the legacy code page.
    Legacy,
    /// Valid UTF-8 whose characters are all Latin-1 (U+0080..=U+00FF): the
    /// zip layer shows a CP949 entry name this way (each byte as one
    /// character, `libzip` guess in C++). The bytes were recovered and
    /// decoded from the legacy code page.
    Mojibake,
}

/// A pack virtual name as text. Names are lowercase ASCII paths except for a
/// few that are legacy-code-page names.
///
/// Valid UTF-8 is kept (`NameSource::Utf8`) unless it is Latin-1 mojibake of
/// legacy bytes that decode cleanly in `legacy_code_page` (CP949 for the
/// shipped packs, [`DEFAULT_NAME_CODE_PAGE`]); invalid UTF-8 is decoded with
/// `legacy_code_page`.
pub fn virtual_name(name: &[u8], legacy_code_page: u32) -> (Cow<'_, str>, NameSource) {
    let Ok(s) = std::str::from_utf8(name) else {
        return (decode(legacy_code_page, name), NameSource::Legacy);
    };
    if !s.is_ascii() && s.chars().all(|c| (c as u32) <= 0xff) {
        let raw: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
        if let Some(text) = decode_strict(legacy_code_page, &raw) {
            // Real Latin-1 text that is not valid in the code page stays as is.
            return (Cow::Owned(text.into_owned()), NameSource::Mojibake);
        }
    }
    (Cow::Borrowed(s), NameSource::Utf8)
}

/// [`virtual_name`] over all names; returns the UTF-8 names, and the
/// indexes (into the input order) of those that were not plain UTF-8.
pub fn virtual_names<'a>(
    names: impl IntoIterator<Item = &'a [u8]>,
    legacy_code_page: u32,
) -> (Vec<Cow<'a, str>>, Vec<usize>) {
    let mut out = Vec::new();
    let mut legacy = Vec::new();
    for (i, name) in names.into_iter().enumerate() {
        let (s, source) = virtual_name(name, legacy_code_page);
        if source != NameSource::Utf8 {
            legacy.push(i);
        }
        out.push(s);
    }
    (out, legacy)
}

// UTF-8 accessors for the byte fields of the Phase 1 structures. The bytes
// are in the code page of the locale the table was loaded from.

impl crate::proto::ItemTable {
    /// `name` (the Korean/base name) as UTF-8, in `code_page`.
    pub fn name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode_cstr(code_page, &self.name)
    }

    /// `locale_name` (the displayed name) as UTF-8, in `code_page`.
    pub fn locale_name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode_cstr(code_page, &self.locale_name)
    }
}

impl crate::proto::MobTable {
    /// `name` as UTF-8, in `code_page`.
    pub fn name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode_cstr(code_page, &self.name)
    }

    /// `locale_name` (the displayed name) as UTF-8, in `code_page`.
    pub fn locale_name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode_cstr(code_page, &self.locale_name)
    }
}

impl crate::proto::ItemData {
    /// `description` (itemdesc.txt) as UTF-8, in `code_page`.
    pub fn description_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.description)
    }

    /// `summary` (itemdesc.txt) as UTF-8, in `code_page`.
    pub fn summary_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.summary)
    }
}

impl crate::proto::AffectData {
    /// `description` as UTF-8, in `code_page`.
    pub fn description_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.description)
    }
}

impl crate::proto::GradeData {
    /// `name` as UTF-8, in `code_page`.
    pub fn name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.name)
    }
}

impl crate::proto::SkillData {
    /// `name` as UTF-8, in `code_page`.
    pub fn name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.name)
    }

    /// `description` as UTF-8, in `code_page`.
    pub fn description_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.description)
    }

    /// The condition lines as UTF-8, in `code_page`.
    pub fn conditions_utf8(&self, code_page: u32) -> Vec<Cow<'_, str>> {
        self.conditions.iter().map(|c| decode(code_page, c)).collect()
    }
}

impl crate::map::AtlasEntry {
    /// The map name as UTF-8 (a path-like ASCII name in the shipped data).
    pub fn name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.name)
    }
}

impl crate::map::Terrain {
    /// The area name as UTF-8.
    pub fn name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.name)
    }
}

impl crate::property::Property {
    /// The property's file name as UTF-8 (CP949 bytes decoded; the usual
    /// lowercase ASCII path is borrowed).
    pub fn file_name_utf8(&self, code_page: u32) -> Cow<'_, str> {
        decode(code_page, &self.file_name)
    }

    /// A string value as UTF-8.
    pub fn get_string_utf8(&self, key: impl AsRef<[u8]>, code_page: u32) -> Option<Cow<'_, str>> {
        self.get_string(key).map(|v| decode(code_page, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_borrowed() {
        assert!(matches!(decode(1252, b"abc"), Cow::Borrowed("abc")));
        assert!(matches!(decode(949, b"abc"), Cow::Borrowed("abc")));
    }

    #[test]
    fn single_byte_pages() {
        assert_eq!(decode(1252, b"caf\xe9 \x80"), "caf\u{e9} \u{20ac}");
        assert_eq!(decode(1251, b"\xcf\xf0\xe8\xe2\xe5\xf2"), "\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}");
        assert_eq!(decode(1253, b"\xe1\xe2"), "\u{3b1}\u{3b2}");
        assert_eq!(decode(1254, b"\xfd\xdd"), "\u{131}\u{130}");
        assert_eq!(decode(1250, b"\xb9\xe8"), "\u{105}\u{10d}");
    }

    #[test]
    fn hangul() {
        assert_eq!(decode(949, b"\xb0\xa1\xb3\xaa"), "\u{ac00}\u{b098}");
        assert_eq!(encode(949, "\u{ac00}").as_ref(), b"\xb0\xa1");
    }

    #[test]
    fn utf8_and_unknown_pages_are_utf8() {
        assert_eq!(decode(0, "h\u{e9}".as_bytes()), "h\u{e9}");
        assert_eq!(decode(65001, b"a\xffb"), "a\u{fffd}b");
        assert_eq!(decode(437, b"abc"), "abc");
        assert_eq!(encode(0, "h\u{e9}").as_ref(), "h\u{e9}".as_bytes());
    }

    #[test]
    fn no_bom_handling() {
        assert_eq!(decode(65001, b"\xef\xbb\xbfa"), "\u{feff}a");
    }

    #[test]
    fn unmappable_encodes_to_question_mark() {
        assert_eq!(encode(1252, "a\u{3b1}b").as_ref(), b"a?b");
        assert_eq!(encode(1252, &"x".repeat(1000)).len(), 1000);
    }

    #[test]
    fn strict_rejects_unmappable() {
        assert_eq!(decode_strict(949, b"ab\xb0\xa1").unwrap(), "ab\u{ac00}");
        assert_eq!(decode_strict(65001, b"ab\xffcd"), None);
    }

    #[test]
    fn cstr_stops_at_nul() {
        assert_eq!(decode_cstr(1252, b"ab\0cd"), "ab");
    }

    #[test]
    fn virtual_names_split_utf8_and_legacy() {
        let (s, src) = virtual_name(b"d:/ymir work/a.gr2", 949);
        assert_eq!((s.as_ref(), src), ("d:/ymir work/a.gr2", NameSource::Utf8));
        let (s, src) = virtual_name(b"icon/\xb0\xa1.tga", 949);
        assert_eq!((s.as_ref(), src), ("icon/\u{ac00}.tga", NameSource::Legacy));
        // CP949 bytes B0 A1 shown as two Latin-1 characters.
        let (s, src) = virtual_name("icon/\u{b0}\u{a1}.tga".as_bytes(), 949);
        assert_eq!((s.as_ref(), src), ("icon/\u{ac00}.tga", NameSource::Mojibake));
        // Latin-1 text that is not a valid CP949 sequence stays.
        let (s, src) = virtual_name("caf\u{e9}".as_bytes(), 949);
        assert_eq!((s.as_ref(), src), ("caf\u{e9}", NameSource::Utf8));
        let (all, legacy) = virtual_names([b"a".as_slice(), b"\xb0\xa1", b"b"], 949);
        assert_eq!((all.len(), legacy), (3, vec![1]));
    }

    #[test]
    fn locale_table() {
        assert_eq!(locale_code_page("gr"), Some(1253));
        assert_eq!(locale_code_page("ru"), Some(1251));
        assert_eq!(locale_code_page("xx"), None);
        for (name, cp) in SHIPPED_LOCALES {
            assert_eq!(locale_code_page(name), Some(cp));
        }
    }
}
