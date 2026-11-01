//! Differential tests: the Rust image decoders against the C++ ones
//! (`CDXTCImage`, `CTGAImage`, `Platform_GetImageInfo`, `Platform_DecodeImage`)
//! on every `.dds`, `.tga`, `.jpg` and `.bmp` in the client packs, plus
//! synthetic files for formats the packs do not contain. Both dumpers print the
//! same canonical text, with an fnv1a64 per decoded image and full pixel dumps
//! for a sample of files.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

const RUST_IMAGE_DUMP: &str = env!("CARGO_BIN_EXE_m2-image-dump");

fn timed(tool: &Path, args: &[&OsStr]) -> (Vec<u8>, Duration) {
    let start = Instant::now();
    let out = run(tool, args);
    (out, start.elapsed())
}

/// Runs both dumpers with `args`, asserts identical output, returns it.
fn assert_same_dump(args: &[&OsStr]) -> Vec<u8> {
    let (cpp, cpp_time) = timed(&cpp_tool("image"), args);
    let (rust, rust_time) = timed(Path::new(RUST_IMAGE_DUMP), args);
    let lines = assert_same_lines(&cpp, &rust);
    eprintln!("{lines} lines identical (c++ {cpp_time:.2?}, rust {rust_time:.2?})");
    cpp
}

fn lines(dump: &[u8]) -> impl Iterator<Item = &str> {
    dump.split(|&b| b == b'\n').filter_map(|l| std::str::from_utf8(l).ok())
}

/// Every image file under the client's pack folders, sorted, with its extension.
fn image_files(dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            image_files(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = ext.to_ascii_lowercase();
            if ["dds", "tga", "jpg", "jpeg", "png", "bmp"].contains(&ext.as_str()) {
                out.push((ext, path));
            }
        }
    }
}

fn sample(step: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    image_files(&client_dir().join("pack"), &mut files);
    files.sort();
    let mut per_ext = std::collections::BTreeMap::<String, usize>::new();
    files
        .into_iter()
        .filter(|(ext, _)| {
            let n = per_ext.entry(ext.clone()).or_default();
            *n += 1;
            (*n - 1).is_multiple_of(step)
        })
        .map(|(_, p)| p)
        .collect()
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn all_pack_images_match_cpp() {
    let client = client_dir();
    let dump = assert_same_dump(&[OsStr::new("pack"), client.as_os_str(), OsStr::new("pack/Index.dev")]);

    let mut by_ext = std::collections::BTreeMap::<String, usize>::new();
    let mut dds_formats = std::collections::BTreeMap::<String, usize>::new();
    let mut mips = std::collections::BTreeMap::<String, usize>::new();
    let (mut decoded, mut tga_ok, mut tga_other) = (0, 0, 0);
    for l in lines(&dump) {
        let mut it = l.split(' ');
        match it.next() {
            Some("file") => {
                let name = l.rsplit_once(' ').map_or(l, |(n, _)| n);
                *by_ext.entry(name.rsplit('.').next().unwrap_or("").to_string()).or_default() += 1;
            }
            Some("dds") => {
                let rest: Vec<_> = it.collect();
                if rest.first() != Some(&"fail") {
                    *dds_formats.entry(rest[0].to_string()).or_default() += 1;
                    *mips.entry(format!("{} mips", rest[3])).or_default() += 1;
                } else {
                    *dds_formats.entry("fail".into()).or_default() += 1;
                }
            }
            Some("decode") if l != "decode -" => decoded += 1,
            Some("tga") if it.next() == Some("fail") || l.contains("unsupported") => tga_other += 1,
            Some("tga") => tga_ok += 1,
            _ => {}
        }
    }
    assert!(by_ext["dds"] > 1000 && by_ext["tga"] > 1000, "{by_ext:?}");
    eprintln!("files by extension: {by_ext:?}");
    eprintln!("dds formats: {dds_formats:?}");
    eprintln!("dds mip counts: {mips:?}");
    eprintln!("stb decode ok: {decoded}; CTGAImage ok: {tga_ok}, failed/unsupported: {tga_other}");
}

/// Full per-pixel dumps of a sample (every 25th file of each kind).
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn sampled_pixels_match_cpp() {
    let files = sample(25);
    assert!(files.len() > 100);
    let mut args: Vec<&OsStr> = vec![OsStr::new("--full"), OsStr::new("files")];
    args.extend(files.iter().map(|p| p.as_os_str()));
    let dump = assert_same_dump(&args);
    let px = lines(&dump).filter(|l| l.starts_with("px ")).count();
    eprintln!("{} files, {px} pixel dumps", files.len());
}
