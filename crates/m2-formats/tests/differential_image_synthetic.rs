//! Differential tests for formats and variants the client packs do not contain
//! (PNG, more BMP and TGA flavours, odd DDS headers): synthetic files are fed to
//! both `m2diff_image` and `m2-image-dump`, which must print the same text.
//!
//! Needs the C++ tool, so the test is ignored by default; run it with
//! `scripts/differential.sh`.

mod support;

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use support::deterministic_bytes as rnd;
use support::differential::{assert_same_lines, cpp_tool, run};

const RUST_IMAGE_DUMP: &str = env!("CARGO_BIN_EXE_m2-image-dump");

fn tga(img_type: u8, bits: u8, desc: u8, w: u16, h: u16, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0, 0, img_type, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.push(bits);
    v.push(desc);
    v.extend_from_slice(body);
    v
}

fn bmp(w: i32, h: i32, bpp: u16, compress: u32, palette: &[u8], extra_header: &[u8], pixels: &[u8]) -> Vec<u8> {
    let hsz = 40 + extra_header.len() as u32;
    let offset = 14 + hsz + palette.len() as u32;
    let mut v = b"BM".to_vec();
    v.extend_from_slice(&(offset + pixels.len() as u32).to_le_bytes());
    v.extend_from_slice(&[0; 4]);
    v.extend_from_slice(&offset.to_le_bytes());
    v.extend_from_slice(&hsz.to_le_bytes());
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&bpp.to_le_bytes());
    v.extend_from_slice(&compress.to_le_bytes());
    v.extend_from_slice(&[0; 20]);
    v.extend_from_slice(extra_header);
    v.extend_from_slice(palette);
    v.extend_from_slice(pixels);
    v
}

fn png_file(
    color: png::ColorType,
    depth: png::BitDepth,
    w: u32,
    h: u32,
    seed: u32,
    extra: impl FnOnce(&mut png::Encoder<&mut Vec<u8>>),
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(color);
    enc.set_depth(depth);
    extra(&mut enc);
    let mut writer = enc.write_header().unwrap();
    let bits = depth as usize;
    let row = (w as usize * color.samples() * bits).div_ceil(8);
    writer.write_image_data(&rnd(row * h as usize, seed)).unwrap();
    writer.finish().unwrap();
    out
}

fn dds(fourcc: &[u8; 4], w: u32, h: u32, mips: u32, flags: u32, linear: u32, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; 128];
    v[..4].copy_from_slice(b"DDS ");
    let mut put = |o: usize, x: u32| v[o..o + 4].copy_from_slice(&x.to_le_bytes());
    put(4, 124);
    put(8, flags);
    put(12, h);
    put(16, w);
    put(20, linear);
    put(28, mips);
    put(76, 32);
    put(80, 4);
    v[84..88].copy_from_slice(fourcc);
    v.extend_from_slice(payload);
    v
}

fn synthetic() -> Vec<(String, Vec<u8>)> {
    use png::{BitDepth::*, ColorType::*};
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut add = |name: &str, data: Vec<u8>| files.push((name.to_string(), data));

    // PNG
    add("g8.png", png_file(Grayscale, Eight, 7, 5, 1, |_| {}));
    add("g2.png", png_file(Grayscale, Two, 9, 3, 2, |_| {}));
    add("ga8.png", png_file(GrayscaleAlpha, Eight, 6, 6, 3, |_| {}));
    add("rgb8.png", png_file(Rgb, Eight, 13, 4, 4, |_| {}));
    add("rgba8.png", png_file(Rgba, Eight, 5, 9, 5, |_| {}));
    add("rgb16.png", png_file(Rgb, Sixteen, 4, 4, 6, |_| {}));
    add("g16.png", png_file(Grayscale, Sixteen, 4, 4, 7, |_| {}));
    add(
        "pal4_trns.png",
        png_file(Indexed, Four, 8, 8, 8, |e| {
            e.set_palette((0..16u8).flat_map(|i| [i * 16, 255 - i * 8, i]).collect::<Vec<_>>());
            e.set_trns(vec![0, 128, 255, 17]);
        }),
    );
    add("rgb8_trns.png", png_file(Rgb, Eight, 4, 4, 9, |e| e.set_trns(vec![0, 10, 0, 20, 0, 30])));

    // BMP
    let pal4: Vec<u8> = (0..16u8).flat_map(|i| [i * 3, i * 7, i * 11, 0]).collect();
    add("bgr24.bmp", bmp(5, 3, 24, 0, &[], &[], &rnd(3 * 16, 10)));
    add("bgr24_topdown.bmp", bmp(5, -3, 24, 0, &[], &[], &rnd(3 * 16, 11)));
    add("bgra32.bmp", bmp(4, 4, 32, 0, &[], &[], &rnd(64, 12)));
    let noalpha: Vec<u8> = rnd(64, 13).chunks(4).flat_map(|p| [p[0], p[1], p[2], 0]).collect();
    add("bgra32_noalpha.bmp", bmp(4, 4, 32, 0, &[], &[], &noalpha));
    add("pal8.bmp", bmp(6, 2, 8, 0, &rnd(256 * 4, 14), &[], &rnd(16, 15)));
    add("pal4.bmp", bmp(7, 3, 4, 0, &pal4, &[], &rnd(4 * 3, 16)));
    add("pal1.bmp", bmp(10, 3, 1, 0, &[0, 0, 0, 0, 255, 128, 64, 0], &[], &rnd(8, 17)));
    add("rgb555.bmp", bmp(3, 3, 16, 0, &[], &[], &rnd(8 * 3, 18)));
    let mut bf = Vec::new();
    for m in [0xf800u32, 0x07e0, 0x001f] {
        bf.extend_from_slice(&m.to_le_bytes());
    }
    add("rgb565.bmp", bmp(3, 3, 16, 3, &[], &bf, &rnd(8 * 3, 19)));

    // TGA, read by both stb and CTGAImage
    let px32 = rnd(4 * 3 * 4, 20);
    add("t2_32_bl.tga", tga(2, 32, 8, 4, 3, &px32));
    add("t2_32_tl.tga", tga(2, 32, 0x28, 4, 3, &px32));
    add("t2_24_bl.tga", tga(2, 24, 0, 5, 3, &rnd(5 * 3 * 3, 21)));
    add("t2_16_tl.tga", tga(2, 16, 0x20, 5, 3, &rnd(5 * 3 * 2, 22)));
    add("t3_8.tga", tga(3, 8, 0, 6, 2, &rnd(12, 23)));
    // RLE: a repeat of 5 then a raw run of 3, 24 and 32 bit
    let mut rle24 = vec![0x84, 1, 2, 3, 0x02, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    add("t10_24.tga", tga(10, 24, 0, 8, 1, &rle24));
    rle24[0] = 0x83;
    add("t10_24_short_run.tga", tga(10, 24, 0x20, 7, 1, &rle24));
    let rle32 = vec![0x83, 1, 2, 3, 4, 0x03, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 11, 12, 13, 14, 15, 16];
    add("t10_32.tga", tga(10, 32, 0, 8, 1, &rle32));
    // RLE overflowing the pixel count: both fail
    add("t10_overflow.tga", tga(10, 24, 0, 2, 1, &[0x84, 1, 2, 3]));
    // stb only: paletted with a 4 entry 24 bit palette
    let mut pal = tga(1, 8, 0, 4, 2, &[]);
    pal[1] = 1;
    pal[5] = 4;
    pal[7] = 24;
    pal.extend_from_slice(&rnd(12, 24));
    pal.extend_from_slice(&[0, 1, 2, 3, 3, 2, 1, 0]);
    add("t1_pal.tga", pal);

    // DDS: random blocks, with and without the mip chain and with a pitch flag
    for (name, cc, blk) in [("dxt1", b"DXT1", 8usize), ("dxt3", b"DXT3", 16), ("dxt5", b"DXT5", 16)] {
        let level_bytes = |w: usize, h: usize| (w / 4) * (h / 4) * blk;
        let total: usize = (0..3).map(|l| level_bytes(16 >> l, 8 >> l)).sum();
        let linear = level_bytes(16, 8) as u32;
        add(&format!("{name}_mips.dds"), dds(cc, 16, 8, 3, 0x000A_1007, linear, &rnd(total, 30)));
        add(&format!("{name}_one.dds"), dds(cc, 16, 8, 1, 0x0008_1007, linear, &rnd(total, 31)));
        add(&format!("{name}_pitch.dds"), dds(cc, 16, 8, 1, 0x0001_000F, 64, &rnd(total, 32)));
        add(&format!("{name}_odd.dds"), dds(cc, 10, 6, 2, 0x000A_1007, level_bytes(8, 4) as u32, &rnd(total, 33)));
        add(&format!("{name}_truncated.dds"), dds(cc, 16, 8, 3, 0x000A_1007, linear, &rnd(total / 2, 34)));
    }
    add("unknown_fourcc.dds", dds(b"ATI2", 8, 8, 1, 0x0008_1007, 64, &rnd(64, 35)));
    files
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_images_match_cpp() {
    let dir = tempfile::tempdir().unwrap();
    let files = synthetic();
    let paths: Vec<_> = files
        .iter()
        .map(|(name, data)| {
            let p = dir.path().join(name);
            fs::write(&p, data).unwrap();
            p
        })
        .collect();
    let mut args: Vec<&OsStr> = vec![OsStr::new("--full"), OsStr::new("files")];
    args.extend(paths.iter().map(|p| p.as_os_str()));

    let cpp = run(&cpp_tool("image"), &args);
    let rust = run(Path::new(RUST_IMAGE_DUMP), &args);
    let lines = assert_same_lines(&cpp, &rust);
    let decoded = cpp.split(|&b| b == b'\n').filter(|l| l.starts_with(b"decode ") && *l != b"decode -").count();
    eprintln!("{} synthetic files, {lines} lines identical, {decoded} decoded by stb", files.len());
}
