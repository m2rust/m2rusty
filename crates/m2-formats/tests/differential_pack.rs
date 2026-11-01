//! Differential tests: the Rust pack reader against the C++ one on real
//! client data. Both sides print a sorted `name, size, fnv1a64` dump, which
//! must be byte-identical.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use m2_formats::pack::{FolderPack, Provider, normalize};
use m2_formats::text;
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};
use support::{write_raw_name_zip, write_zip};

const RUST_PACK_DUMP: &str = env!("CARGO_BIN_EXE_m2-pack-dump");

fn assert_same_dump(client: &Path, index: &str) -> usize {
    let args = [client.as_os_str(), index.as_ref()];
    let cpp = run(&cpp_tool("pack"), &args);
    let rust = run(Path::new(RUST_PACK_DUMP), &args);
    assert_same_lines(&cpp, &rust)
}

/// Pack names in an index file, without the container.
fn index_packs(index: &Path) -> Vec<String> {
    let bytes = fs::read(index).unwrap();
    text::lines(&bytes)
        .into_iter()
        .filter_map(|line| text::split_by_tab(line).first().map(|n| String::from_utf8_lossy(n).into_owned()))
        .collect()
}

fn find_dir_ignoring_case(parent: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .find(|e| e.path().is_dir() && e.file_name().as_encoded_bytes().eq_ignore_ascii_case(name.as_bytes()))
        .map(|e| e.path())
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn folder_packs_match_cpp() {
    let client = client_dir();
    let files = assert_same_dump(&client, "pack/Index.dev");
    assert!(files > 0, "no files registered");
    eprintln!("{files} files identical");
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn release_index_matches_cpp() {
    // Whatever `pack/Index` resolves to (ZIP packs, possibly missing).
    let client = client_dir();
    assert_same_dump(&client, "pack/Index");
}

/// Zips small real packs, plus an archive with non-UTF-8 names, and checks
/// that libzip and the `zip` crate see the same files.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn zip_packs_match_cpp() {
    let client = client_dir();
    let fixture = tempfile::tempdir().unwrap();
    let pack_dir = fixture.path().join("pack");
    fs::create_dir_all(&pack_dir).unwrap();

    // Up to eight packs under 4 MiB, smallest first.
    let mut candidates: Vec<(u64, String, PathBuf)> = index_packs(&client.join("pack/Index.dev"))
        .into_iter()
        .filter_map(|name| {
            let dir = find_dir_ignoring_case(&client.join("pack"), &name)?;
            let size = dir_size(&dir);
            (size > 0 && size < 4 << 20).then_some((size, name, dir))
        })
        .collect();
    candidates.sort();
    candidates.truncate(8);
    assert!(!candidates.is_empty(), "no small packs found");

    let mut index = String::new();
    for (_, name, dir) in &candidates {
        let pack = FolderPack::open(dir).unwrap();
        let mut entries = Vec::new();
        collect_files(dir, dir, &mut entries);
        let entries: Vec<(&str, &[u8])> = entries.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
        assert_eq!(entries.len(), pack.file_names().len(), "{name}: non-UTF-8 names; pick another pack");
        write_zip(&pack_dir.join(format!("{name}.zip")), &entries);
        index.push_str(&format!("{name}\tZIP\n"));
    }

    // CP949, valid UTF-8, and mixed-case ASCII names without the UTF-8 flag.
    write_raw_name_zip(
        &pack_dir.join("rawnames.zip"),
        &[
            (b"Ymir Work/\xBF\xEB\xB8\xB6.dds", b"cp949"),
            ("caf\u{e9}/a.txt".as_bytes(), b"utf8"),
            (b"Dir\\Back\\Slash.TXT", b"backslash"),
            (b"dup.txt", b"first"),
            (b"DUP.TXT", b"second"),
            (b"emptydir/", b""),
        ],
    );
    index.push_str("rawnames\tZIP\n");
    fs::write(pack_dir.join("Index"), index).unwrap();

    let files = assert_same_dump(fixture.path(), "pack/Index");
    eprintln!("{} packs, {files} files identical", candidates.len() + 1);
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| {
            let path = e.path();
            if path.is_dir() { dir_size(&path) } else { e.metadata().map_or(0, |m| m.len()) }
        })
        .sum()
}

/// Files under `dir` with their path relative to `root`, UTF-8 names only.
fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else if let Some(name) = path.strip_prefix(root).unwrap().to_str() {
            // Two files differing only in case would collide in the zip too.
            if !out.iter().any(|(n, _)| normalize(n) == normalize(name)) {
                out.push((name.to_owned(), fs::read(&path).unwrap()));
            }
        }
    }
}
