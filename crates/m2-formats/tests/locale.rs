//! Port of `neoeter/src/platform/locale/tests/test_locale_codecs.cpp`, plus
//! the glibc behaviours the Rust codec reproduces.

use m2_formats::locale::{
    code_page as cp, decode, encode, multi_byte_to_wide_char, wide_char_to_multi_byte, ymir_decode, ymir_encode,
    ymir_multi_byte_to_wide_char,
};

fn roundtrip(code_page: u32, input: &[u8]) {
    let mut wide = [0; 256];
    let n = multi_byte_to_wide_char(code_page, input, &mut wide);
    assert!(n > 0);
    let mut back = [0; 256];
    let m = wide_char_to_multi_byte(code_page, &wide[..n], &mut back);
    assert_eq!(&back[..m], input, "code page {code_page}");
}

#[test]
fn round_trips() {
    roundtrip(cp::UTF8, b"Caf\xc3\xa9\0");
    roundtrip(cp::UTF8, "日本語\0".as_bytes());
    roundtrip(cp::LATIN, b"Caf\xe9\0");
    roundtrip(cp::HANGUL, b"\xc7\xd1\0");
    roundtrip(cp::UTF8, b"abc");
}

#[test]
fn size_queries() {
    let sample = b"Metin2\0";
    let needed = multi_byte_to_wide_char(cp::UTF8, sample, &mut []);
    let mut wide = [0; 32];
    let written = multi_byte_to_wide_char(cp::UTF8, sample, &mut wide);
    assert_eq!(needed, written);
    assert_eq!(written, 7);
    assert_eq!(wide_char_to_multi_byte(cp::UTF8, &wide[..written], &mut []), 7);
    // CP1258 through Ymir_* is the Vietnamese codec, which has no size query.
    assert_eq!(ymir_multi_byte_to_wide_char(cp::VIETNAMESE, b"abc", &mut []), 0);
}

#[test]
fn glibc_specifics() {
    // Undefined CP1252 bytes are skipped (WHATWG would map them).
    assert_eq!(decode(cp::LATIN, b"a\x81b"), [0x61, 0x62]);
    // Unencodable characters are skipped, tag characters silently dropped.
    assert_eq!(encode(cp::LATIN, &[0x41, 0xac00, 0xe0041, 0x42]), b"AB");
    // CP_ACP and unknown code pages are UTF-8.
    assert_eq!(decode(cp::ACP, "é".as_bytes()), [0xe9]);
    assert_eq!(decode(437, "é".as_bytes()), [0xe9]);
    // A truncated character is dropped; the next one survives.
    assert_eq!(decode(cp::HANGUL, b"A\xc7"), [0x41]);
    // Does not fit whole: stop.
    let mut out = [0; 3];
    assert_eq!(wide_char_to_multi_byte(cp::HANGUL, &[0x41, 0xd55c, 0x42], &mut out), 3);
    let mut out = [0; 2];
    assert_eq!(wide_char_to_multi_byte(cp::HANGUL, &[0x41, 0xd55c], &mut out), 1);
}

#[test]
fn ymir_vietnamese() {
    let wide = ymir_decode(cp::VIETNAMESE, b"Vie\xea\xf2t Nam");
    assert_eq!(wide, [0x56, 0x69, 0x65, 0x1ec7, 0x74, 0x20, 0x4e, 0x61, 0x6d]);
    assert_eq!(ymir_encode(cp::VIETNAMESE, &wide), b"Vie\xea\xf2t Nam");
    // Other code pages go through iconv.
    assert_eq!(ymir_decode(cp::LATIN, b"\xe9"), [0xe9]);
}
