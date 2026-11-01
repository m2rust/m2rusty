//! Per-terrain files (`CTerrainImpl` in `neoeter/src/terrain/Terrain.cpp`,
//! `CTerrain` in `client/src/game/AreaTerrain.cpp`, driven by
//! `CMapOutdoor::LoadTerrain`). All little-endian:
//!
//! - `AreaProperty.txt`: `ScriptType AreaProperty`, `AreaName "…"`.
//! - `height.raw`: 131×131 `u16`, the 129×129 vertex heights with a
//!   one-cell border.
//! - `attr.atr`: `u16` magic 2634, width 256, height 256, then 256×256
//!   attribute bytes (block, water, ban-PK flags).
//! - `tile.raw`: 258×258 texture-set indices (256×256 tiles + border).
//! - `water.wtr`: `u16` magic 5426, width 128, height 128, `u8` layer count
//!   `n`, 128×128 layer indices, then `n` `i32` heights.
//! - `shadowmap.raw`: 256×256 `u16` R5G6B5.
//!
//! On load C++ also derives the normal map (from the heights and the map's
//! height scale) and the per-patch tile counts used to pick splats; both are
//! here. The splat alpha textures and patch geometry are not.

use crate::pack::{self, PackManager};
use crate::script::parse_multiple_text_data;

use super::{TERRAIN_SIZE, token};

/// `CTerrainImpl::CELLSCALE`: world units per cell.
pub const CELLSCALE: u32 = 200;
/// `HEIGHTMAP_RAW_XSIZE`.
pub const HEIGHTMAP_RAW_SIZE: usize = TERRAIN_SIZE + 3;
/// `NORMALMAP_XSIZE`.
pub const NORMALMAP_SIZE: usize = TERRAIN_SIZE + 1;
/// `ATTRMAP_XSIZE`.
pub const ATTRMAP_SIZE: usize = TERRAIN_SIZE * 2;
/// `TILEMAP_RAW_XSIZE`.
pub const TILEMAP_RAW_SIZE: usize = TERRAIN_SIZE * 2 + 2;
/// `WATERMAP_XSIZE`.
pub const WATERMAP_SIZE: usize = TERRAIN_SIZE;
/// `SHADOWMAP_XSIZE`.
pub const SHADOWMAP_SIZE: usize = TERRAIN_SIZE * 2;
/// `PATCH_XCOUNT`: patches per terrain side.
pub const PATCH_COUNT: usize = TERRAIN_SIZE / super::PATCH_SIZE;
/// `PATCH_TILE_XSIZE`: tiles per patch side.
const PATCH_TILE_SIZE: usize = super::PATCH_SIZE * 2;
/// `MAX_WATER_NUM`.
pub const MAX_WATER_NUM: usize = 255;
/// `MAXTERRAINTEXTURES`.
const MAX_TEXTURES: usize = 256;

const ATTR_MAGIC: u16 = 2634;
const WATER_MAGIC: u16 = 5426;

/// Why `AreaProperty.txt` stopped `CMapOutdoor::LoadTerrain` (no terrain
/// is created then).
#[derive(Debug)]
pub enum AreaPropertyError {
    Pack(pack::Error),
    MissingKey(&'static str),
    /// `ScriptType` is not `AreaProperty`.
    ScriptType,
}

/// `AreaProperty.txt` as `CMapOutdoor::LoadTerrain` reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaProperty {
    /// `AreaName`, given to `CTerrain::SetName`.
    pub area_name: Vec<u8>,
}

/// The checks of `CMapOutdoor::LoadTerrain` on `AreaProperty.txt`.
pub fn parse_area_property(data: &[u8]) -> Result<AreaProperty, AreaPropertyError> {
    let map = parse_multiple_text_data(data);
    let Some(script_type) = map.get(b"scripttype".as_slice()) else {
        return Err(AreaPropertyError::MissingKey("scripttype"));
    };
    let Some(name) = map.get(b"areaname".as_slice()) else {
        return Err(AreaPropertyError::MissingKey("areaname"));
    };
    if token(script_type, 0) != b"AreaProperty" {
        return Err(AreaPropertyError::ScriptType);
    }
    Ok(AreaProperty {
        area_name: token(name, 0).to_vec(),
    })
}

/// What each loader of `CMapOutdoor::LoadTerrain` returned (C++ only
/// traces failures and keeps the terrain).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerrainLoad {
    pub water: bool,
    pub height: bool,
    pub attr: bool,
    pub tile: bool,
    pub shadow: bool,
}

/// The data of a `CTerrain` after `CMapOutdoor::LoadTerrain`, starting
/// from a freshly constructed one.
#[derive(Debug, Clone)]
pub struct Terrain {
    /// `m_strName`: the area name.
    pub name: Vec<u8>,
    /// `m_fHeightScale`, copied from the map setting.
    pub height_scale: f32,
    /// `m_awRawHeightMap`, `HEIGHTMAP_RAW_SIZE²`, row-major; vertex
    /// `(x, y)` (0..=128) is at `(y + 1) * 131 + x + 1`.
    pub height_map: Vec<u16>,
    /// `m_acNormalMap`: `NORMALMAP_SIZE²` signed x, y, z triples scaled to
    /// 127 (`CTerrain::CalculateNormal`).
    pub normal_map: Vec<i8>,
    /// `m_abyAttrMap`, `ATTRMAP_SIZE²`.
    pub attr_map: Vec<u8>,
    /// `m_abyTileMap`, `TILEMAP_RAW_SIZE²`: texture-set index per tile.
    pub tile_map: Vec<u8>,
    /// `m_abyWaterMap`, `WATERMAP_SIZE²`: water layer per cell.
    pub water_map: Vec<u8>,
    /// `m_byNumWater`.
    pub num_water: u8,
    /// `m_lWaterHeight`: `MAX_WATER_NUM + 1` entries, -1 past `num_water`.
    pub water_heights: Vec<i32>,
    /// `m_awShadowMap`, `SHADOWMAP_SIZE²` (0xFFFF when not loaded).
    pub shadow_map: Vec<u16>,
    /// `m_TerrainSplatPatch.TileCount`: tiles per texture index.
    pub tile_count: Vec<u32>,
    /// `m_TerrainSplatPatch.PatchTileCount`: per patch (row-major,
    /// `PATCH_COUNT²`), tiles per texture index.
    pub patch_tile_count: Vec<[u32; MAX_TEXTURES]>,
}

impl Default for Terrain {
    fn default() -> Self {
        Self {
            name: Vec::new(),
            height_scale: 0.0,
            height_map: vec![0; HEIGHTMAP_RAW_SIZE * HEIGHTMAP_RAW_SIZE],
            normal_map: vec![0; NORMALMAP_SIZE * NORMALMAP_SIZE * 3],
            attr_map: vec![0; ATTRMAP_SIZE * ATTRMAP_SIZE],
            tile_map: vec![0; TILEMAP_RAW_SIZE * TILEMAP_RAW_SIZE],
            water_map: vec![0; WATERMAP_SIZE * WATERMAP_SIZE],
            num_water: 0,
            water_heights: vec![-1; MAX_WATER_NUM + 1],
            shadow_map: vec![0xFFFF; SHADOWMAP_SIZE * SHADOWMAP_SIZE],
            tile_count: vec![0; MAX_TEXTURES],
            patch_tile_count: vec![[0; MAX_TEXTURES]; PATCH_COUNT * PATCH_COUNT],
        }
    }
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

/// `std::stringstream::read` into `u16`s: copies what is there (a final odd
/// byte becomes the low byte) and fails if it was short.
fn read_u16s(dst: &mut [u16], data: &[u8]) -> bool {
    let n = data.len().min(dst.len() * 2);
    for (i, chunk) in data[..n].chunks(2).enumerate() {
        dst[i] = match *chunk {
            [lo, hi] => u16::from_le_bytes([lo, hi]),
            [lo] => (dst[i] & 0xFF00) | u16::from(lo),
            _ => unreachable!(),
        };
    }
    data.len() >= dst.len() * 2
}

/// `std::stringstream::read` into bytes.
fn read_bytes(dst: &mut [u8], data: &[u8]) -> bool {
    let n = data.len().min(dst.len());
    dst[..n].copy_from_slice(&data[..n]);
    data.len() >= dst.len()
}

/// `PR_FLOAT_TO_INT` (`(long)std::floor(f)`) as x86-64 computes it:
/// `cvttss2si` gives `i64::MIN` for NaN and out-of-range values.
fn float_to_long(f: f32) -> i64 {
    let f = f.floor();
    if f.is_nan() || f >= 9.223_372e18 || f < -9.223_372e18 { i64::MIN } else { f as i64 }
}

impl Terrain {
    /// `CTerrainImpl::LoadHeightMap`: reads up to 131×131 heights. False
    /// when the file is shorter; what it had is kept.
    pub fn load_height_map(&mut self, data: &[u8]) -> bool {
        read_u16s(&mut self.height_map, data)
    }

    /// `GetHeightMapValue(sx, sy)`.
    pub fn height(&self, x: i32, y: i32) -> u16 {
        self.height_map[((y + 1) as usize) * HEIGHTMAP_RAW_SIZE + (x + 1) as usize]
    }

    /// `CTerrain::CalculateNormal` for every vertex, which
    /// `CTerrain::LoadHeightMap` runs whether or not the heights loaded.
    pub fn calculate_normals(&mut self) {
        for y in 0..NORMALMAP_SIZE as i32 {
            for x in 0..NORMALMAP_SIZE as i32 {
                let h = |x, y| f32::from(self.height(x, y));
                let nx = -self.height_scale * (h(x - 1, y) - h(x + 1, y));
                let ny = -self.height_scale * (h(x, y - 1) - h(x, y + 1));
                let nz = 2.0 * CELLSCALE as f32;
                let scale = 127.0 / (nx * nx + ny * ny + nz * nz).sqrt();
                let i = (y as usize * NORMALMAP_SIZE + x as usize) * 3;
                for (k, c) in [nx, ny, nz].into_iter().enumerate() {
                    self.normal_map[i + k] = float_to_long(c * scale) as i8;
                }
            }
        }
    }

    /// `CTerrainImpl::LoadAttrMap`.
    pub fn load_attr_map(&mut self, data: &[u8]) -> bool {
        if data.len() < 6
            || u16_at(data, 0) != ATTR_MAGIC
            || usize::from(u16_at(data, 2)) != ATTRMAP_SIZE
            || usize::from(u16_at(data, 4)) != ATTRMAP_SIZE
        {
            return false;
        }
        read_bytes(&mut self.attr_map, &data[6..])
    }

    /// `CTerrainImpl::RAW_LoadTileMap`.
    pub fn load_tile_map(&mut self, data: &[u8]) -> bool {
        read_bytes(&mut self.tile_map, data)
    }

    /// `CTerrain::LoadShadowMap`: only a file of exactly 256×256 `u16`.
    pub fn load_shadow_map(&mut self, data: &[u8]) -> bool {
        if data.len() != self.shadow_map.len() * 2 {
            return false;
        }
        read_u16s(&mut self.shadow_map, data)
    }

    /// `CTerrainImpl::LoadWaterMapFile`.
    ///
    /// Besides the `i32` heights C++ accepts a variant with `u16` heights,
    /// but after reading it falls through to the `i32` path and reads the
    /// map again past the end, so that variant always fails. A file with no
    /// layers is that variant too (both sizes agree), so it fails as well.
    /// Mirrored.
    fn load_water_map_file(&mut self, data: &[u8]) -> bool {
        const HEADER: usize = 7;
        if data.len() < HEADER
            || u16_at(data, 0) != WATER_MAGIC
            || usize::from(u16_at(data, 2)) != WATERMAP_SIZE
            || usize::from(u16_at(data, 4)) != WATERMAP_SIZE
        {
            return false;
        }
        self.num_water = data[6];
        let n = usize::from(self.num_water);
        let rest = &data[HEADER..];
        let map_len = self.water_map.len();
        // DWORD arithmetic; rest is far below 4 GiB for anything real.
        let rest_len = rest.len() as u32;
        let need = (map_len + 4 * n) as u32;
        let need_word = (map_len + 2 * n) as u32;

        let mut pos = 0;
        if rest_len == need_word {
            pos += map_len;
            self.water_map.copy_from_slice(&rest[..map_len]);
            for i in 0..n.min(MAX_WATER_NUM) {
                self.water_heights[i] = i32::from(u16_at(rest, pos + 2 * i));
            }
            pos += 2 * n.min(MAX_WATER_NUM);
        } else if rest_len != need {
            return false;
        }

        let rest = &rest[pos..];
        if !read_bytes(&mut self.water_map, rest) {
            return false;
        }
        let rest = &rest[map_len..];
        if n > 0 {
            for i in 0..n {
                let Some(b) = rest.get(4 * i..4 * i + 4) else {
                    return false;
                };
                self.water_heights[i] = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            }
        }
        true
    }

    /// `CTerrainImpl::LoadWaterMap`: on failure the map is 0xFF, there are
    /// no layers and every height is -1. `None` is a missing file.
    pub fn load_water_map(&mut self, data: Option<&[u8]>) -> bool {
        if data.is_some_and(|d| self.load_water_map_file(d)) {
            return true;
        }
        self.water_map.fill(0xFF);
        self.num_water = 0;
        self.water_heights.fill(-1);
        false
    }

    /// `CTerrain::RAW_CountTiles` (from `CTerrain::RAW_LoadTileMap`): counts
    /// each tile into its patch and, on patch borders, into the neighbours
    /// that share it.
    pub fn count_tiles(&mut self) {
        const N: i64 = TILEMAP_RAW_SIZE as i64;
        const PT: i64 = PATCH_TILE_SIZE as i64;
        const PC: i64 = PATCH_COUNT as i64;
        self.tile_count.fill(0);
        self.patch_tile_count.iter_mut().for_each(|p| p.fill(0));

        for y in 0..N {
            let py = ((y - 1) / PT).clamp(0, PC - 1);
            for x in 0..N {
                let px = ((x - 1) / PT).clamp(0, PC - 1);
                let tile = usize::from(self.tile_map[(y * N + x) as usize]);
                let mut add = |py: i64, px: i64| self.patch_tile_count[(py * PC + px) as usize][tile] += 1;

                add(py, px);
                let right = x % PT == 0 && x != 0 && x != N - 2;
                let left = x % PT == 1 && x != N - 1 && x != 1;
                let (next_x, prev_x) = ((px + 1).min(PC - 1), (px - 1).max(0));

                if y % PT == 0 && y != 0 && y != N - 2 {
                    let below = (py + 1).min(PC - 1);
                    add(below, px);
                    if right {
                        add(py, next_x);
                        add(below, next_x);
                    } else if left {
                        add(py, prev_x);
                        add(below, prev_x);
                    }
                } else if y % PT == 1 && y != N - 1 && y != 1 {
                    let above = (py - 1).max(0);
                    add(above, px);
                    if right {
                        add(py, next_x);
                        add(above, next_x);
                    } else if left {
                        add(py, prev_x);
                        add(above, prev_x);
                    }
                } else if right {
                    add(py, next_x);
                } else if left {
                    add(py, prev_x);
                }

                self.tile_count[tile] += 1;
            }
        }
    }

    /// `CMapOutdoor::LoadTerrain` minus textures and patches: checks
    /// `AreaProperty.txt` (any failure there means no terrain), then loads
    /// water, heights (and normals), attributes, tiles (and tile counts)
    /// and the shadow map from `<data dir>\<id>\`.
    pub fn load(
        manager: &PackManager,
        map_data_dir: &[u8],
        x: u16,
        y: u16,
        height_scale: f32,
    ) -> Result<(Self, TerrainLoad), AreaPropertyError> {
        let name = super::terrain_file_name_untruncated(map_data_dir, x, y, "AreaProperty.txt");
        let property = parse_area_property(&manager.get(name).map_err(AreaPropertyError::Pack)?)?;

        let get = |file: &str| manager.get(super::terrain_file_name(map_data_dir, x, y, file)).ok();
        let mut terrain = Self {
            height_scale,
            ..Self::default()
        };
        let mut load = TerrainLoad {
            water: terrain.load_water_map(get("water.wtr").as_deref()),
            ..TerrainLoad::default()
        };
        load.height = get("height.raw").is_some_and(|d| terrain.load_height_map(&d));
        terrain.calculate_normals();
        load.attr = get("attr.atr").is_some_and(|d| terrain.load_attr_map(&d));
        load.tile = get("tile.raw").is_some_and(|d| terrain.load_tile_map(&d));
        terrain.count_tiles();
        load.shadow = get("shadowmap.raw").is_some_and(|d| terrain.load_shadow_map(&d));
        terrain.name = property.area_name;
        Ok((terrain, load))
    }
}
