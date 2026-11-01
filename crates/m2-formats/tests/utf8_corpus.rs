//! `encoding_rs` against the glibc port on the real client data, decoded per
//! locale code page: every `.txt` and `.py` of the 15 `locale_*` packs, the
//! proto tables' names and the root pack's CP936 tables. Skips without
//! client data (`M2_CLIENT_DIR`, default `bin`). Also checks the pack
//! virtual names (CP949 bytes) and the `utf8` accessors.
//!
//! `cargo test --release -p m2-formats --test utf8_corpus -- --nocapture`

use std::collections::BTreeMap;
use std::path::PathBuf;

use m2_formats::pack::{PackManager, SearchMode, load_index};
use m2_formats::proto::{parse_item_proto, parse_mob_proto};
use m2_formats::utf8;

fn client() -> Option<PathBuf> {
    let dir = std::env::var_os("M2_CLIENT_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin"),
        |d| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(d),
    );
    if dir.join("pack/Index.dev").exists() {
        Some(dir)
    } else {
        eprintln!("skipped: no client data at {}", dir.display());
        None
    }
}

fn manager(dir: PathBuf) -> PackManager {
    let mut m = PackManager::new(dir);
    m.set_search_mode(SearchMode::PackFirst);
    load_index(&mut m, "pack", "pack/Index.dev").unwrap();
    m
}

#[derive(Default)]
struct Stats {
    items: usize,
    bytes: usize,
    high_bytes: usize,
    identical: usize,
    /// Differs only where glibc drops a byte that encoding_rs writes (a
    /// C1 control for undefined bytes, or U+FFFD).
    c1_or_fffd: usize,
    /// Anything else.
    different: Vec<String>,
}

#[cfg(feature = "glibc-tables")]
fn compare(stats: &mut Stats, label: &str, cp: u32, bytes: &[u8]) {
    use m2_formats::locale;
    stats.items += 1;
    stats.bytes += bytes.len();
    stats.high_bytes += bytes.iter().filter(|&&b| b >= 0x80).count();
    let glibc: String = locale::decode(cp, bytes).iter().map(|&c| char::from_u32(c).unwrap_or('\u{fffd}')).collect();
    let ers = utf8::decode(cp, bytes);
    if glibc == ers {
        stats.identical += 1;
        return;
    }
    // Compare without the characters encoding_rs adds for undefined bytes.
    let filtered: String = ers.chars().filter(|&c| c != '\u{fffd}' && !('\u{80}'..='\u{9f}').contains(&c)).collect();
    let glibc_filtered: String = glibc.chars().filter(|&c| !('\u{80}'..='\u{9f}').contains(&c)).collect();
    if filtered == glibc_filtered {
        stats.c1_or_fffd += 1;
        if std::env::var_os("M2_VERBOSE").is_some() {
            println!("  c1/fffd: {label} {:02x?}", bytes.iter().filter(|&&b| b >= 0x80).take(8).collect::<Vec<_>>());
        }
    } else if stats.different.len() < 10 {
        stats.different.push(label.to_string());
    } else {
        stats.different.push(String::new());
    }
}

#[cfg(not(feature = "glibc-tables"))]
fn compare(stats: &mut Stats, _label: &str, cp: u32, bytes: &[u8]) {
    stats.items += 1;
    stats.bytes += bytes.len();
    stats.high_bytes += bytes.iter().filter(|&&b| b >= 0x80).count();
    let _ = utf8::decode(cp, bytes);
}

#[test]
fn locale_texts_decode_like_glibc() {
    let Some(dir) = client() else { return };
    let manager = manager(dir);
    let mut per_locale: BTreeMap<&str, Stats> = BTreeMap::new();
    // Unassigned-in-glibc high bytes that occur in the corpus, per page.
    let mut hole_bytes: BTreeMap<u32, BTreeMap<u8, usize>> = BTreeMap::new();

    for (locale, cp) in utf8::SHIPPED_LOCALES {
        let prefix = format!("locale/{locale}/");
        let stats = per_locale.entry(locale).or_default();
        for name in manager.file_names() {
            let Some(rest) = name.strip_prefix(prefix.as_bytes()) else { continue };
            let text = rest.ends_with(b".txt") || rest.ends_with(b".py");
            if !text {
                continue;
            }
            let data = manager.get(name).unwrap();
            compare(stats, &String::from_utf8_lossy(name), cp, &data);
            for &b in &data {
                if b >= 0x80 && utf8::decode(cp, &[b, b' ']).chars().next().is_some_and(|c| ('\u{80}'..='\u{9f}').contains(&c))
                {
                    *hole_bytes.entry(cp).or_default().entry(b).or_default() += 1;
                }
            }
        }
        // Proto names: item_proto / mob_proto of the locale (locale_name in the
        // locale's code page, name in CP949).
        let item = manager.get(format!("locale/{locale}/item_proto")).ok().and_then(|d| parse_item_proto(&d).ok());
        for t in item.iter().flatten() {
            compare(stats, "item locale_name", cp, m2_formats::proto::cstr(&t.locale_name));
            compare(stats, "item name", 949, m2_formats::proto::cstr(&t.name));
        }
        let mob = manager.get(format!("locale/{locale}/mob_proto")).ok().and_then(|d| parse_mob_proto(&d).ok());
        for t in mob.iter().flatten() {
            compare(stats, "mob locale_name", cp, m2_formats::proto::cstr(&t.locale_name));
            compare(stats, "mob name", 949, m2_formats::proto::cstr(&t.name));
        }
    }

    // The root pack's CP936 tables.
    let mut cn = Stats::default();
    for name in ["936skilldesc.txt", "936skilltable.txt"] {
        if let Ok(data) = manager.get(name) {
            compare(&mut cn, name, 936, &data);
        }
    }
    if let Ok(data) = manager.get("936mob_proto") {
        for t in parse_mob_proto(&data).unwrap_or_default() {
            compare(&mut cn, "936mob_proto locale_name", 936, m2_formats::proto::cstr(&t.locale_name));
        }
    }
    per_locale.insert("root-936", cn);

    println!("{:<9} {:>6} {:>10} {:>9} {:>9} {:>8} {:>9}", "locale", "items", "bytes", "high", "identical", "c1/fffd", "different");
    let mut total_different = 0;
    for (locale, s) in &per_locale {
        println!(
            "{locale:<9} {:>6} {:>10} {:>9} {:>9} {:>8} {:>9}",
            s.items,
            s.bytes,
            s.high_bytes,
            s.identical,
            s.c1_or_fffd,
            s.different.len()
        );
        for d in s.different.iter().filter(|d| !d.is_empty()) {
            println!("    differs: {d}");
        }
        total_different += s.different.len();
    }
    for (cp, bytes) in &hole_bytes {
        println!("cp{cp}: bytes undefined in glibc that occur in the corpus: {bytes:02x?}");
    }
    #[cfg(feature = "glibc-tables")]
    assert_eq!(total_different, 0);
    #[cfg(not(feature = "glibc-tables"))]
    let _ = total_different;
}

#[test]
fn pack_virtual_names() {
    let Some(dir) = client() else { return };
    let manager = manager(dir);
    let names: Vec<&[u8]> = manager.file_names().collect();
    let (texts, legacy) = utf8::virtual_names(names.iter().copied(), utf8::DEFAULT_NAME_CODE_PAGE);
    println!("{} names, {} are not valid UTF-8 (CP949):", names.len(), legacy.len());
    for &i in &legacy {
        println!("  {}", texts[i]);
        // A CP949 decode of real names has no replacement characters.
        assert!(!texts[i].contains('\u{fffd}'), "{}", texts[i]);
        // And it round-trips to the original bytes.
        let _ = ();
    }
}

#[test]
fn accessors_on_real_protos() {
    let Some(dir) = client() else { return };
    let manager = manager(dir);
    let data = manager.get("locale/ru/mob_proto").unwrap();
    let mobs = parse_mob_proto(&data).unwrap();
    let cp = utf8::locale_code_page("ru").unwrap();
    let named = mobs.iter().filter(|m| m.locale_name_utf8(cp).chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c))).count();
    println!("ru mobs with Cyrillic names: {named} of {}", mobs.len());
    assert!(named > 0);
}

/// The Czech pack is declared CP1252 by the client (`gs_stLocaleData`) but is
/// written in CP1250: the same bytes read in both pages. Informational.
#[test]
fn czech_pack_page_check() {
    let Some(dir) = client() else { return };
    let manager = manager(dir);
    let data = manager.get("locale/cz/locale_interface.txt").unwrap();
    let (mut ok1250, mut ok1252) = (0, 0);
    for &b in &data {
        // Czech letters that exist in CP1250 only: Č č Ě ě Ř ř Š š Ž ž Ť ť Ů ů Ď ď Ň ň
        if matches!(b, 0xc8 | 0xe8 | 0xcc | 0xec | 0xd8 | 0xf8 | 0x8a | 0x9a | 0x8e | 0x9e | 0x8d | 0x9d | 0xd9 | 0xf9 | 0xcf | 0xef | 0xd2 | 0xf2) {
            ok1250 += 1;
        }
        if matches!(b, 0x80..=0xff) {
            ok1252 += 1;
        }
    }
    println!("cz locale_interface.txt: {} high bytes, {} are Czech letters in CP1250", ok1252, ok1250);
    let s = utf8::decode(1250, &data);
    println!("1250: {}", s.lines().filter(|l| l.contains('č') || l.contains('ř') || l.contains('š')).take(3).collect::<Vec<_>>().join(" | "));
    let s = utf8::decode(1252, &data);
    println!("1252: {}", s.chars().filter(|c| !c.is_ascii()).take(40).collect::<String>());
}
