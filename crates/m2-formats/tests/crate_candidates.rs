//! Evidence for the "crate or own code" decision per codec (D16): how far do
//! off-the-shelf decoders get from the C++ output on the real client files?
//! The own port (`m2_formats::image`) is identical to C++ on all of them (see
//! `differential_image.rs`), so it stands in for C++ here.
//!
//! Prints mismatch counts; run with `scripts/differential.sh` style env:
//! `M2_CLIENT_DIR=bin M2_DIFF_BUILD=x cargo test --release -p m2-formats --test crate_candidates -- --ignored --nocapture`

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use m2_formats::image::dxt::DxtcImage;
use m2_formats::image::stb;
use support::differential::client_dir;

fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        let p = e.path();
        if p.is_dir() {
            files(&p, ext, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)) {
            out.push(p);
        }
    }
}

fn all(ext: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    files(&client_dir().join("pack"), ext, &mut v);
    v.sort();
    v
}

/// (files differing, files total, max abs channel difference)
fn compare_with_image_crate(ext: &str) -> (usize, usize, u8) {
    let (mut differ, mut total, mut max) = (0, 0, 0u8);
    for p in all(ext) {
        let bytes = fs::read(&p).unwrap();
        let Some(ours) = stb::load_rgba(&bytes) else { continue };
        total += 1;
        let theirs = image::load_from_memory(&bytes).map(|i| i.to_rgba8());
        match theirs {
            Ok(t) if t.width() == ours.width && t.height() == ours.height => {
                let d = t.as_raw().iter().zip(&ours.pixels).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
                if d != 0 {
                    differ += 1;
                    max = max.max(d);
                }
            }
            _ => {
                differ += 1;
                max = 255;
            }
        }
    }
    (differ, total, max)
}

#[test]
#[ignore = "evidence: needs the client data"]
fn image_crate_vs_stb_port() {
    for ext in ["jpg", "tga", "bmp"] {
        let (differ, total, max) = compare_with_image_crate(ext);
        eprintln!(
            "image 0.25 (zune-jpeg / tga / bmp) vs stb on .{ext}: {differ}/{total} files differ, max channel delta {max}"
        );
    }
}

/// A conventional DXT1 decoder (5/6 bit channels replicated to 8 bits, as
/// every GPU and library does) against `CDXTCImage`'s `<< 3` expansion.
#[test]
#[ignore = "evidence: needs the client data"]
fn conventional_dxt1_vs_cdxtc() {
    let expand5 = |v: u16| u32::from((v << 3) | (v >> 2));
    let expand6 = |v: u16| u32::from((v << 2) | (v >> 4));
    let (mut differ, mut total) = (0, 0);
    for p in all("dds") {
        let bytes = fs::read(&p).unwrap();
        let Some(d) = DxtcImage::load_header(&bytes) else { continue };
        if d.format_name != "DXT1" {
            continue;
        }
        let Some(comp) = d.copy(&bytes, 0) else { continue };
        let Some(ours) = d.decompress(0, comp) else { continue };
        total += 1;
        let (w, h) = (d.width as usize, d.height as usize);
        let mut bad = false;
        'blocks: for by in 0..h / 4 {
            for bx in 0..w / 4 {
                let b = &comp[(by * (w / 4) + bx) * 8..][..8];
                let (c0, c1) = (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]]));
                let rgb = |c: u16| [expand5(c >> 11), expand6((c >> 5) & 63), expand5(c & 31)];
                let (r0, r1) = (rgb(c0), rgb(c1));
                let mix = |a: u32, b: u32, wa: u32, wb: u32, div: u32| (a * wa + b * wb) / div;
                let pal: [[u32; 4]; 4] = if c0 > c1 {
                    [
                        [r0[0], r0[1], r0[2], 255],
                        [r1[0], r1[1], r1[2], 255],
                        [mix(r0[0], r1[0], 2, 1, 3), mix(r0[1], r1[1], 2, 1, 3), mix(r0[2], r1[2], 2, 1, 3), 255],
                        [mix(r0[0], r1[0], 1, 2, 3), mix(r0[1], r1[1], 1, 2, 3), mix(r0[2], r1[2], 1, 2, 3), 255],
                    ]
                } else {
                    [
                        [r0[0], r0[1], r0[2], 255],
                        [r1[0], r1[1], r1[2], 255],
                        [mix(r0[0], r1[0], 1, 1, 2), mix(r0[1], r1[1], 1, 1, 2), mix(r0[2], r1[2], 1, 1, 2), 255],
                        [0, 0, 0, 0],
                    ]
                };
                for row in 0..4 {
                    for n in 0..4 {
                        let c = pal[usize::from((b[4 + row] >> (2 * n)) & 3)];
                        let px = c[3] << 24 | c[0] << 16 | c[1] << 8 | c[2];
                        if px != ours[(by * 4 + row) * w + bx * 4 + n] {
                            bad = true;
                            break 'blocks;
                        }
                    }
                }
            }
        }
        differ += usize::from(bad);
    }
    eprintln!("conventional DXT1 vs CDXTCImage: {differ}/{total} files differ");
}
