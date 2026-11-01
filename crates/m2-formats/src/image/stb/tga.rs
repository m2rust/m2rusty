//! stb_image's TGA loader (`stbi__tga_*`): paletted, grey, 15/16/24/32 bit,
//! raw or RLE. This is what `Platform_DecodeImage` uses for `.tga` files; the
//! client's own reader is [`crate::image::tga`].

use super::{Ctx, Info, MAX_DIMENSIONS, Rgba, convert_to_rgba, mad3sizes_valid};

/// `stbi__tga_get_comp`: `(components, is_rgb16)`, components 0 on error.
fn get_comp(bits_per_pixel: i32, is_grey: bool) -> (i32, bool) {
    match bits_per_pixel {
        8 => (1, false),
        16 if is_grey => (2, false),
        15 | 16 => (3, true),
        24 | 32 => (bits_per_pixel / 8, false),
        _ => (0, false),
    }
}

/// `stbi__tga_test`.
pub(super) fn test(data: &[u8]) -> bool {
    let mut s = Ctx::new(data);
    s.get8(); // offset
    let tga_color_type = s.get8();
    if tga_color_type > 1 {
        return false; // only RGB or indexed allowed
    }
    let mut sz = s.get8(); // image type
    if tga_color_type == 1 {
        // colormapped (paletted) image
        if sz != 1 && sz != 9 {
            return false;
        }
        s.skip(4); // index of first colormap entry and number of entries
        sz = s.get8(); // bits per palette color entry
        if !matches!(sz, 8 | 15 | 16 | 24 | 32) {
            return false;
        }
        s.skip(4); // image x and y origin
    } else {
        // "normal" image w/o colormap
        if !matches!(sz, 2 | 3 | 10 | 11) {
            return false; // only RGB or grey allowed, +/- RLE
        }
        s.skip(9); // colormap specification and image x/y origin
    }
    if s.get16le() < 1 {
        return false; // width
    }
    if s.get16le() < 1 {
        return false; // height
    }
    sz = s.get8(); // bits per pixel
    if tga_color_type == 1 && sz != 8 && sz != 16 {
        return false; // for colormapped images, bpp is size of an index
    }
    matches!(sz, 8 | 15 | 16 | 24 | 32)
}

/// `stbi__tga_info`.
pub(super) fn info(data: &[u8]) -> Option<Info> {
    let mut s = Ctx::new(data);
    s.get8(); // offset
    let colormap_type = s.get8();
    if colormap_type > 1 {
        return None;
    }
    let image_type = s.get8();
    let colormap_bpp;
    if colormap_type == 1 {
        if image_type != 1 && image_type != 9 {
            return None;
        }
        s.skip(4);
        let sz = s.get8();
        if !matches!(sz, 8 | 15 | 16 | 24 | 32) {
            return None;
        }
        s.skip(4);
        colormap_bpp = i32::from(sz);
    } else {
        if !matches!(image_type, 2 | 3 | 10 | 11) {
            return None;
        }
        s.skip(9);
        colormap_bpp = 0;
    }
    let w = s.get16le();
    if w < 1 {
        return None;
    }
    let h = s.get16le();
    if h < 1 {
        return None;
    }
    let bits_per_pixel = i32::from(s.get8());
    s.get8(); // alpha bits
    let comp;
    if colormap_bpp != 0 {
        if bits_per_pixel != 8 && bits_per_pixel != 16 {
            return None;
        }
        comp = get_comp(colormap_bpp, false).0;
    } else {
        comp = get_comp(bits_per_pixel, image_type == 3 || image_type == 11).0;
    }
    if comp == 0 {
        return None;
    }
    Some(Info { width: w as u32, height: h as u32, components: comp as u32 })
}

/// `stbi__tga_read_rgb16`: a 15/16 bit pixel as three bytes.
fn read_rgb16(s: &mut Ctx) -> [u8; 3] {
    let px = s.get16le() as u16;
    let r = u32::from((px >> 10) & 31);
    let g = u32::from((px >> 5) & 31);
    let b = u32::from(px & 31);
    [(r * 255 / 31) as u8, (g * 255 / 31) as u8, (b * 255 / 31) as u8]
}

/// `stbi__tga_load` with `req_comp == 4`.
pub(super) fn load(data: &[u8]) -> Option<Rgba> {
    let mut s = Ctx::new(data);
    let tga_offset = i32::from(s.get8());
    let tga_indexed = s.get8() != 0;
    let mut image_type = i32::from(s.get8());
    let palette_start = s.get16le();
    let palette_len = s.get16le() as usize;
    let palette_bits = i32::from(s.get8());
    s.get16le(); // x origin
    s.get16le(); // y origin
    let width = s.get16le();
    let height = s.get16le();
    let bits_per_pixel = i32::from(s.get8());
    let mut inverted = i32::from(s.get8());

    if height as u32 > MAX_DIMENSIONS || width as u32 > MAX_DIMENSIONS {
        return None;
    }

    let mut is_rle = false;
    if image_type >= 8 {
        image_type -= 8;
        is_rle = true;
    }
    inverted = 1 - ((inverted >> 5) & 1);
    let inverted = inverted != 0;

    // If I'm paletted, then I'll use the number of bits from the palette
    let (comp, rgb16) =
        if tga_indexed { get_comp(palette_bits, false) } else { get_comp(bits_per_pixel, image_type == 3) };
    if comp == 0 {
        return None; // bad format
    }
    if !mad3sizes_valid(width, height, comp) {
        return None; // too large
    }
    let (w, h, comp) = (width as usize, height as usize, comp as usize);
    let mut tga_data = Vec::new();
    tga_data.try_reserve_exact(w * h * comp).ok()?;
    tga_data.resize(w * h * comp, 0u8); // C: uninitialised

    // skip to the data's starting position (offset usually = 0)
    s.skip(tga_offset);

    if !tga_indexed && !is_rle && !rgb16 {
        for i in 0..h {
            let row = if inverted { h - i - 1 } else { i };
            // A short read leaves the row untouched (C: uninitialised).
            s.getn(&mut tga_data[row * w * comp..(row + 1) * w * comp]);
        }
    } else {
        // do I need to load a palette?
        let mut palette = Vec::new();
        if tga_indexed {
            if palette_len == 0 {
                return None; // you have to have at least one entry!
            }
            // any data to skip? (offset usually = 0)
            s.skip(palette_start);
            palette.resize(palette_len * comp, 0u8);
            if rgb16 {
                for entry in palette.chunks_exact_mut(comp) {
                    entry.copy_from_slice(&read_rgb16(&mut s));
                }
            } else if !s.getn(&mut palette) {
                return None; // bad palette
            }
        }
        // load the data
        let mut raw_data = [0u8; 4];
        let mut rle_count = 0i32;
        let mut rle_repeating = false;
        let mut read_next_pixel = true;
        for i in 0..w * h {
            // if I'm in RLE mode, do I need to get a RLE packet?
            if is_rle {
                if rle_count == 0 {
                    // yep, get the next byte as a RLE command
                    let cmd = i32::from(s.get8());
                    rle_count = 1 + (cmd & 127);
                    rle_repeating = cmd >> 7 != 0;
                    read_next_pixel = true;
                } else if !rle_repeating {
                    read_next_pixel = true;
                }
            } else {
                read_next_pixel = true;
            }
            // OK, if I need to read a pixel, do it now
            if read_next_pixel {
                if tga_indexed {
                    // read in index, then perform the lookup
                    let mut pal_idx = if bits_per_pixel == 8 { i32::from(s.get8()) } else { s.get16le() };
                    if pal_idx as usize >= palette_len {
                        pal_idx = 0; // invalid index
                    }
                    let at = pal_idx as usize * comp;
                    raw_data[..comp].copy_from_slice(&palette[at..at + comp]);
                } else if rgb16 {
                    raw_data[..3].copy_from_slice(&read_rgb16(&mut s));
                } else {
                    // read in the data raw
                    for b in &mut raw_data[..comp] {
                        *b = s.get8();
                    }
                }
                // clear the reading flag for the next pixel
                read_next_pixel = false;
            }
            tga_data[i * comp..(i + 1) * comp].copy_from_slice(&raw_data[..comp]);
            // in case we're in RLE mode, keep counting down
            rle_count -= 1;
        }
        // do I need to invert the image?
        if inverted {
            let row_len = w * comp;
            for j in 0..h / 2 {
                let (top, bottom) = tga_data.split_at_mut((h - 1 - j) * row_len);
                top[j * row_len..(j + 1) * row_len].swap_with_slice(&mut bottom[..row_len]);
            }
        }
    }

    // swap RGB - if the source data was RGB16, it already is in the right order
    if comp >= 3 && !rgb16 {
        for px in tga_data.chunks_exact_mut(comp) {
            px.swap(0, 2);
        }
    }

    Some(Rgba {
        width: width as u32,
        height: height as u32,
        components: comp as u32,
        pixels: convert_to_rgba(&tga_data, comp),
    })
}
