//! Differential tests: the Rust gr2 reader against `GrannyParseGR2FromMemory`
//! on every `.gr2` in the client packs, on synthetic files and on damaged
//! copies of real files. Both dumpers print the same canonical text.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

#[path = "gr2.rs"]
mod gr2_cases;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

const RUST_GR2_DUMP: &str = env!("CARGO_BIN_EXE_m2-gr2-dump");

fn timed(tool: &Path, args: &[&OsStr]) -> (Vec<u8>, Duration) {
    let start = Instant::now();
    let out = run(tool, args);
    (out, start.elapsed())
}

/// Runs both dumpers with `args`, asserts identical output, returns it.
fn assert_same_dump(args: &[&OsStr]) -> Vec<u8> {
    let (cpp, cpp_time) = timed(&cpp_tool("gr2"), args);
    let (rust, rust_time) = timed(Path::new(RUST_GR2_DUMP), args);
    let lines = assert_same_lines(&cpp, &rust);
    eprintln!("{lines} lines identical (c++ {cpp_time:.2?}, rust {rust_time:.2?})");
    cpp
}

fn count(dump: &[u8], prefix: &[u8]) -> usize {
    dump.split(|&b| b == b'\n').filter(|l| l.starts_with(prefix)).count()
}

/// Every `.gr2` under the client's pack folders, sorted.
fn gr2_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            gr2_files(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("gr2")) {
            out.push(path);
        }
    }
}

fn sample(step: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    gr2_files(&client_dir().join("pack"), &mut files);
    files.sort();
    files.into_iter().step_by(step).collect()
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn all_pack_gr2_match_cpp() {
    let client = client_dir();
    let dump = assert_same_dump(&[OsStr::new("pack"), client.as_os_str(), OsStr::new("pack/Index.dev")]);
    let files = count(&dump, b"file ");
    assert!(files > 1000, "only {files} gr2 files");
    eprintln!(
        "{files} files ({} rejected by both), {} models, {} meshes, {} animations, {} tracks",
        count(&dump, b"error"),
        count(&dump, b"model "),
        count(&dump, b" mesh "),
        count(&dump, b"animation "),
        count(&dump, b"  track "),
    );

    // How many of them exercise the decompressor.
    let compressed = sample(1)
        .iter()
        .filter(|p| {
            let b = fs::read(p).unwrap();
            if b.len() < 88 {
                return false;
            }
            let u = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize;
            let (table, n) = (32 + u(44), u(48));
            (0..n.min(64)).any(|i| b.get(table + i * 44..table + i * 44 + 4) == Some(&2u32.to_le_bytes()[..]))
        })
        .count();
    eprintln!("{compressed} files on disk have Oodle1-compressed sections");
}

/// Element-by-element dump of every 40th real file.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn full_dump_of_real_sample_matches_cpp() {
    let files = sample(40);
    let mut args: Vec<&OsStr> = vec![OsStr::new("--full"), OsStr::new("files")];
    args.extend(files.iter().map(|p| p.as_os_str()));
    assert_same_dump(&args);
    eprintln!("{} files", files.len());
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_files_match_cpp() {
    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for (name, bytes) in gr2_cases::fixture::cases() {
        let path = dir.path().join(format!("{name}.gr2"));
        fs::write(&path, bytes).unwrap();
        paths.push(path);
    }
    let mut args: Vec<&OsStr> = vec![OsStr::new("--full"), OsStr::new("files")];
    args.extend(paths.iter().map(|p| p.as_os_str()));
    let dump = assert_same_dump(&args);
    assert_eq!(count(&dump, b"error"), 7, "expected rejections changed");
}

/// Truncated real files (cut inside section data, relocations, the table)
/// and real files with a byte flipped in their compressed data.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn damaged_real_files_match_cpp() {
    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for (i, src) in sample(150).iter().enumerate() {
        let bytes = fs::read(src).unwrap();
        let mut variants = Vec::new();
        for frac in [10, 50, 90, 99] {
            variants.push(bytes[..bytes.len() * frac / 100].to_vec());
        }
        // A flip well inside the file, past the header and section table.
        for frac in [30, 60, 95] {
            let mut b = bytes.clone();
            let at = bytes.len() * frac / 100;
            b[at] ^= 0x5a;
            variants.push(b);
        }
        for (j, v) in variants.into_iter().enumerate() {
            let path = dir.path().join(format!("{i:04}_{j}.gr2"));
            fs::write(&path, v).unwrap();
            paths.push(path);
        }
    }
    let mut args: Vec<&OsStr> = vec![OsStr::new("files")];
    args.extend(paths.iter().map(|p| p.as_os_str()));
    let dump = assert_same_dump(&args);
    eprintln!(
        "{} damaged files, {} rejected by both",
        paths.len(),
        count(&dump, b"error")
    );
}
