//! Unit tests of `m2_formats::image`: the cases of `neoeter/src/image/tests`
//! (`test_image.cpp`, `test_dxt_invariants.cpp`) plus edge cases of the loaders.

use m2_formats::image::dxt::{DxtcImage, PixFormat};
use m2_formats::image::tga::IMAGEDESC_TOPLEFT;
use m2_formats::image::{Image, TgaImage, decode_image, get_image_info};

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*s)
}

fn put32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// `makeDds` of test_dxt_invariants.cpp.
fn make_dds(four_cc: u32, width: u32, height: u32, mips: u32, linear: u32, payload: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; 128 + payload.len()];
    put32(&mut buf, 0, fourcc(b"DDS "));
    let mut flags = 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000;
    if mips > 1 {
        flags |= 0x20000;
    }
    put32(&mut buf, 4, 124);
    put32(&mut buf, 8, flags);
    put32(&mut buf, 12, height);
    put32(&mut buf, 16, width);
    put32(&mut buf, 20, linear);
    put32(&mut buf, 28, mips);
    put32(&mut buf, 76, 32);
    put32(&mut buf, 80, 4);
    put32(&mut buf, 84, four_cc);
    buf[128..].copy_from_slice(payload);
    buf
}

fn color_block(c0: u16, c1: u16, rows: [u8; 4]) -> Vec<u8> {
    let mut b = c0.to_le_bytes().to_vec();
    b.extend_from_slice(&c1.to_le_bytes());
    b.extend_from_slice(&rows);
    b
}

fn alpha_block5(a0: u8, a1: u8, indices: [u8; 16]) -> Vec<u8> {
    let mut bits = 0u64;
    for (i, &x) in indices.iter().enumerate() {
        bits |= u64::from(x & 7) << (3 * i);
    }
    let mut b = vec![a0, a1];
    b.extend_from_slice(&bits.to_le_bytes()[..6]);
    b
}

fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
}

/// `decompress` of test_dxt_invariants.cpp: payload is everything after the header.
fn decompress(dds: &[u8], level: u32) -> Vec<u32> {
    let img = DxtcImage::load_header(dds).unwrap();
    let start = img.level_offsets[level as usize].unwrap();
    let need = img.level_data_size(level).unwrap();
    img.decompress(level, &dds[start..start + need]).unwrap()
}

// ---- test_image.cpp ----------------------------------------------------------

#[test]
fn image_surface_operations() {
    let mut img = Image::default();
    assert!(img.is_empty());
    img = Image::new(8, 8);
    assert!(!img.is_empty());
    assert_eq!((img.width(), img.height()), (8, 8));

    img.clear(0xFF11_2233);
    assert_eq!(img.pixels()[0], 0xFF11_2233);

    let mut sub = Image::new(2, 2);
    sub.clear(0xFFAA_BBCC);
    img.put_image(0, 0, &sub);
    assert_eq!(img.line(0)[0], 0xFFAA_BBCC);
    assert_eq!(img.line(7)[0], 0xFF11_2233);

    img.flip_top_to_bottom();
    assert_eq!(img.line(7)[0], 0xFFAA_BBCC);
    assert_eq!(img.line(0)[0], 0xFF11_2233);
}

#[test]
fn empty_surface_of_zero_size_is_not_empty() {
    assert!(!Image::new(0, 0).is_empty());
}

#[test]
fn tga_rle_round_trip() {
    let mut out = TgaImage::create(4, 4);
    out.set_alpha_channel(true);
    out.set_compressed(true);
    out.header_mut().desc |= IMAGEDESC_TOPLEFT;
    let px = out.image_mut().pixels_mut();
    px[..8].fill(0xFFFF_0000);
    for (i, p) in px.iter_mut().enumerate().skip(8) {
        *p = 0xFF00_FF00 + i as u32;
    }
    let bytes = out.save_to_bytes();
    // run of 8 reds: one repeat packet (0x87) then one raw packet of 8
    assert_eq!(bytes[18], 0x87);
    assert_eq!(bytes[23], 7);

    let mut back = TgaImage::new();
    assert!(back.load_from_memory(&bytes));
    assert_eq!((back.image().width(), back.image().height()), (4, 4));
    assert_eq!(back.image().pixels(), out.image().pixels());
}

#[test]
fn tga_uncompressed_round_trip_and_flip() {
    let mut out = TgaImage::create(3, 2);
    out.image_mut().pixels_mut().copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    let bytes = out.save_to_bytes();
    assert_eq!(bytes.len(), 18 + 24);
    // no top-left bit: the loader flips
    let back = TgaImage::decode(&bytes).unwrap();
    assert_eq!(back.image().pixels(), &[4, 5, 6, 1, 2, 3]);
    // `CTGAImage(CImage&)` flips on construction, so saving that and loading
    // gives the original back
    let img = out.image().clone();
    let again = TgaImage::decode(&TgaImage::from_image(&img).save_to_bytes()).unwrap();
    assert_eq!(again.image().pixels(), img.pixels());
}

#[test]
fn tga_loader_quirks() {
    let header = |ty: u8, bits: u8, desc: u8, w: u16, h: u16| {
        let mut v = vec![0, 0, ty, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&[bits, desc]);
        v
    };
    // 24 bit raw: file order b, g, r is read as r, g, b (red and blue swapped)
    let mut f = header(2, 24, 0x20, 1, 1);
    f.extend_from_slice(&[0x11, 0x22, 0x33]);
    assert_eq!(TgaImage::decode(&f).unwrap().image().pixels(), &[0xFF11_2233]);
    // RLE 24 bit reads b, g, r properly
    let mut f = header(10, 24, 0x20, 2, 1);
    f.extend_from_slice(&[0x81, 0x11, 0x22, 0x33]);
    assert_eq!(TgaImage::decode(&f).unwrap().image().pixels(), &[0xFF33_2211; 2]);
    // 16 bit: 5 bit channels shifted into the top of each byte
    let mut f = header(2, 16, 0x20, 1, 1);
    f.extend_from_slice(&0x7FFFu16.to_le_bytes());
    assert_eq!(TgaImage::decode(&f).unwrap().image().pixels(), &[0xFFF8_F8F8]);
    // alpha only
    let mut f = header(3, 8, 0x20, 1, 1);
    f.push(0x7F);
    assert_eq!(TgaImage::decode(&f).unwrap().image().pixels(), &[0x7F7F_7F7F]);
    // an RLE packet running past the last pixel is rejected
    let mut f = header(10, 24, 0, 2, 1);
    f.extend_from_slice(&[0x84, 1, 2, 3]);
    assert!(TgaImage::decode(&f).is_none());
    // too short for the header, or for the pixels (undefined behaviour in C++)
    assert!(TgaImage::decode(&[0; 17]).is_none());
    assert!(TgaImage::decode(&header(2, 32, 0, 4, 4)).is_none());
    // unsupported combination loads with blank pixels
    let img = TgaImage::decode(&header(1, 8, 0x20, 2, 2)).unwrap();
    assert_eq!(img.image().pixels(), &[0; 4]);
}

#[test]
fn dxt1_block_decompression_64_bit_header() {
    // BuildMinimalDXT1DDS: one pure red block, every pixel selects colour 0
    let block = color_block(0xF800, 0x001F, [0; 4]);
    let dds = make_dds(fourcc(b"DXT1"), 4, 4, 1, 8, &block);
    let img = DxtcImage::load_header(&dds).unwrap();
    assert_eq!(img.comp_format, PixFormat::Dxt1);
    assert_eq!((img.width, img.height), (4, 4));
    assert_eq!(decompress(&dds, 0), vec![0xFFF8_0000; 16]);
}

// ---- test_dxt_invariants.cpp --------------------------------------------------

#[test]
fn dxt1_four_colour_block() {
    let block = color_block(0xF800, 0x001F, [0xE4, 0x1B, 0xE4, 0x1B]);
    let px = decompress(&make_dds(fourcc(b"DXT1"), 4, 4, 1, 8, &block), 0);
    let (c0, c1) = (argb(0xFF, 0xF8, 0, 0), argb(0xFF, 0, 0, 0xF8));
    let (c2, c3) = (argb(0xFF, 165, 0, 82), argb(0xFF, 82, 0, 165));
    assert_eq!(px[..8], [c0, c1, c2, c3, c3, c2, c1, c0]);
}

#[test]
fn dxt1_three_colour_transparent_block() {
    let block = color_block(0x001F, 0xF800, [0xE4; 4]);
    let px = decompress(&make_dds(fourcc(b"DXT1"), 4, 4, 1, 8, &block), 0);
    assert_eq!(px[0], argb(0xFF, 0, 0, 0xF8));
    assert_eq!(px[1], argb(0xFF, 0xF8, 0, 0));
    assert_eq!(px[2], argb(0xFF, 124, 0, 124));
    assert_eq!(px[3], 0);
}

#[test]
fn dxt3_explicit_alpha() {
    let mut block: Vec<u8> = [0x3210u16, 0x7654, 0xBA98, 0xFEDC].iter().flat_map(|w| w.to_le_bytes()).collect();
    block.extend(color_block(0xF800, 0xF800, [0; 4]));
    let px = decompress(&make_dds(fourcc(b"DXT3"), 4, 4, 1, 16, &block), 0);
    for (i, &p) in px.iter().enumerate() {
        let n = i as u8;
        assert_eq!(p, argb(n << 4 | n, 0xF8, 0, 0), "pixel {i}");
    }
}

#[test]
fn dxt5_alpha_modes() {
    let mut idx = [0u8; 16];
    for (i, x) in idx.iter_mut().enumerate() {
        *x = (i % 8) as u8;
    }
    let colour = color_block(0xF800, 0xF800, [0; 4]);
    for (a0, a1, expected) in
        [(200, 100, [200, 100, 185, 171, 157, 142, 128, 114]), (100, 200, [100, 200, 120, 140, 160, 180, 0, 255])]
    {
        let mut block = alpha_block5(a0, a1, idx);
        block.extend(&colour);
        let px = decompress(&make_dds(fourcc(b"DXT5"), 4, 4, 1, 16, &block), 0);
        for (i, &p) in px.iter().enumerate() {
            assert_eq!(p, argb(expected[i % 8], 0xF8, 0, 0), "a0 {a0} pixel {i}");
        }
    }
}

#[test]
fn dxt1_block_grid_is_row_major() {
    let payload: Vec<u8> = (0..4).flat_map(|_| color_block(0xF800, 0xF800, [0; 4])).collect();
    let px = decompress(&make_dds(fourcc(b"DXT1"), 8, 8, 1, 32, &payload), 0);
    assert_eq!(px, vec![argb(0xFF, 0xF8, 0, 0); 64]);
    // blocks of different colours land in their own quadrant
    let blocks = [(0xF800u16, 0), (0x07E0, 1), (0x001F, 2), (0xFFFF, 3)];
    let payload: Vec<u8> = blocks.iter().flat_map(|&(c, _)| color_block(c, c, [0; 4])).collect();
    let px = decompress(&make_dds(fourcc(b"DXT1"), 8, 8, 1, 32, &payload), 0);
    assert_eq!(px[0], argb(0xFF, 0xF8, 0, 0));
    assert_eq!(px[4], argb(0xFF, 0, 0xFC, 0));
    assert_eq!(px[4 * 8], argb(0xFF, 0, 0, 0xF8));
    assert_eq!(px[4 * 8 + 4], argb(0xFF, 0xF8, 0xFC, 0xF8));
}

#[test]
fn mip_chain_offsets_and_copy() {
    let red: Vec<u8> = (0..4).flat_map(|_| color_block(0xF800, 0xF800, [0; 4])).collect();
    let blue = color_block(0x001F, 0x001F, [0; 4]);
    let payload = [red, blue.clone()].concat();
    let dds = make_dds(fourcc(b"DXT1"), 8, 8, 2, 32, &payload);
    let img = DxtcImage::load_header(&dds).unwrap();
    assert_eq!(img.mip_map_count, 2);
    assert!(img.mip_texture);
    assert_eq!(img.level_offsets[1], Some(128 + 32));
    assert_eq!(decompress(&dds, 1), vec![argb(0xFF, 0, 0, 0xF8); 16]);
    // Copy: pitch >> (2 * level) bytes from the level's start
    assert_eq!(img.copy(&dds, 1).unwrap(), &blue[..]);
    assert_eq!(img.copy(&dds, 0).unwrap().len(), 32);
}

#[test]
fn copy_of_a_mip_level_needs_the_mipmapcount_flag() {
    let dds = make_dds(fourcc(b"DXT1"), 8, 8, 1, 32, &[0; 40]);
    let img = DxtcImage::load_header(&dds).unwrap();
    assert!(img.copy(&dds, 1).is_none());
    assert!(img.copy(&dds, 0).is_some());
    // pointing outside the file is refused where C++ would read out of bounds
    assert!(img.copy(&dds[..140], 0).is_none());
}

#[test]
fn dds_header_validation() {
    assert!(DxtcImage::load_header(&[]).is_none());
    assert!(DxtcImage::load_header(&[0; 3]).is_none());

    let mut bad_magic = make_dds(fourcc(b"DXT1"), 4, 4, 1, 8, &[0; 8]);
    bad_magic[0] = b'X';
    assert!(DxtcImage::load_header(&bad_magic).is_none());

    let truncated = make_dds(fourcc(b"DXT1"), 4, 4, 1, 8, &[]);
    assert!(DxtcImage::load_header(&truncated[..4 + 123]).is_none());

    for cc in [b"DXT2", b"DXT4", b"ABCD"] {
        assert!(DxtcImage::load_header(&make_dds(fourcc(cc), 4, 4, 1, 8, &[0; 8])).is_none());
    }

    let valid = make_dds(fourcc(b"DXT5"), 4, 4, 1, 16, &[0; 16]);
    let img = DxtcImage::load_header(&valid).unwrap();
    assert_eq!(img.comp_format, PixFormat::Dxt5);
    assert_eq!(img.mip_map_count, 1);
    assert_eq!(img.level_offsets[0], Some(128));
    assert_eq!(img.format_name, "DXT5");
}

#[test]
fn uncompressed_dds_names_its_format_and_is_rejected() {
    let mut dds = make_dds(0, 4, 4, 1, 64, &[0; 64]);
    put32(&mut dds, 4 + 84, 32);
    put32(&mut dds, 4 + 88, 0x00FF_0000);
    assert!(DxtcImage::load_header(&dds).is_none());
    let (name, f) = m2_formats::image::dxt::decode_pixel_format(&m2_formats::image::dxt::DdsPixelFormat {
        r_bit_mask: 0xFF0000,
        g_bit_mask: 0xFF00,
        b_bit_mask: 0xFF,
        rgb_alpha_bit_mask: 0xFF00_0000,
        ..Default::default()
    });
    assert_eq!((name.as_str(), f), ("ARGB-8888", PixFormat::Argb));
}

#[test]
fn mip_count_is_clamped_and_small_levels_are_empty() {
    let dds = make_dds(fourcc(b"DXT1"), 4, 4, 20, 8, &[0; 8]);
    let img = DxtcImage::load_header(&dds).unwrap();
    assert_eq!(img.mip_map_count, 12);
    // level 1 of a 4x4 image has no complete block: nothing is decoded
    assert_eq!(img.level_data_size(1), Some(0));
    assert_eq!(img.decompress(1, &[]).unwrap(), vec![0; 4]);
    // too little data
    assert!(img.decompress(0, &[0; 7]).is_none());
}

// ---- Platform_GetImageInfo / Platform_DecodeImage ----------------------------

#[test]
fn image_info_of_dds() {
    let dds = make_dds(fourcc(b"DXT3"), 64, 32, 1, 16, &[0; 16]);
    let i = get_image_info(&dds).unwrap();
    assert_eq!((i.width, i.height, i.format), (64, 32, 0x3354_5844));
    let dds = make_dds(fourcc(b"DXT2"), 64, 32, 1, 16, &[0; 16]);
    assert_eq!(get_image_info(&dds).unwrap().format, 21);
    assert!(get_image_info(&[0; 17]).is_none());
}

#[test]
fn image_info_falls_back_to_the_raw_tga_header() {
    let mut f = vec![0u8; 20];
    f[12..14].copy_from_slice(&300u16.to_le_bytes());
    f[14..16].copy_from_slice(&200u16.to_le_bytes());
    let i = get_image_info(&f).unwrap();
    assert_eq!((i.width, i.height, i.format), (300, 200, 21));
    f[12..14].copy_from_slice(&5000u16.to_le_bytes());
    assert!(get_image_info(&f).is_none());
}

#[test]
fn decode_image_swaps_to_bgra() {
    // stb TGA: 2x1 32 bit, bottom-left origin, bytes b g r a
    let mut f = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 0, 32, 8];
    f.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let d = decode_image(&f).unwrap();
    assert_eq!((d.width, d.height), (2, 1));
    assert_eq!(d.bgra, [1, 2, 3, 4, 5, 6, 7, 8]);
    assert!(decode_image(&[1, 2, 3]).is_none());
    assert!(decode_image(&make_dds(fourcc(b"DXT1"), 4, 4, 1, 8, &[0; 8])).is_none());
}

#[test]
fn decode_png_and_bmp() {
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, 2, 1);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[10, 20, 30, 40, 50, 60]).unwrap();
        w.finish().unwrap();
    }
    let d = decode_image(&png).unwrap();
    assert_eq!(d.bgra, [30, 20, 10, 255, 60, 50, 40, 255]);
    assert_eq!(get_image_info(&png).unwrap().format, 20);

    // 1x2 24 bit BMP, bottom row first
    let mut bmp = b"BM".to_vec();
    bmp.extend_from_slice(&[0; 8]);
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&1i32.to_le_bytes());
    bmp.extend_from_slice(&2i32.to_le_bytes());
    bmp.extend_from_slice(&[1, 0, 24, 0]);
    bmp.extend_from_slice(&[0; 24]);
    bmp.extend_from_slice(&[1, 2, 3, 0, 4, 5, 6, 0]);
    let d = decode_image(&bmp).unwrap();
    assert_eq!(d.bgra, [4, 5, 6, 255, 1, 2, 3, 255]);
}
