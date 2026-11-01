//! The parts of stb_image (`neoeter/include/Platform/stb_image.h`) the client
//! links, as used by `ImageLoaderCompat.cpp`: `stbi_load_from_memory(…, 4)`
//! and `stbi_info_from_memory`, with `STBI_NO_STDIO`, `STBI_NO_LINEAR` and
//! `STBI_NO_HDR`.
//!
//! Ported: JPEG (baseline and progressive, the SSE2 build's results), TGA, BMP
//! and PNG. GIF, PSD, PIC and PNM are recognised by their magic numbers, so a
//! file in one of those formats is not mistaken for a JPEG or TGA, but they are
//! not decoded (deviation: the client never ships them).
//!
//! Reading past the end of the data yields zeros, as `stbi__get8` does for a
//! memory buffer.

mod bmp;
mod jpeg;
mod png;
mod tga;

/// `stbi__context` over a memory buffer.
pub(crate) struct Ctx<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Ctx<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Ctx { data, pos: 0 }
    }

    /// `stbi__get8`.
    pub fn get8(&mut self) -> u8 {
        match self.data.get(self.pos) {
            Some(&b) => {
                self.pos += 1;
                b
            }
            None => 0,
        }
    }

    /// `stbi__at_eof`.
    pub fn at_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// `stbi__skip`: a negative count jumps to the end.
    pub fn skip(&mut self, n: i32) {
        if n == 0 {
            return;
        }
        if n < 0 {
            self.pos = self.data.len();
            return;
        }
        self.pos = self.pos.saturating_add(n as usize);
    }

    /// `stbi__getn`: all or nothing.
    pub fn getn(&mut self, out: &mut [u8]) -> bool {
        match self.pos.checked_add(out.len()) {
            Some(end) if end <= self.data.len() => {
                out.copy_from_slice(&self.data[self.pos..end]);
                self.pos = end;
                true
            }
            _ => false,
        }
    }

    pub fn get16be(&mut self) -> i32 {
        let z = i32::from(self.get8());
        (z << 8) + i32::from(self.get8())
    }

    pub fn get16le(&mut self) -> i32 {
        let z = i32::from(self.get8());
        z + (i32::from(self.get8()) << 8)
    }

    pub fn get32le(&mut self) -> u32 {
        let z = self.get16le() as u32;
        z.wrapping_add((self.get16le() as u32) << 16)
    }
}

/// `STBI_MAX_DIMENSIONS`.
pub(crate) const MAX_DIMENSIONS: u32 = 1 << 24;

/// `stbi__mad3sizes_valid(a, b, c, 0)` on `int`s.
pub(crate) fn mad3sizes_valid(a: i32, b: i32, c: i32) -> bool {
    let mul = |a: i32, b: i32| -> Option<i32> {
        if a < 0 || b < 0 {
            return None;
        }
        a.checked_mul(b)
    };
    mul(a, b).and_then(|ab| mul(ab, c)).is_some()
}

/// `stbi__convert_format` to 4 components (`req_comp == 4`) from `img_n`.
pub(crate) fn convert_to_rgba(data: &[u8], img_n: usize) -> Vec<u8> {
    match img_n {
        4 => data.to_vec(),
        1 => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        2 => data.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        3 => data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        _ => unreachable!("img_n {img_n}"),
    }
}

/// What `stbi__load_main` / `stbi__info_main` try, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Bmp,
    Gif,
    Psd,
    Pic,
    Jpeg,
    Pnm,
    Tga,
}

/// `stbi__load_main`'s sequence of `*_test` calls: the first format whose
/// test passes. `None` is `Image not of any known type`.
pub fn detect(data: &[u8]) -> Option<Format> {
    let starts = |magic: &[u8]| data.starts_with(magic);
    if starts(b"\x89PNG\r\n\x1a\n") {
        return Some(Format::Png);
    }
    if bmp::test(data) {
        return Some(Format::Bmp);
    }
    // stbi__gif_test_raw: "GIF8" then '9' or '7' then 'a'.
    if starts(b"GIF8") && matches!(data.get(4), Some(b'9' | b'7')) && data.get(5) == Some(&b'a') {
        return Some(Format::Gif);
    }
    if starts(b"8BPS") {
        return Some(Format::Psd);
    }
    // stbi__pic_test_core: magic, 84 bytes, "PICT".
    if starts(b"\x53\x80\xF6\x34") && data.get(88..92) == Some(b"PICT") {
        return Some(Format::Pic);
    }
    if jpeg::test(data) {
        return Some(Format::Jpeg);
    }
    if starts(b"P5") || starts(b"P6") {
        return Some(Format::Pnm);
    }
    if tga::test(data) {
        return Some(Format::Tga);
    }
    None
}

/// Result of `stbi_load_from_memory(…, req_comp = 4)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    /// `comp`: components of the file (what `stbi_info` would report).
    pub components: u32,
    /// `width * height * 4` bytes, R G B A.
    pub pixels: Vec<u8>,
}

/// `stbi_load_from_memory(data, len, &w, &h, &comp, 4)`; `None` where it
/// returns NULL (including formats this port does not decode, see the module
/// docs).
pub fn load_rgba(data: &[u8]) -> Option<Rgba> {
    match detect(data)? {
        Format::Png => png::load(data),
        Format::Bmp => bmp::load(data),
        Format::Jpeg => jpeg::load(data),
        Format::Tga => tga::load(data),
        Format::Gif | Format::Psd | Format::Pic | Format::Pnm => None,
    }
}

/// `stbi_info_from_memory`: width, height and component count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub components: u32,
}

/// `stbi__info_main`. Unlike [`load_rgba`] it tries JPEG first, then PNG, GIF,
/// BMP, PSD, PIC, PNM and TGA, each with its own (weaker) header test. GIF, PSD,
/// PIC and PNM headers are not parsed here.
pub fn info(data: &[u8]) -> Option<Info> {
    if let Some(i) = jpeg::info(data).or_else(|| png::info(data)).or_else(|| bmp::info(data)) {
        return Some(i);
    }
    if matches!(detect(data), Some(Format::Gif | Format::Psd | Format::Pic | Format::Pnm)) {
        return None; // their info routines would succeed; not ported
    }
    tga::info(data)
}
