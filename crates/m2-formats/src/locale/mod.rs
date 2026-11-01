//! Code page ids, and (feature `glibc-tables`, off by default) the glibc-exact
//! port of `neoeter/src/platform/locale`: the code page conversions of
//! `StringCodec.cpp` (glibc iconv on Linux), the CP1258 codec of
//! `StringCodec_Vietnamese.cpp`, Arabic shaping and Shift-JIS helpers.
//!
//! Under D14/D16 the default build converts text with [`crate::utf8`]
//! (`encoding_rs`); the glibc port is kept, with its 320 KB of tables, as the
//! reference the differential tests compare `encoding_rs` against. Nothing in
//! the client's shipped locales needs Arabic shaping, the Shift-JIS helpers
//! or the CP1258 composition (`neoeter`'s only users are its own tests).
//!
//! Wide characters are Linux `wchar_t` code units (`u32`, UTF-32). Inputs
//! are explicit slices: where the C++ caller passes a length of -1, pass the
//! string including its NUL.

#[cfg(feature = "glibc-tables")]
pub mod arabic;
#[cfg(feature = "glibc-tables")]
mod glibc;
#[cfg(feature = "glibc-tables")]
mod iconv;
#[cfg(feature = "glibc-tables")]
pub mod japanese;
#[cfg(feature = "glibc-tables")]
pub mod vietnamese;

#[cfg(feature = "glibc-tables")]
pub use glibc::*;

/// Code page ids (`CodePageId.h`).
pub mod code_page {
    pub const ACP: u32 = 0;
    pub const THAI: u32 = 874;
    pub const JAPANESE: u32 = 932;
    pub const CHINESE_SIMPLE: u32 = 936;
    pub const HANGUL: u32 = 949;
    pub const CHINESE_TRAD: u32 = 950;
    pub const EASTEUROPE: u32 = 1250;
    pub const CYRILLIC: u32 = 1251;
    pub const LATIN: u32 = 1252;
    pub const GREEK: u32 = 1253;
    pub const TURKISH: u32 = 1254;
    pub const HEBREW: u32 = 1255;
    pub const ARABIC: u32 = 1256;
    pub const BALTIC: u32 = 1257;
    pub const VIETNAMESE: u32 = 1258;
    pub const UTF8: u32 = 65001;
}

