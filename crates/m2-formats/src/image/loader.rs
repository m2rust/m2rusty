//! `Platform_GetImageInfo` and `Platform_DecodeImage`
//! (`neoeter/src/platform/base/ImageLoaderCompat.cpp`): what the client's image
//! loading entry points return for a file's bytes.

use super::stb;

/// D3DFMT_A8R8G8B8.
pub const D3DFMT_A8R8G8B8: u32 = 21;
/// D3DFMT_R8G8B8.
pub const D3DFMT_R8G8B8: u32 = 20;
/// D3DFMT_DXT1 (`'DXT1'`).
pub const D3DFMT_DXT1: u32 = 0x3154_5844;
/// D3DFMT_DXT3 (`'DXT3'`).
pub const D3DFMT_DXT3: u32 = 0x3354_5844;
/// D3DFMT_DXT5 (`'DXT5'`).
pub const D3DFMT_DXT5: u32 = 0x3554_5844;

/// Output of `Platform_GetImageInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    /// A `D3DFMT_*` value.
    pub format: u32,
}

/// `Platform_GetImageInfo`: `None` where it returns false.
///
/// A DDS (magic only; the header is not validated) reports the dimensions at
/// byte offsets 16/12 and DXT1/3/5 by the fourcc at 84, anything else as
/// A8R8G8B8. Other files go through `stbi_info`; if stb does not know them the
/// first four TGA header fields at bytes 12..16 are tried as a last resort.
///
/// Deviation: C++ reads bytes 12..20 of a DDS shorter than 20 bytes (and bytes
/// 12..16 of any file shorter than 16) from beyond the buffer; those return
/// `None` here.
pub fn get_image_info(data: &[u8]) -> Option<ImageInfo> {
    if data.len() < 18 {
        return None;
    }

    let u32_at = |o: usize| -> Option<u32> { Some(u32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?)) };

    if data.starts_with(b"DDS ") {
        let height = u32_at(12)?;
        let width = u32_at(16)?;
        let format = if data.len() >= 88 && data[84..87] == *b"DXT" {
            match data[87] {
                b'1' => D3DFMT_DXT1,
                b'3' => D3DFMT_DXT3,
                b'5' => D3DFMT_DXT5,
                _ => D3DFMT_A8R8G8B8,
            }
        } else {
            D3DFMT_A8R8G8B8
        };
        return Some(ImageInfo { width, height, format });
    }

    // stb_image takes an `int` length.
    if i32::try_from(data.len()).is_ok()
        && let Some(i) = stb::info(data)
    {
        let format = if i.components == 4 { D3DFMT_A8R8G8B8 } else { D3DFMT_R8G8B8 };
        return Some(ImageInfo { width: i.width, height: i.height, format });
    }

    // Try TGA raw header fallback (bytes 12-15)
    let tga_w = u16::from_le_bytes([data[12], data[13]]);
    let tga_h = u16::from_le_bytes([data[14], data[15]]);
    if (1..=4096).contains(&tga_w) && (1..=4096).contains(&tga_h) {
        return Some(ImageInfo { width: u32::from(tga_w), height: u32::from(tga_h), format: D3DFMT_A8R8G8B8 });
    }
    None
}

/// Output of `Platform_DecodeImage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, B G R A.
    pub bgra: Vec<u8>,
}

/// `Platform_DecodeImage`: stb_image to RGBA, then red and blue swapped. `None`
/// where it returns false: fewer than 4 bytes, an unknown format, a corrupt
/// file, or a format this port does not decode (see [`stb`]). DDS files are not
/// stb formats and always fail here.
pub fn decode_image(data: &[u8]) -> Option<DecodedImage> {
    if data.len() < 4 || i32::try_from(data.len()).is_err() {
        return None;
    }
    let mut img = stb::load_rgba(data)?;
    for px in img.pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Some(DecodedImage { width: img.width, height: img.height, bgra: img.pixels })
}
