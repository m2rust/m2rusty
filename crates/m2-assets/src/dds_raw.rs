//! Uncompressed RGB(A) DDS files and the texture decoder's tests.

use crate::texture::{M2ImageSettings, decode};

/// Uncompressed RGB(A) DDS with 16, 24 or 32 bits per pixel and arbitrary
/// channel masks (A1R5G5B5, R5G6B5, A4R4G4B4, R8G8B8, A8R8G8B8, X8R8G8B8...).
/// Channels narrower than 8 bits are expanded by bit replication, as D3DX does.
/// Returns `(width, height, RGBA8 bytes per mip level)`.
pub(crate) fn decode_uncompressed_dds(bytes: &[u8]) -> Result<(u32, u32, Vec<Vec<u8>>), String> {
    const DDPF_RGB: u32 = 0x40;
    let u32_at = |o: usize| -> Option<u32> { Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?)) };
    let trunc = || "truncated DDS".to_owned();
    let (height, width) = (u32_at(12).ok_or_else(trunc)?, u32_at(16).ok_or_else(trunc)?);
    let mips = u32_at(28).ok_or_else(trunc)?.clamp(1, 16);
    let (flags, bits) = (u32_at(80).ok_or_else(trunc)?, u32_at(88).ok_or_else(trunc)?);
    let masks = [u32_at(92), u32_at(96), u32_at(100), u32_at(104)].map(|m| m.unwrap_or(0));
    if flags & DDPF_RGB == 0 || !matches!(bits, 16 | 24 | 32) {
        return Err(format!("unsupported DDS pixel format (flags {flags:#x}, {bits} bpp)"));
    }
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(format!("bad DDS size {width}x{height}"));
    }
    let channel = |px: u32, mask: u32, default: u8| -> u8 {
        if mask == 0 {
            return default;
        }
        let n = mask.count_ones();
        let v = (px & mask) >> mask.trailing_zeros();
        if n >= 8 { (v >> (n - 8)) as u8 } else { (v * 255 / ((1 << n) - 1)) as u8 }
    };
    let bpp = (bits / 8) as usize;
    let mut offset = 128;
    let mut levels = Vec::new();
    for level in 0..mips {
        let (w, h) = ((width >> level).max(1) as usize, (height >> level).max(1) as usize);
        let Some(src) = bytes.get(offset..offset + w * h * bpp) else {
            if level == 0 {
                return Err(trunc());
            }
            break;
        };
        offset += w * h * bpp;
        let mut rgba = Vec::with_capacity(w * h * 4);
        for p in src.chunks_exact(bpp) {
            let mut raw = [0u8; 4];
            raw[..bpp].copy_from_slice(p);
            let px = u32::from_le_bytes(raw);
            rgba.extend_from_slice(&[
                channel(px, masks[0], 0),
                channel(px, masks[1], 0),
                channel(px, masks[2], 0),
                channel(px, masks[3], 255),
            ]);
        }
        levels.push(rgba);
    }
    Ok((width, height, levels))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DDS header with the given pixel format fields.
    fn dds(width: u32, height: u32, mips: u32, flags: u32, fourcc: &[u8; 4], bits: u32, masks: [u32; 4]) -> Vec<u8> {
        let mut h = vec![0u8; 128];
        h[..4].copy_from_slice(b"DDS ");
        let mut put = |o: usize, v: u32| h[o..o + 4].copy_from_slice(&v.to_le_bytes());
        put(4, 124);
        put(8, 0x0008_1007); // caps, height, width, pixel format, linear size
        put(12, height);
        put(16, width);
        put(20, 8);
        put(28, mips);
        put(76, 32);
        put(80, flags);
        put(88, bits);
        for (i, m) in masks.into_iter().enumerate() {
            put(92 + i * 4, m);
        }
        h[84..88].copy_from_slice(fourcc);
        h
    }

    #[test]
    fn dxt1_channel_order() {
        // One 4x4 DXT1 block: colour0 = pure red (565 0xF800), colour1 = pure blue (0x001F),
        // all indices 0 -> every texel is colour0.
        let mut f = dds(4, 4, 0, 4, b"DXT1", 0, [0; 4]);
        f.extend_from_slice(&[0x00, 0xF8, 0x1F, 0x00, 0, 0, 0, 0]);
        let img = decode(&f, &M2ImageSettings::default()).unwrap();
        // 565 -> 8 bit is `<< 3` / `<< 2` without replication (CDXTCImage): R=248, G=0, B=0.
        assert_eq!(&img.data.as_ref().unwrap()[..4], [248, 0, 0, 255]);
    }

    #[test]
    fn tga_is_rgba() {
        // 2x1 uncompressed 24-bit TGA: pixels stored B,G,R: red then green.
        let mut t = vec![0u8; 18];
        t[2] = 2;
        t[12] = 2;
        t[14] = 1;
        t[16] = 24;
        t[17] = 0x20; // top-left origin
        t.extend_from_slice(&[0, 0, 255, 0, 255, 0]);
        let img = decode(&t, &M2ImageSettings::default()).unwrap();
        assert_eq!(img.data.as_ref().unwrap().as_slice(), &[255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn uncompressed_a1r5g5b5() {
        let mut f = dds(2, 1, 1, 0x41, &[0; 4], 16, [0x7c00, 0x03e0, 0x001f, 0x8000]);
        f.extend_from_slice(&0xFC00u16.to_le_bytes()); // opaque red
        f.extend_from_slice(&0x001Fu16.to_le_bytes()); // transparent blue
        let img = decode(&f, &M2ImageSettings::default()).unwrap();
        assert_eq!(img.data.as_ref().unwrap().as_slice(), &[255, 0, 0, 255, 0, 0, 255, 0]);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(decode(b"DDS ", &M2ImageSettings::default()).is_err());
        assert!(decode(&[1, 2, 3, 4, 5], &M2ImageSettings::default()).is_err());
        assert!(decode(&dds(4, 4, 0, 4, b"DXT1", 0, [0; 4]), &M2ImageSettings::default()).is_err());
    }
}
