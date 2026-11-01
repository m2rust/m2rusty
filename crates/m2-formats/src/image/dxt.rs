//! `CDXTCImage` (`neoeter/src/image/DXTCImage.cpp`): DDS header parsing and
//! DXT1/3/5 decompression.
//!
//! The client uploads the compressed blocks to the GPU as they are
//! ([`DxtcImage::copy`]); [`DxtcImage::decompress`] is the CPU decoder the C++
//! tests use, ported bit for bit (RGB565 expanded by `<< 3` / `<< 2` without
//! replicating the high bits, 1/3 and 2/3 interpolation by integer division,
//! DXT3 alpha by nibble replication, DXT5 alpha by the 8/6 alpha rule).
//!
//! The C++ class keeps pointers into the caller's buffer; here they are byte
//! offsets from the start of the file ([`DxtcImage::level_offsets`]).
//!
//! Deviations from C++, all on malformed input where C++ reads out of bounds:
//! [`DxtcImage::copy`] and [`DxtcImage::decompress`] return `None` when the
//! data they were handed is too short.

/// `MAX_MIPLEVELS`.
pub const MAX_MIPLEVELS: usize = 12;

const DDSD_PITCH: u32 = 0x0000_0008;
const DDSD_MIPMAPCOUNT: u32 = 0x0002_0000;
const DDPF_ALPHAPREMULT: u32 = 0x0000_8000;

/// Size of `DDSURFACEDESC2` on disk.
const DDSURFACEDESC2_SIZE: usize = 124;

const fn fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    u32::from_le_bytes([a, b, c, d])
}

/// `EPixFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixFormat {
    Argb,
    Dxt1,
    Dxt2,
    Dxt3,
    Dxt4,
    Dxt5,
    Unknown,
}

/// The fields of `XDDPIXELFORMAT` that `LoadHeaderFromMemory` keeps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DdsPixelFormat {
    pub size: u32,
    pub flags: u32,
    pub four_cc: u32,
    pub rgb_bit_count: u32,
    pub r_bit_mask: u32,
    pub g_bit_mask: u32,
    pub b_bit_mask: u32,
    pub rgb_alpha_bit_mask: u32,
}

/// `CDXTCImage` after a successful `LoadHeaderFromMemory`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxtcImage {
    /// `m_pbCompBufferByLevels`, as offsets from the start of the file;
    /// `None` where C++ leaves a null pointer. May point past the end of a
    /// truncated file, as the C++ pointers do.
    pub level_offsets: [Option<usize>; MAX_MIPLEVELS],
    /// `m_strFormat` ("DXT1", "DXT3", "DXT5").
    pub format_name: String,
    /// `m_CompFormat`; always DXT1, DXT3 or DXT5 after a successful load.
    pub comp_format: PixFormat,
    /// `m_lPitch`: the first level's byte size (`dwLinearSize`) or pitch.
    pub pitch: i64,
    /// `m_dwMipMapCount`: at least 1, at most [`MAX_MIPLEVELS`].
    pub mip_map_count: u32,
    /// `m_bMipTexture`: the header's mip count was above 0 (before clamping).
    pub mip_texture: bool,
    /// `m_dwFlags`: `DDSD_*`.
    pub flags: u32,
    /// `m_nWidth`.
    pub width: i32,
    /// `m_nHeight`.
    pub height: i32,
    /// `m_xddPixelFormat`.
    pub pixel_format: DdsPixelFormat,
}

/// `DecodePixelFormat`: the format name and `EPixFormat` of a pixel format.
pub fn decode_pixel_format(pf: &DdsPixelFormat) -> (String, PixFormat) {
    match pf.four_cc {
        0 => {
            // Not compressed: "ARGB-<bits of a><r><g><b>[-premul]". C++ prints
            // through snprintf(…, 31, …), which cannot overflow here.
            let a = pf.rgb_alpha_bit_mask.count_ones();
            let r = pf.r_bit_mask.count_ones();
            let g = pf.g_bit_mask.count_ones();
            let b = pf.b_bit_mask.count_ones();
            let premul = if pf.b_bit_mask & DDPF_ALPHAPREMULT != 0 { "-premul" } else { "" };
            (format!("ARGB-{a}{r}{g}{b}{premul}"), PixFormat::Argb)
        }
        x if x == fourcc(b'D', b'X', b'T', b'1') => ("DXT1".into(), PixFormat::Dxt1),
        x if x == fourcc(b'D', b'X', b'T', b'2') => ("DXT2".into(), PixFormat::Dxt2),
        x if x == fourcc(b'D', b'X', b'T', b'3') => ("DXT3".into(), PixFormat::Dxt3),
        x if x == fourcc(b'D', b'X', b'T', b'4') => ("DXT4".into(), PixFormat::Dxt4),
        x if x == fourcc(b'D', b'X', b'T', b'5') => ("DXT5".into(), PixFormat::Dxt5),
        _ => ("Format Unknown".into(), PixFormat::Unknown),
    }
}

impl DxtcImage {
    /// `CDXTCImage::LoadHeaderFromMemory`: `None` where C++ returns false
    /// (too short, not "DDS ", or not DXT1/3/5).
    pub fn load_header(data: &[u8]) -> Option<Self> {
        let magic = data.get(..4)?;
        if magic != b"DDS " {
            return None;
        }
        let h = data.get(4..4 + DDSURFACEDESC2_SIZE)?;
        let u32_at = |o: usize| u32::from_le_bytes([h[o], h[o + 1], h[o + 2], h[o + 3]]);

        let flags = u32_at(4);
        let height = u32_at(8);
        let width = u32_at(12);
        let linear_size = u32_at(16);
        let mut mip_map_count = u32_at(24);
        let pixel_format = DdsPixelFormat {
            size: u32_at(72),
            flags: u32_at(76),
            four_cc: u32_at(80),
            rgb_bit_count: u32_at(84),
            r_bit_mask: u32_at(88),
            g_bit_mask: u32_at(92),
            b_bit_mask: u32_at(96),
            rgb_alpha_bit_mask: u32_at(100),
        };

        let mip_texture = mip_map_count > 0;
        let (format_name, comp_format) = decode_pixel_format(&pixel_format);
        if !matches!(comp_format, PixFormat::Dxt1 | PixFormat::Dxt3 | PixFormat::Dxt5) {
            return None;
        }

        mip_map_count = mip_map_count.min(MAX_MIPLEVELS as u32);
        let mip_map_count = mip_map_count.max(1);

        let data_start = 4 + DDSURFACEDESC2_SIZE;
        let mut level_offsets = [None; MAX_MIPLEVELS];
        // C++ assigns the DWORD to a `long`; lPitch is a signed LONG.
        let pitch;
        if flags & DDSD_PITCH != 0 {
            pitch = i64::from(linear_size as i32);
            level_offsets[0] = Some(data_start);
        } else {
            pitch = i64::from(linear_size);
            if flags & DDSD_MIPMAPCOUNT != 0 {
                let mut size = linear_size;
                let mut at = data_start;
                for slot in level_offsets.iter_mut().take(mip_map_count as usize) {
                    *slot = Some(at);
                    at += size as usize;
                    size >>= 2;
                }
            } else {
                level_offsets[0] = Some(data_start);
            }
        }

        Some(DxtcImage {
            level_offsets,
            format_name,
            comp_format,
            pitch,
            mip_map_count,
            mip_texture,
            flags,
            width: width as i32,
            height: height as i32,
            pixel_format,
        })
    }

    /// `CDXTCImage::Copy`: the compressed bytes of `level` as uploaded to the
    /// GPU (`m_lPitch >> (level * 2)` bytes), or `None` where C++ returns
    /// false (a mip level without `DDSD_MIPMAPCOUNT`) or would read outside
    /// `file`.
    pub fn copy<'a>(&self, file: &'a [u8], level: usize) -> Option<&'a [u8]> {
        if self.flags & DDSD_MIPMAPCOUNT == 0 && level != 0 {
            return None;
        }
        let start = (*self.level_offsets.get(level)?)?;
        let len = usize::try_from(self.pitch >> (level * 2)).ok()?;
        file.get(start..start.checked_add(len)?)
    }

    /// Bytes `decompress` needs for `level`: 8 per block for DXT1, 16 else.
    pub fn level_data_size(&self, level: u32) -> Option<usize> {
        let (w, h) = self.level_size(level)?;
        let block = if self.comp_format == PixFormat::Dxt1 { 8 } else { 16 };
        Some((w / 4) as usize * (h / 4) as usize * block)
    }

    /// `(m_nWidth >> level, m_nHeight >> level)` as C++ computes them
    /// (`UINT`), or `None` for sizes that wrap or that no one could allocate.
    fn level_size(&self, level: u32) -> Option<(u32, u32)> {
        if self.width < 0 || self.height < 0 || level >= 32 {
            return None;
        }
        Some(((self.width as u32) >> level, (self.height as u32) >> level))
    }

    /// `CDXTCImage::Decompress` for DXT1/3/5: decodes `comp` (the compressed
    /// bytes of `level`, as C++ copies them into `m_bCompVector[level]`) to
    /// `(width >> level) * (height >> level)` pixels, `0xAARRGGBB`, row 0 first.
    ///
    /// Only whole 4x4 blocks are decoded; pixels outside them (sizes that are
    /// not multiples of 4, mip levels below 4x4) stay 0, as in C++ with a
    /// zeroed destination. `None` if `comp` is shorter than
    /// [`level_data_size`](Self::level_data_size).
    pub fn decompress(&self, level: u32, comp: &[u8]) -> Option<Vec<u32>> {
        let (w, h) = self.level_size(level)?;
        let (xblocks, yblocks) = ((w / 4) as usize, (h / 4) as usize);
        let block_size = if self.comp_format == PixFormat::Dxt1 { 8 } else { 16 };
        if comp.len() < xblocks * yblocks * block_size {
            return None;
        }
        let mut out = super::alloc_pixels(w as usize * h as usize)?;
        let width = w as usize;

        for y in 0..yblocks {
            for x in 0..xblocks {
                let block = &comp[(y * xblocks + x) * block_size..][..block_size];
                let (alpha, color) = if block_size == 16 { (&block[..8], &block[8..]) } else { (&block[..0], block) };
                let palette = color_block_colors(color);

                let base = x * 4 + y * 4 * width;
                for row in 0..4 {
                    let bits = color[4 + row];
                    for n in 0..4 {
                        out[base + row * width + n] = palette[usize::from((bits >> (2 * n)) & 3)];
                    }
                }
                match self.comp_format {
                    PixFormat::Dxt3 => decode_alpha_explicit(&mut out[base..], width, alpha),
                    PixFormat::Dxt5 => decode_alpha_3bit_linear(&mut out[base..], width, alpha),
                    _ => {}
                }
            }
        }
        Some(out)
    }
}

/// `GetColorBlockColors`: the four colours of a colour block as `0xAARRGGBB`.
fn color_block_colors(block: &[u8]) -> [u32; 4] {
    let raw0 = u16::from_le_bytes([block[0], block[1]]);
    let raw1 = u16::from_le_bytes([block[2], block[3]]);
    // Shift to full precision without replicating the high bits, as C++.
    let split = |raw: u16| -> [u32; 3] {
        [
            u32::from(((raw >> 11) & 0x1f) << 3 & 0xff),
            u32::from(((raw >> 5) & 0x3f) << 2 & 0xff),
            u32::from((raw & 0x1f) << 3 & 0xff),
        ]
    };
    let (c0, c1) = (split(raw0), split(raw1));
    let pack = |c: [u32; 3], a: u32| a << 24 | c[0] << 16 | c[1] << 8 | c[2];
    let mix = |wa: u32, wb: u32, div: u32| -> [u32; 3] {
        [(c0[0] * wa + c1[0] * wb) / div, (c0[1] * wa + c1[1] * wb) / div, (c0[2] * wa + c1[2] * wb) / div]
    };

    if raw0 > raw1 {
        // Four colours: 2/3 + 1/3 and 1/3 + 2/3.
        [pack(c0, 0xff), pack(c1, 0xff), pack(mix(2, 1, 3), 0xff), pack(mix(1, 2, 3), 0xff)]
    } else {
        // Three colours and transparent black.
        [pack(c0, 0xff), pack(c1, 0xff), pack(mix(1, 1, 2), 0xff), 0]
    }
}

/// `DecodeAlphaExplicit`: 4 bit alpha per pixel, replicated to 8 bits.
/// `px` starts at the block's top-left pixel; `width` is the row stride.
fn decode_alpha_explicit(px: &mut [u32], width: usize, alpha: &[u8]) {
    for row in 0..4 {
        let mut wrd = u16::from_le_bytes([alpha[row * 2], alpha[row * 2 + 1]]);
        for pix in 0..4 {
            let a = u32::from(wrd & 0xf);
            let a = a | a << 4;
            let p = &mut px[row * width + pix];
            *p = (*p & 0x00ff_ffff) | a << 24;
            wrd >>= 4;
        }
    }
}

/// `DecodeAlpha3BitLinear`: the DXT5 alpha block.
fn decode_alpha_3bit_linear(px: &mut [u32], width: usize, alpha: &[u8]) {
    let a0 = u32::from(alpha[0]);
    let a1 = u32::from(alpha[1]);
    let table: [u32; 8] = if a0 > a1 {
        [
            a0,
            a1,
            (6 * a0 + a1) / 7,
            (5 * a0 + 2 * a1) / 7,
            (4 * a0 + 3 * a1) / 7,
            (3 * a0 + 4 * a1) / 7,
            (2 * a0 + 5 * a1) / 7,
            (a0 + 6 * a1) / 7,
        ]
    } else {
        [a0, a1, (4 * a0 + a1) / 5, (3 * a0 + 2 * a1) / 5, (2 * a0 + 3 * a1) / 5, (a0 + 4 * a1) / 5, 0, 255]
    };

    // Two 24 bit little-endian halves of eight 3 bit codes each.
    for half in 0..2 {
        let o = 2 + half * 3;
        let mut bits = u32::from(alpha[o]) | u32::from(alpha[o + 1]) << 8 | u32::from(alpha[o + 2]) << 16;
        for i in 0..8 {
            let row = half * 2 + i / 4;
            let p = &mut px[row * width + i % 4];
            *p = (*p & 0x00ff_ffff) | table[(bits & 7) as usize] << 24;
            bits >>= 3;
        }
    }
}
