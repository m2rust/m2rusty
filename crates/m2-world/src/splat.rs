//! Splat alpha maps: `CTerrain::RAW_GenerateSplat` and `AddTexture32`
//! (`AreaTerrain.cpp`). Each texture of the set except the first gets one
//! alpha map per terrain, marking the tiles that use it and their 8
//! neighbours.
//!
//! C++ stores the result in an `A8R8G8B8` texture with the alpha in the top
//! byte; here it is a plain 8-bit map. The Linux compatibility layer's
//! `GetLevelCount()` is 1, so the C++ reference only ever has level 0; the
//! mip chain the original D3D9 build asked for (5 levels, built by the same
//! loop) is available through [`mip_chain`].

use m2_formats::map::Terrain;

/// `SPLATALPHA_RAW_XSIZE`.
pub const RAW_SIZE: usize = 258;
/// Width of the texture `AddTexture32` creates.
pub const TEXTURE_SIZE: usize = 256;

/// The alpha map of one splat texture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplatAlpha {
    /// Index in the texture set (1 and up).
    pub texture: usize,
    /// Level 0 of the texture, `TEXTURE_SIZE²` bytes.
    pub alpha: Vec<u8>,
}

/// The 258x258 byte map `RAW_GenerateSplat` builds for texture `i`.
pub fn raw_alpha_map(tile_map: &[u8], i: u8) -> Vec<u8> {
    let n = RAW_SIZE;
    let tile = |x: usize, y: usize| tile_map[y * n + x];
    let mut out = Vec::with_capacity(n * n);
    for y in 0..n {
        for x in 0..n {
            let t = tile(x, y);
            let v = if t == i {
                0xFF
            } else if t > i {
                let get = |ok: bool, xx: usize, yy: usize| if ok { tile(xx, yy) } else { 0 };
                let (l, r, u, d) = (x > 0, x < n - 1, y > 0, y < n - 1);
                let near = [
                    get(l && u, x.wrapping_sub(1), y.wrapping_sub(1)),
                    get(r && u, x + 1, y.wrapping_sub(1)),
                    get(l && d, x.wrapping_sub(1), y + 1),
                    get(r && d, x + 1, y + 1),
                    get(u, x, y.wrapping_sub(1)),
                    get(d, x, y + 1),
                    get(l, x.wrapping_sub(1), y),
                    get(r, x + 1, y),
                ];
                if near.contains(&i) { 0xFF } else { 0x00 }
            } else {
                0x00
            };
            out.push(v);
        }
    }
    out
}

/// The 258 -> 256 resample of `AddTexture32`.
pub fn resample(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(TEXTURE_SIZE * TEXTURE_SIZE);
    for y in 0..TEXTURE_SIZE {
        for x in 0..TEXTURE_SIZE {
            let s = |o: usize| u32::from(raw[y * RAW_SIZE + x + o]);
            let sum = s(0) + s(1) + s(2) + s(258) + s(260) + s(516) + s(517) + s(518);
            out.push((((sum >> 3) + s(259)) >> 1) as u8);
        }
    }
    out
}

/// The mip levels 1..=4 of `AddTexture32`'s loop (128, 64, 32, 16 wide),
/// each a 2x2 box average of the previous one.
pub fn mip_chain(level0: &[u8]) -> Vec<Vec<u8>> {
    let mut levels: Vec<Vec<u8>> = Vec::new();
    let mut size = TEXTURE_SIZE;
    for _ in 1..5 {
        let src: &[u8] = levels.last().map_or(level0, Vec::as_slice);
        let mut dst = Vec::with_capacity(size / 2 * (size / 2));
        for y in (0..size).step_by(2) {
            for x in (0..size).step_by(2) {
                let s = |o: usize| u32::from(src[y * size + x + o]);
                dst.push(((s(0) + s(1) + s(size) + s(size + 1)) >> 2) as u8);
            }
        }
        size /= 2;
        levels.push(dst);
    }
    levels
}

/// `RAW_GenerateSplat`: one alpha map per texture `1..texture_count` that
/// the terrain uses (`TileCount > 0`), in texture order.
pub fn generate_splats(terrain: &Terrain, texture_count: usize) -> Vec<SplatAlpha> {
    let mut out = Vec::new();
    for i in 1..texture_count.min(256) {
        if terrain.tile_count[i] > 0 {
            out.push(SplatAlpha { texture: i, alpha: resample(&raw_alpha_map(&terrain.tile_map, i as u8)) });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_of_a_higher_tile_get_alpha() {
        let mut tiles = vec![0u8; RAW_SIZE * RAW_SIZE];
        tiles[10 * RAW_SIZE + 10] = 1;
        tiles[10 * RAW_SIZE + 11] = 2;
        tiles[20 * RAW_SIZE + 20] = 2;
        let a = raw_alpha_map(&tiles, 1);
        assert_eq!(a[10 * RAW_SIZE + 10], 0xFF);
        assert_eq!(a[10 * RAW_SIZE + 11], 0xFF, "higher tile next to tile 1");
        assert_eq!(a[20 * RAW_SIZE + 20], 0, "far away");
        assert_eq!(a[0], 0);
    }

    #[test]
    fn mips_halve() {
        let l0 = vec![200u8; TEXTURE_SIZE * TEXTURE_SIZE];
        let m = mip_chain(&l0);
        assert_eq!(m.iter().map(Vec::len).collect::<Vec<_>>(), [128 * 128, 64 * 64, 32 * 32, 16 * 16]);
        assert!(m.iter().flatten().all(|&b| b == 200));
    }
}
