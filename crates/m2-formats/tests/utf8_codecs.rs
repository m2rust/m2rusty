//! `encoding_rs` (through `m2_formats::utf8`) against the glibc `iconv` port
//! (`m2_formats::locale`), over every byte and byte pair of every code page.
//! This is the "what could differ" half of the D16 evaluation;
//! `utf8_corpus.rs` is the "what does differ on the real data" half.
//!
//! Needs feature `glibc-tables` (on in `cargo test`, see Cargo.toml).
//! `cargo test -p m2-formats --test utf8_codecs -- --nocapture` prints the table.
#![cfg(feature = "glibc-tables")]

use std::collections::BTreeMap;

use m2_formats::locale;
use m2_formats::utf8;

const PAGES: [u32; 14] = [874, 932, 936, 949, 950, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258];

/// Shipped locale code pages (+ 949 for names): no "different character" class.
const SHIPPED: [u32; 6] = [949, 1250, 1251, 1252, 1253, 1254];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    /// Both decode the byte(s) to the same character.
    Same,
    /// Neither decodes them (glibc skips, encoding_rs writes U+FFFD).
    BothInvalid,
    /// glibc rejects the byte(s); encoding_rs maps them to some character
    /// (a C1 control for WHATWG holes, a private-use code point for GBK/Big5
    /// user-defined areas).
    GlibcInvalid,
    /// glibc maps the byte(s) to a character; encoding_rs writes U+FFFD.
    ErsInvalid,
    /// Both map them, to different characters.
    DifferentCharacter,
}

/// The one character `seq` decodes to, if it is a valid character of its own
/// (not a replacement and not a skipped lead followed by the trail byte read
/// on its own).
fn single(decode: impl Fn(&[u8]) -> String, seq: &[u8]) -> Option<String> {
    let mut input = seq.to_vec();
    input.push(b' '); // flushes glibc's held combining characters
    let s = decode(&input);
    let s = s.strip_suffix(' ')?;
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_some() && s != "\u{fffd}" && !s.chars().all(|c| c != '\u{fffd}') {
        return None;
    }
    if s.contains('\u{fffd}') {
        return None;
    }
    if seq.len() == 2 {
        // [lead, trail] where the lead was dropped and the trail decoded alone.
        let mut alone = vec![seq[1], b' '];
        let t = decode(&alone);
        alone.clear();
        if t.strip_suffix(' ') == Some(s) {
            return None;
        }
    }
    Some(if s.chars().count() == 1 { c.to_string() } else { s.to_string() })
}

fn classify(cp: u32, seq: &[u8]) -> (Class, String, String) {
    let glibc = |b: &[u8]| -> String {
        locale::decode(cp, b).iter().map(|&c| char::from_u32(c).unwrap_or('\u{fffd}')).collect()
    };
    let ers = |b: &[u8]| utf8::decode(cp, b).into_owned();
    let (g, e) = (single(glibc, seq), single(ers, seq));
    let class = match (&g, &e) {
        (Some(g), Some(e)) if g == e => Class::Same,
        (Some(_), Some(_)) => Class::DifferentCharacter,
        (None, None) => Class::BothInvalid,
        (None, Some(_)) => Class::GlibcInvalid,
        (Some(_), None) => Class::ErsInvalid,
    };
    (class, g.unwrap_or_default(), e.unwrap_or_default())
}

fn all_sequences(cp: u32) -> Vec<Vec<u8>> {
    let mut v: Vec<Vec<u8>> = (0x80..=0xffu32).map(|b| vec![b as u8]).collect();
    if matches!(cp, 932 | 936 | 949 | 950) {
        for lead in 0x81..=0xfeu32 {
            for trail in 0x40..=0xfeu32 {
                v.push(vec![lead as u8, trail as u8]);
            }
        }
    }
    v
}

#[test]
fn report_per_code_page() {
    println!("{:>5} {:>7} {:>8} {:>13} {:>13} {:>10}", "cp", "same", "both-inv", "glibc-inv", "ers-inv", "different");
    for cp in PAGES {
        let mut counts: BTreeMap<Class, usize> = BTreeMap::new();
        let mut examples: BTreeMap<Class, Vec<String>> = BTreeMap::new();
        for seq in all_sequences(cp) {
            let (class, g, e) = classify(cp, &seq);
            *counts.entry(class).or_default() += 1;
            let ex = examples.entry(class).or_default();
            if class != Class::Same && class != Class::BothInvalid && ex.len() < 4 {
                ex.push(format!("{seq:02x?} glibc={g:?} ers={e:?}"));
            }
        }
        let n = |c| counts.get(&c).copied().unwrap_or(0);
        println!(
            "{cp:>5} {:>7} {:>8} {:>13} {:>13} {:>10}",
            n(Class::Same),
            n(Class::BothInvalid),
            n(Class::GlibcInvalid),
            n(Class::ErsInvalid),
            n(Class::DifferentCharacter)
        );
        for (class, ex) in examples {
            for e in ex {
                println!("        {class:?}: {e}");
            }
        }
        if SHIPPED.contains(&cp) {
            assert_eq!(n(Class::DifferentCharacter), 0, "code page {cp}");
            assert_eq!(n(Class::ErsInvalid), 0, "code page {cp}");
        }
    }
}

/// Single-byte differences of the single-byte pages, byte by byte.
#[test]
fn list_single_byte_differences() {
    for cp in [1250, 1251, 1252, 1253, 1254, 874] {
        let mut by_class: BTreeMap<Class, Vec<u8>> = BTreeMap::new();
        for b in 0x80..=0xffu8 {
            let (class, _, _) = classify(cp, &[b]);
            if class != Class::Same {
                by_class.entry(class).or_default().push(b);
            }
        }
        for (class, bytes) in by_class {
            println!("cp{cp} {class:?}: {bytes:02x?}");
        }
    }
}
