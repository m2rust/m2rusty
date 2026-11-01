//! Outdoor map files, as the client loads them (`client/src/game`:
//! `MapBase.cpp`, `MapOutdoorLoad.cpp`, `Area.cpp`, `AreaTerrain.cpp`,
//! `MapManager.cpp`; `neoeter/src/terrain`: `Terrain.cpp`, `TextureSet.cpp`).
//!
//! A map is a directory named after it (`metin2_map_a1`, or the
//! `ParentMapName` of its `MapProperty.txt`):
//!
//! ```text
//! metin2_map_a1/MapProperty.txt    MapProperty     [`MapProperty`]
//! metin2_map_a1/Setting.txt        MapSetting      [`MapSetting`] (+ its [`TextureSet`])
//! metin2_map_a1/regen.txt          monster areas   [`MonsterArea`]
//! metin2_map_a1/001002/            terrain/area x=1 y=2 ([`terrain_id`])
//!     AreaData.txt, AreaAmbienceData.txt           [`Area`]
//!     AreaProperty.txt, height.raw, attr.atr,
//!     tile.raw, water.wtr, shadowmap.raw           [`Terrain`]
//! ```
//!
//! plus `AtlasInfo.txt` ([`AtlasEntry`]), the list of maps and their world
//! positions. Images (`shadowmap.dds`, `minimap.dds`, textures) are left to
//! the image readers. The indoor (`MAPTYPE_INDOOR`) loader is not
//! implemented in the client; `CDungeonBlock` only loads a `.gr2` model, so
//! indoor maps have no format of their own beyond the outdoor files.
//!
//! The loaders keep C++'s partial results: a `Load…` that fails half way
//! leaves what it had already stored, and the map loader carries on.

mod area;
mod atlas;
mod monster;
mod setting;
mod terrain;
mod texture_set;

pub use area::{Area, AreaFileError, ObjectData, PORTAL_ID_MAX_NUM, parse_area_ambience_data, parse_area_data};
pub use atlas::{AtlasEntry, parse_atlas_info};
pub use monster::{MonsterArea, MonsterAreaKind, parse_monster_areas};
pub use setting::{
    ENVIRONMENT_ROOT, MapProperty, MapSetting, MapType, PropertyError, SettingError, splat_alpha_matrix,
    static_shadow_matrix, texture_set_file_name,
};
pub use terrain::{
    AreaProperty, AreaPropertyError, ATTRMAP_SIZE, CELLSCALE, HEIGHTMAP_RAW_SIZE, MAX_WATER_NUM, NORMALMAP_SIZE,
    PATCH_COUNT, SHADOWMAP_SIZE, Terrain, TerrainLoad, TILEMAP_RAW_SIZE, WATERMAP_SIZE, parse_area_property,
};
pub use texture_set::{TERRAIN_TEX_COORD_BASE, TerrainTexture, TextureSet, TextureSetError};

/// `CTerrainImpl::XSIZE`/`YSIZE`: cells per terrain side.
pub const TERRAIN_SIZE: usize = 128;
/// `CTerrainImpl::PATCH_XSIZE`: cells per patch side.
pub const PATCH_SIZE: usize = 16;
/// `CTerrainImpl::TERRAIN_XSIZE`: world units per terrain side.
pub const TERRAIN_WORLD_SIZE: u32 = (TERRAIN_SIZE as u32) * CELLSCALE;

/// `atol`: `strtol(s, NULL, 10)` with a 64-bit `long` (the reference build
/// is LP64). Same parsing as [`crate::script::atoi`] without the final
/// truncation to 32 bits.
pub fn atol(s: &[u8]) -> i64 {
    let start = s.iter().position(|&b| !matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')).unwrap_or(s.len());
    let mut s = &s[start..];
    let negative = match s.first() {
        Some(b'-') => {
            s = &s[1..];
            true
        }
        Some(b'+') => {
            s = &s[1..];
            false
        }
        _ => false,
    };
    let mut value: i64 = 0;
    for &b in s.iter().take_while(|b| b.is_ascii_digit()) {
        let d = i64::from(b - b'0');
        value = if negative {
            value.saturating_mul(10).saturating_sub(d)
        } else {
            value.saturating_mul(10).saturating_add(d)
        };
    }
    value
}

/// The `ulID` of `CMapOutdoor::LoadArea`/`LoadTerrain`: `x * 1000 + y`,
/// printed with `%06u` as the directory name.
pub fn terrain_id(x: u16, y: u16) -> u32 {
    u32::from(x) * 1000 + u32::from(y)
}

/// `_snprintf(buf, 64 + 1, "%s\\%06u\\%s", …)`: the path of a file in a
/// terrain directory, cut to 64 bytes as `CMapOutdoor::LoadArea` and
/// `LoadTerrain` do for everything but `AreaProperty.txt`.
pub fn terrain_file_name(map_dir: &[u8], x: u16, y: u16, file: &str) -> Vec<u8> {
    let mut name = terrain_file_name_untruncated(map_dir, x, y, file);
    name.truncate(64);
    name
}

/// `_snprintf(buf, 64 + 1, "%s\\%06u\\", …)`: the area directory of
/// `CMapOutdoor::LoadArea`, cut to 64 bytes; `CArea::Load` appends the file
/// names to it.
pub fn area_dir_name(map_dir: &[u8], x: u16, y: u16) -> Vec<u8> {
    let mut name = terrain_file_name_untruncated(map_dir, x, y, "");
    name.truncate(64);
    name
}

/// `sprintf(buf, "%s\\%06u\\%s", …)` without the cut (used for
/// `AreaProperty.txt`).
pub fn terrain_file_name_untruncated(map_dir: &[u8], x: u16, y: u16, file: &str) -> Vec<u8> {
    let mut name = map_dir.to_vec();
    name.extend_from_slice(format!("\\{:06}\\{file}", terrain_id(x, y)).as_bytes());
    name
}

/// What C++ reads for `vector[i]` of a token vector. Out of range is
/// undefined behaviour there; here it is an empty token.
pub(crate) fn token(tokens: &[Vec<u8>], i: usize) -> &[u8] {
    tokens.get(i).map_or(&[], Vec::as_slice)
}
