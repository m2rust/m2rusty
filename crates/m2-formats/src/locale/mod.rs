//! Port of `neoeter/src/platform/locale`: the code page conversions of
//! `StringCodec.cpp` (glibc iconv on Linux, see [`iconv`]), the CP1258 codec
//! of `StringCodec_Vietnamese.cpp`, Arabic shaping and Shift-JIS helpers.
//!
//! Wide characters are Linux `wchar_t` code units (`u32`, UTF-32). Inputs
//! are explicit slices: where the C++ caller passes a length of -1, pass the
//! string including its NUL.

pub mod arabic;
mod iconv;
pub mod japanese;
pub mod vietnamese;

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

/// `MultiByteToWideChar` (Linux): decodes `input` into `output` and returns
/// the number of characters written; with an empty `output`, returns the
/// number needed. CP_ACP and unknown code pages are UTF-8.
///
/// Invalid bytes are skipped one at a time. A character glibc's CP1255 and
/// CP1258 decoders hold back to combine with a following mark is lost at the
/// end of the input, as in C++.
pub fn multi_byte_to_wide_char(code_page: u32, input: &[u8], output: &mut [u32]) -> usize {
    if input.is_empty() {
        return 0;
    }
    if output.is_empty() {
        return iconv::decode(code_page, input, usize::MAX).len();
    }
    let wide = iconv::decode(code_page, input, output.len());
    output[..wide.len()].copy_from_slice(&wide);
    wide.len()
}

/// `WideCharToMultiByte` (Linux): encodes `input` into `output` and returns
/// the number of bytes written; with an empty `output`, returns the number
/// needed. Unencodable characters are skipped (no default character); a
/// character that does not fit whole ends the conversion.
pub fn wide_char_to_multi_byte(code_page: u32, input: &[u32], output: &mut [u8]) -> usize {
    if input.is_empty() {
        return 0;
    }
    if output.is_empty() {
        return iconv::encode(code_page, input, usize::MAX).len();
    }
    let bytes = iconv::encode(code_page, input, output.len());
    output[..bytes.len()].copy_from_slice(&bytes);
    bytes.len()
}

/// `Ymir_MultiByteToWideChar`: [`vietnamese::decode`] for CP1258 (where an
/// empty `output` returns 0, not a size), [`multi_byte_to_wide_char`]
/// otherwise.
pub fn ymir_multi_byte_to_wide_char(code_page: u32, input: &[u8], output: &mut [u32]) -> usize {
    if code_page == code_page::VIETNAMESE {
        vietnamese::decode(input, output)
    } else {
        multi_byte_to_wide_char(code_page, input, output)
    }
}

/// `Ymir_WideCharToMultiByte`: [`vietnamese::encode`] for CP1258 (where an
/// empty `output` returns 0, not a size), [`wide_char_to_multi_byte`]
/// otherwise.
pub fn ymir_wide_char_to_multi_byte(code_page: u32, input: &[u32], output: &mut [u8]) -> usize {
    if code_page == code_page::VIETNAMESE {
        vietnamese::encode(input, output)
    } else {
        wide_char_to_multi_byte(code_page, input, output)
    }
}

/// The whole of `input` through [`multi_byte_to_wide_char`].
pub fn decode(code_page: u32, input: &[u8]) -> Vec<u32> {
    iconv::decode(code_page, input, usize::MAX)
}

/// The whole of `input` through [`wide_char_to_multi_byte`].
pub fn encode(code_page: u32, input: &[u32]) -> Vec<u8> {
    iconv::encode(code_page, input, usize::MAX)
}

/// The whole of `input` through [`ymir_multi_byte_to_wide_char`], as the
/// text renderer decodes (with room for every character, also in CP1258).
pub fn ymir_decode(code_page: u32, input: &[u8]) -> Vec<u32> {
    if code_page == code_page::VIETNAMESE {
        let mut wide = vec![0; input.len()];
        let n = vietnamese::decode(input, &mut wide);
        wide.truncate(n);
        wide
    } else {
        decode(code_page, input)
    }
}

/// The whole of `input` through [`ymir_wide_char_to_multi_byte`] (with room
/// for every byte, also in CP1258).
pub fn ymir_encode(code_page: u32, input: &[u32]) -> Vec<u8> {
    if code_page == code_page::VIETNAMESE {
        let mut multi = vec![0; input.len() * 2];
        let n = vietnamese::encode(input, &mut multi);
        multi.truncate(n);
        multi
    } else {
        encode(code_page, input)
    }
}
