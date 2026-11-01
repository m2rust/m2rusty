#![cfg(feature = "glibc-tables")]
//! Differential tests: the Rust locale codecs against `neoeter/src/platform/locale`
//! (and so glibc iconv). `m2diff_locale` and `m2-locale-dump` print the same
//! dumps, which must be identical:
//! - every code page: all single bytes, all byte pairs, combining triples
//!   (CP1255, CP1258), UTF-8 sequence classes, the BMP and a sample of the
//!   other code points, each with the size query and a one-unit buffer;
//! - the CP1258 `Ymir_*` codec, Arabic shaping, Shift-JIS helpers;
//! - every `.txt` of the client's locale packs, line by line in its locale's
//!   code page, and whole in every code page.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use m2_formats::pack::{PackManager, SearchMode, load_index};
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

const RUST_LOCALE_DUMP: &str = env!("CARGO_BIN_EXE_m2-locale-dump");

fn assert_same_dump(args: &[&OsStr]) -> usize {
    let cpp = run(&cpp_tool("locale"), args);
    let rust = run(Path::new(RUST_LOCALE_DUMP), args);
    assert_same_lines(&cpp, &rust)
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn codecs_match_cpp() {
    // CP_ACP and an unknown code page are UTF-8 too.
    for cp in [0, 437, 65001, 949, 932, 936, 950, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258, 874] {
        let cp = cp.to_string();
        let lines = assert_same_dump(&["codec".as_ref(), cp.as_ref()]);
        eprintln!("code page {cp}: {lines} lines identical");
    }
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn helpers_match_cpp() {
    for command in ["vietnamese", "arabic", "japanese"] {
        let lines = assert_same_dump(&[command.as_ref()]);
        eprintln!("{command}: {lines} lines identical");
    }
}

/// Code page of each locale pack (`gs_stLocaleData` in client/src/ui/Locale.cpp;
/// Greece is listed there as "tr", 1253).
fn locale_code_page(locale: &str) -> Option<u32> {
    Some(match locale {
        "de" | "en" | "es" | "it" | "fr" | "pt" | "dk" | "nl" | "cz" => 1252,
        "pl" | "hu" | "ro" => 1250,
        "tr" => 1254,
        "ru" => 1251,
        "gr" => 1253,
        _ => return None,
    })
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn locale_texts_match_cpp() {
    let mut manager = PackManager::new(client_dir());
    manager.set_search_mode(SearchMode::PackFirst);
    load_index(&mut manager, "pack", "pack/Index.dev").unwrap();

    // locale/<xx>/...txt, grouped by locale.
    let mut by_locale: BTreeMap<String, Vec<Vec<u8>>> = BTreeMap::new();
    for name in manager.file_names() {
        let Some(rest) = name.strip_prefix(b"locale/") else {
            continue;
        };
        let Some(slash) = rest.iter().position(|&b| b == b'/') else {
            continue;
        };
        if name.ends_with(b".txt") {
            let locale = String::from_utf8_lossy(&rest[..slash]).into_owned();
            by_locale.entry(locale).or_default().push(name.to_vec());
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let (mut files, mut lines) = (0, 0);
    for (locale, mut names) in by_locale {
        let cp = locale_code_page(&locale).unwrap_or_else(|| panic!("no code page for locale {locale}"));
        names.sort();
        let locale_dir = dir.path().join(&locale);
        fs::create_dir_all(&locale_dir).unwrap();
        let paths: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let path = locale_dir.join(format!("{i}.txt"));
                fs::write(&path, manager.get(name).unwrap()).unwrap();
                path
            })
            .collect();

        let cp = cp.to_string();
        let mut args: Vec<&OsStr> = vec!["corpus".as_ref(), cp.as_ref()];
        args.extend(paths.iter().map(|p| p.as_os_str()));
        lines += assert_same_dump(&args);
        files += paths.len();
    }
    assert!(files > 0, "no locale texts found");
    eprintln!("{files} files, {lines} lines identical");
}
