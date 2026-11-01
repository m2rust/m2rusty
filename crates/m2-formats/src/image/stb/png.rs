//! PNG as `stbi_load_from_memory(…, 4)` returns it: 8 bit RGBA, with palettes,
//! low bit depths and `tRNS` expanded, 16 bit channels cut to their high byte
//! and grey replicated. Decoding is done by the `png` crate (inflate and
//! filtering are lossless, so the pixels cannot differ); the conversions above
//! are stb's. `stbi_info` is a walk over the chunks in front of the first
//! `IDAT`, as `stbi__parse_png_file(…, STBI__SCAN_header)` does.
//!
//! The client ships no PNG files; this is covered by synthetic tests only.

use super::{Info, MAX_DIMENSIONS, Rgba, convert_to_rgba};

/// `stbi__png_info`.
pub(super) fn info(data: &[u8]) -> Option<Info> {
    if !data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let be32 = |at: usize| -> Option<u32> { Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?)) };

    let mut pos = 8;
    let mut first = true;
    let (mut width, mut height) = (0, 0);
    let mut pal_img_n = 0;
    let mut img_n = 0;
    loop {
        let length = be32(pos)?;
        let ty = data.get(pos + 4..pos + 8)?;
        let body = pos + 8;
        match ty {
            b"IHDR" => {
                if !first || length != 13 {
                    return None;
                }
                first = false;
                width = be32(body)?;
                height = be32(body + 4)?;
                if height > MAX_DIMENSIONS || width > MAX_DIMENSIONS {
                    return None;
                }
                let depth = *data.get(body + 8)?;
                let color = *data.get(body + 9)?;
                if !matches!(depth, 1 | 2 | 4 | 8 | 16) || color > 6 || (color == 3 && depth == 16) {
                    return None;
                }
                if color == 3 {
                    pal_img_n = 3;
                } else if color & 1 != 0 {
                    return None;
                }
                if *data.get(body + 10)? != 0 || *data.get(body + 11)? != 0 || *data.get(body + 12)? > 1 {
                    return None;
                }
                if width == 0 || height == 0 {
                    return None;
                }
                if pal_img_n == 0 {
                    img_n = (if color & 2 != 0 { 3 } else { 1 }) + (if color & 4 != 0 { 1 } else { 0 });
                    if (1u32 << 30) / width / img_n < height {
                        return None;
                    }
                } else {
                    img_n = 1;
                    if (1u32 << 30) / width / 4 < height {
                        return None;
                    }
                }
            }
            b"PLTE" => {
                if first || length > 256 * 3 || length % 3 != 0 {
                    return None;
                }
            }
            b"tRNS" => {
                if first {
                    return None;
                }
                if pal_img_n != 0 {
                    img_n = 4;
                    break;
                }
                if img_n & 1 == 0 || length != img_n * 2 {
                    return None; // tRNS with alpha / bad length
                }
                img_n += 1;
                break;
            }
            b"IDAT" => {
                if first {
                    return None;
                }
                if pal_img_n != 0 {
                    img_n = pal_img_n;
                }
                break;
            }
            b"IEND" => {
                if first {
                    return None;
                }
                break;
            }
            _ => {
                // critical unknown chunks fail
                if first || ty[0] & 0x20 == 0 {
                    return None;
                }
            }
        }
        pos = body.checked_add(length as usize)?.checked_add(4)?; // data + CRC
    }
    Some(Info { width, height, components: img_n })
}

/// `stbi__png_load` with `req_comp == 4`.
pub(super) fn load(data: &[u8]) -> Option<Rgba> {
    let components = info(data)?.components;

    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    buf.truncate(frame.buffer_size());

    let n = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return None,
    };
    if frame.bit_depth != png::BitDepth::Eight {
        return None;
    }
    Some(Rgba { width: frame.width, height: frame.height, components, pixels: convert_to_rgba(&buf, n) })
}
