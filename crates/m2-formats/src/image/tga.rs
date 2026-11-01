//! `CTGAImage` (`neoeter/src/image/TGAImage.cpp`): the client's own TGA
//! reader/writer, used by `CTargaResource` (`render/TargaResource.cpp`).
//!
//! It is much narrower than stb's TGA loader ([`super::stb`]) and has quirks
//! that are reproduced as they are:
//!
//! * uncompressed 24 bit reads the pixel bytes as `r, g, b` in file order
//!   (the file holds `b, g, r`), so red and blue come out swapped; RLE 24 bit
//!   reads `b, g, r` and is right;
//! * only types 2 (raw), 3 (alpha only) and 10 (RLE) are decoded, and only
//!   16/24/32 bits for 2 and 24/32 for 10. Anything else "loads" with
//!   uninitialised pixels in C++ (zeros here);
//! * the origin bit `0x20` of the descriptor decides the flip, nothing else.
//!
//! The C++ code never checks how much input is left. Where it would read past
//! the end, [`TgaImage::load_from_memory`] returns `false` instead (deliberate
//! deviation from undefined behaviour); an RLE packet running past the pixel
//! count returns `false` in both.

use super::{Image, alloc_pixels};

/// `IMAGEDESC_TOPLEFT`.
pub const IMAGEDESC_TOPLEFT: u8 = 0x20;

/// `TGA_HEADER`, 18 bytes, little endian.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TgaHeader {
    pub id_len: u8,
    pub pal_type: u8,
    pub img_type: u8,
    pub color_begin: u16,
    pub color_count: u16,
    pub pal_entry_size: u8,
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16,
    pub color_bits: u8,
    pub desc: u8,
}

impl TgaHeader {
    pub const SIZE: usize = 18;

    /// The `memcpy(&m_Header, c_pbMem, 18)` of `LoadFromMemory`.
    pub fn parse(b: &[u8]) -> Option<Self> {
        let b = b.get(..Self::SIZE)?;
        let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        Some(TgaHeader {
            id_len: b[0],
            pal_type: b[1],
            img_type: b[2],
            color_begin: u16_at(3),
            color_count: u16_at(5),
            pal_entry_size: b[7],
            left: u16_at(8),
            top: u16_at(10),
            width: u16_at(12),
            height: u16_at(14),
            color_bits: b[16],
            desc: b[17],
        })
    }

    /// The `fwrite(&m_Header, 18, 1, fp)` of `SaveToDiskFile`.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut o = [0u8; Self::SIZE];
        o[0] = self.id_len;
        o[1] = self.pal_type;
        o[2] = self.img_type;
        o[3..5].copy_from_slice(&self.color_begin.to_le_bytes());
        o[5..7].copy_from_slice(&self.color_count.to_le_bytes());
        o[7] = self.pal_entry_size;
        o[8..10].copy_from_slice(&self.left.to_le_bytes());
        o[10..12].copy_from_slice(&self.top.to_le_bytes());
        o[12..14].copy_from_slice(&self.width.to_le_bytes());
        o[14..16].copy_from_slice(&self.height.to_le_bytes());
        o[16] = self.color_bits;
        o[17] = self.desc;
        o
    }
}

/// `CTGAImage`: an [`Image`] with its TGA header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TgaImage {
    header: TgaHeader,
    image: Image,
}

/// Cursor over the pixel data, failing (instead of reading garbage) at the end.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.data.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(s)
    }
}

fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
}

impl TgaImage {
    /// `CTGAImage::CTGAImage()`: no surface yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// `CTGAImage::Create`: a 32 bit uncompressed header and a blank surface.
    pub fn create(width: i32, height: i32) -> Self {
        let header = TgaHeader {
            img_type: 2,
            width: width as u16,
            height: height as u16,
            color_bits: 32,
            desc: 0x08, // alpha channel present
            ..TgaHeader::default()
        };
        TgaImage { header, image: Image::new(width, height) }
    }

    /// `CTGAImage::CTGAImage(CImage &)`: copies `image` and flips it, so the
    /// bottom-left origin the header describes is right.
    pub fn from_image(image: &Image) -> Self {
        let mut t = Self::create(image.width(), image.height());
        t.image.pixels_mut().copy_from_slice(image.pixels());
        t.image.flip_top_to_bottom();
        t
    }

    /// `CTGAImage::GetHeader`.
    pub fn header(&self) -> &TgaHeader {
        &self.header
    }

    /// `CTGAImage::GetHeader`, mutable.
    pub fn header_mut(&mut self) -> &mut TgaHeader {
        &mut self.header
    }

    /// The surface (`CImage` base).
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// The surface (`CImage` base), mutable.
    pub fn image_mut(&mut self) -> &mut Image {
        &mut self.image
    }

    /// `CTGAImage::SetCompressed`.
    pub fn set_compressed(&mut self, compress: bool) {
        self.header.img_type = if compress { 10 } else { 2 };
    }

    /// `CTGAImage::SetAlphaChannel`.
    pub fn set_alpha_channel(&mut self, exists: bool) {
        if exists {
            self.header.desc |= 0x08;
        } else {
            self.header.desc &= !0x08;
        }
    }

    /// `CTGAImage::LoadFromMemory`. On success the previous contents are
    /// replaced; on failure `self` is left as it was.
    pub fn load_from_memory(&mut self, data: &[u8]) -> bool {
        match Self::decode(data) {
            Some(loaded) => {
                *self = loaded;
                true
            }
            None => false,
        }
    }

    /// `LoadFromMemory` as a constructor.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let header = TgaHeader::parse(data)?;
        let hxw = usize::from(header.width) * usize::from(header.height);
        let mut r = Reader { data, pos: TgaHeader::SIZE };

        let pixels = match header.img_type {
            // Alpha only, one byte per pixel, replicated into all channels.
            3 => r.take(hxw)?.iter().map(|&a| u32::from_ne_bytes([a; 4])).collect(),
            2 => match header.color_bits {
                16 => r
                    .take(hxw * 2)?
                    .chunks_exact(2)
                    .map(|c| {
                        let w = u16::from_le_bytes([c[0], c[1]]);
                        // The C++ shifts a BYTE: 5 bit values land in the top 5 bits.
                        let b = ((w & 0x1f) as u8) << 3;
                        let g = (((w >> 5) & 0x1f) as u8) << 3;
                        let r = (((w >> 10) & 0x1f) as u8) << 3;
                        argb(0xff, r, g, b)
                    })
                    .collect(),
                // Bug kept: the file holds b, g, r but they are read as r, g, b.
                24 => r.take(hxw * 3)?.chunks_exact(3).map(|c| argb(0xff, c[0], c[1], c[2])).collect(),
                32 => r.take(hxw * 4)?.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(),
                _ => alloc_pixels(hxw)?, // C++: uninitialised
            },
            10 => match header.color_bits {
                24 => Self::decode_rle(&mut r, hxw, 3)?,
                32 => Self::decode_rle(&mut r, hxw, 4)?,
                _ => alloc_pixels(hxw)?, // C++: uninitialised
            },
            _ => alloc_pixels(hxw)?, // C++: uninitialised
        };

        let mut image = Image::from_pixels(i32::from(header.width), i32::from(header.height), pixels);
        if header.desc & IMAGEDESC_TOPLEFT == 0 {
            image.flip_top_to_bottom();
        }
        Some(TgaImage { header, image })
    }

    /// The RLE loops for 24 and 32 bit (`bytes` per pixel). A packet that would
    /// write past `hxw` pixels fails, like the `RLE overflow` return.
    fn decode_rle(r: &mut Reader, hxw: usize, bytes: usize) -> Option<Vec<u32>> {
        let read_pixel = |r: &mut Reader| -> Option<u32> {
            let p = r.take(bytes)?;
            let a = if bytes == 4 { p[3] } else { 0xff };
            Some(argb(a, p[2], p[1], p[0]))
        };
        let mut out = Vec::new();
        while out.len() < hxw {
            let rle = r.byte()?;
            if rle < 0x80 {
                for _ in 0..=usize::from(rle) {
                    let px = read_pixel(r)?;
                    if out.len() >= hxw {
                        return None;
                    }
                    out.push(px);
                }
            } else {
                let px = read_pixel(r)?;
                for _ in 0..rle - 127 {
                    if out.len() >= hxw {
                        return None;
                    }
                    out.push(px);
                }
            }
        }
        Some(out)
    }

    /// Number of pixels equal to `data[at]` in a row, at most 127
    /// (`GetRLEPixelCount`).
    fn rle_pixel_count(data: &[u32], at: usize) -> usize {
        if at >= data.len() {
            return 0;
        }
        let pixel = data[at];
        let mut r = 1;
        let mut p = at;
        while r < 127 && p < data.len() - 1 {
            p += 1;
            if pixel != data[p] {
                return r;
            }
            r += 1;
        }
        r
    }

    /// Length of the run of pixels that should be stored raw (`GetRawPixelCount`).
    fn raw_pixel_count(data: &[u32], at: usize) -> usize {
        if at >= data.len() {
            return 0;
        }
        let mut i = 0;
        let mut p = at;
        while p < data.len() && i < 127 {
            if Self::rle_pixel_count(data, p) >= 4 {
                break;
            }
            p += 1;
            i += 1;
        }
        i
    }

    /// `CTGAImage::SaveToDiskFile`, to memory: the header followed by the pixels,
    /// RLE packed when the header says type 10. The rows are written as stored
    /// (no flip), `u32` little endian.
    pub fn save_to_bytes(&self) -> Vec<u8> {
        let mut out = self.header.to_bytes().to_vec();
        let data = self.image.pixels();
        if self.header.img_type == 10 {
            let mut at = 0;
            while at < data.len() {
                let rle = Self::rle_pixel_count(data, at);
                if rle < 4 {
                    let mut raw = Self::raw_pixel_count(data, at);
                    if raw == 0 {
                        break;
                    }
                    out.push((raw - 1) as u8);
                    while raw > 0 {
                        out.extend_from_slice(&data[at].to_le_bytes());
                        at += 1;
                        raw -= 1;
                    }
                } else {
                    out.push(((rle - 1) | 0x80) as u8);
                    out.extend_from_slice(&data[at].to_le_bytes());
                    at += rle;
                }
            }
        } else {
            for px in data {
                out.extend_from_slice(&px.to_le_bytes());
            }
        }
        out
    }
}
