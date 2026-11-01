//! `MapProperty.txt` (`CMapBase::LoadProperty`) and `Setting.txt`
//! (`CMapOutdoor::LoadSetting`, `client/src/game/MapOutdoorLoad.cpp`).
//!
//! ```text
//! ScriptType  MapSetting
//! CellScale   200
//! HeightScale 0.500000
//! ViewRadius  128
//! MapSize     4 5
//! BasePosition 409600 896000
//! TextureSet  textureset\metin2_A1.txt
//! Environment A1.msenv
//! ```

use crate::pack::{self, PackManager};
use crate::script::{TokenVectorMap, atof_f32, atoi, parse_multiple_text_data};

use super::texture_set::{TERRAIN_TEX_COORD_BASE, TextureSet, TextureSetError};
use super::{atol, token};

/// `CMapBase::EMAPTYPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapType {
    #[default]
    Invalid = 0,
    Indoor = 1,
    Outdoor = 2,
}

/// Why `CMapBase::LoadProperty` returned false.
#[derive(Debug)]
pub enum PropertyError {
    Pack(pack::Error),
    MissingKey(&'static str),
    /// `ScriptType` is not `MapProperty`.
    ScriptType,
}

/// What `CMapBase::LoadProperty` stores.
#[derive(Debug, Default)]
pub struct MapProperty {
    /// `m_strParentMapName`: when set, every other map file is read from
    /// this directory instead of the map's own (`GetMapDataDirectory`).
    pub parent_map_name: Vec<u8>,
    /// `MapType` `Indoor`, or `Outdoor`/`Invalid` (both outdoor); anything
    /// else leaves the type unset.
    pub map_type: MapType,
    /// `Some` when `LoadProperty` returned false. The fields keep what it
    /// had stored by then (the parent name is stored before the script type
    /// is checked).
    pub error: Option<PropertyError>,
}

impl MapProperty {
    /// `CMapBase::LoadProperty` after reading the file.
    pub fn parse(data: &[u8]) -> Self {
        let mut map = parse_multiple_text_data(data);
        let mut property = Self::default();
        property.error = property.fill(&mut map).err();
        property
    }

    fn fill(&mut self, map: &mut TokenVectorMap) -> Result<(), PropertyError> {
        for key in ["scripttype", "maptype"] {
            if !map.contains_key(key.as_bytes()) {
                return Err(PropertyError::MissingKey(key));
            }
        }
        if let Some(parent) = map.get(b"parentmapname".as_slice()) {
            self.parent_map_name = token(parent, 0).to_vec();
        }
        if token(&map[b"scripttype".as_slice()], 0) != b"MapProperty" {
            return Err(PropertyError::ScriptType);
        }
        match token(&map[b"maptype".as_slice()], 0) {
            b"Indoor" => self.map_type = MapType::Indoor,
            b"Outdoor" | b"Invalid" => self.map_type = MapType::Outdoor,
            _ => {}
        }
        Ok(())
    }

    /// `CMapBase::LoadProperty`: reads `<map name>\MapProperty.txt`.
    pub fn load(manager: &PackManager, map_name: &[u8]) -> Self {
        let mut name = map_name.to_vec();
        name.extend_from_slice(b"\\MapProperty.txt");
        match manager.get(&name) {
            Ok(data) => Self::parse(&data),
            Err(e) => Self {
                error: Some(PropertyError::Pack(e)),
                ..Self::default()
            },
        }
    }

    /// `CMapBase::GetMapDataDirectory`: the parent map's name for a copied
    /// map, else the map's own.
    pub fn data_directory<'a>(&'a self, map_name: &'a [u8]) -> &'a [u8] {
        if self.parent_map_name.is_empty() { map_name } else { &self.parent_map_name }
    }
}

/// Why `CMapOutdoor::LoadSetting` returned false.
#[derive(Debug)]
pub enum SettingError {
    Pack(pack::Error),
    /// One of `scripttype`, `viewradius`, `cellscale`, `heightscale`,
    /// `mapsize`, `textureset` is missing.
    MissingKey(&'static str),
    /// `ScriptType` is not `MapSetting`.
    ScriptType,
    /// `ViewRadius` is not greater than 0.
    ViewRadius,
    /// The texture set did not load.
    TextureSet(TextureSetError),
}

/// What `CMapOutdoor::LoadSetting` stores, starting from the values of a
/// freshly initialised `CMapOutdoor`.
#[derive(Debug, Default)]
pub struct MapSetting {
    /// `m_bSettingTerrainVisible`: `TerrainVisible` (default true).
    pub terrain_visible: bool,
    /// `m_lViewRadius`, in cells.
    pub view_radius: i64,
    /// `m_fHeightScale`: world height per height map unit.
    pub height_scale: f32,
    /// `m_sTerrainCountX`/`Y` from `MapSize`; both stay 0 when either is 0
    /// or above 256 (`SetTerrainCount` refuses them).
    pub terrain_count: (i16, i16),
    /// `m_dwBaseX`/`Y` from `BasePosition` (0 without one).
    pub base: (u32, u32),
    /// The texture set as loaded (empty if it was not reached or failed).
    pub texture_set: TextureSet,
    /// `m_settings_envDataName`: the first `Environment` token.
    pub environment: Vec<u8>,
    /// `Some` when `LoadSetting` returned false. The fields keep what it
    /// had stored by then.
    pub error: Option<SettingError>,
}

/// The texture set file name `LoadSetting` passes to `CTextureSet::Load`.
///
/// C++ means to prefix `textureset\` unless the name already starts with
/// it, but tests `find_first_of("textureset") != 0`, so any name whose first
/// byte is one of `t e x u r s` is used as is. Mirrored.
pub fn texture_set_file_name(texture_set: &[u8]) -> Vec<u8> {
    if texture_set.first().is_some_and(|b| b"textureset".contains(b)) {
        texture_set.to_vec()
    } else {
        let mut name = b"textureset\\".to_vec();
        name.extend_from_slice(texture_set);
        name
    }
}

/// `m_matSplatAlpha` as `LoadSetting` sets it (row-major `_11`…`_44`).
pub fn splat_alpha_matrix() -> [f32; 16] {
    let base = TERRAIN_TEX_COORD_BASE;
    let mut m = identity();
    m[0] = base * 2.0 * 16.0 / 256.0;
    m[5] = -base * 2.0 * 16.0 / 256.0;
    m[10] = 0.0;
    m[12] = base * 4.6;
    m[13] = base * 4.6;
    m
}

/// `m_matStaticShadow` as `LoadSetting` sets it.
pub fn static_shadow_matrix() -> [f32; 16] {
    let base = TERRAIN_TEX_COORD_BASE;
    let mut m = identity();
    m[0] = base * (16.0 / 128.0);
    m[5] = -base * (16.0 / 128.0);
    m[10] = 0.0;
    m
}

fn identity() -> [f32; 16] {
    std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 })
}

/// The environment root of `CMapOutdoor::Load`.
pub const ENVIRONMENT_ROOT: &[u8] = b"d:/ymir work/environment/";

impl MapSetting {
    /// `CMapOutdoor::LoadSetting` after reading the file; the texture set
    /// is read through `load_texture_set` (given the resolved file name).
    pub fn parse(data: &[u8], load_texture_set: impl FnOnce(&[u8]) -> Result<TextureSet, TextureSetError>) -> Self {
        let mut map = parse_multiple_text_data(data);
        let mut setting = Self::default();
        setting.error = setting.fill(&mut map, load_texture_set).err();
        setting
    }

    fn fill(
        &mut self,
        map: &mut TokenVectorMap,
        load_texture_set: impl FnOnce(&[u8]) -> Result<TextureSet, TextureSetError>,
    ) -> Result<(), SettingError> {
        for key in ["scripttype", "viewradius", "cellscale", "heightscale", "mapsize", "textureset"] {
            if !map.contains_key(key.as_bytes()) {
                return Err(SettingError::MissingKey(key));
            }
        }
        self.terrain_visible = map.get(b"terrainvisible".as_slice()).is_none_or(|v| atoi(token(v, 0)) != 0);

        let get = |key: &str| &map[key.as_bytes()];
        let script_type = token(get("scripttype"), 0);
        let view_radius = token(get("viewradius"), 0);
        let height_scale = token(get("heightscale"), 0);
        let size = get("mapsize");
        let texture_set = token(get("textureset"), 0).to_vec();

        if script_type != b"MapSetting" {
            return Err(SettingError::ScriptType);
        }
        self.view_radius = atol(view_radius);
        if self.view_radius <= 0 {
            return Err(SettingError::ViewRadius);
        }
        self.height_scale = atof_f32(height_scale);

        // SetTerrainCount(short, short).
        let (x, y) = (atoi(token(size, 0)) as i16, atoi(token(size, 1)) as i16);
        if (1..=256).contains(&x) && (1..=256).contains(&y) {
            self.terrain_count = (x, y);
        }
        if let Some(base) = map.get(b"baseposition".as_slice()) {
            self.base = (atol(token(base, 0)) as u32, atol(token(base, 1)) as u32);
        }

        self.texture_set = load_texture_set(&texture_set_file_name(&texture_set)).map_err(SettingError::TextureSet)?;

        if let Some(env) = map.get(b"environment".as_slice())
            && let Some(first) = env.first()
        {
            self.environment = first.clone();
        }
        Ok(())
    }

    /// `CMapOutdoor::LoadSetting(<data dir>\Setting.txt)` with its texture
    /// set.
    pub fn load(manager: &PackManager, map_data_dir: &[u8]) -> Self {
        let mut name = map_data_dir.to_vec();
        name.extend_from_slice(b"\\Setting.txt");
        match manager.get(&name) {
            Ok(data) => Self::parse(&data, |ts| TextureSet::load(manager, ts, TERRAIN_TEX_COORD_BASE)),
            Err(e) => Self {
                error: Some(SettingError::Pack(e)),
                ..Self::default()
            },
        }
    }

    /// The environment file `CMapOutdoor::Load` settles on: `<data
    /// dir>\<Environment>` if `exists`, else
    /// `d:/ymir work/environment/<Environment>`, and with no `Environment`
    /// the map name's last two bytes + `.msenv`.
    ///
    /// Deviation: C++ throws (`substr` out of range) for a map name shorter
    /// than two bytes; here the whole name is used.
    pub fn environment_file_name(&self, map_name: &[u8], map_data_dir: &[u8], exists: impl FnOnce(&[u8]) -> bool) -> Vec<u8> {
        let mut local = map_data_dir.to_vec();
        local.push(b'\\');
        local.extend_from_slice(&self.environment);
        if exists(&local) {
            return local;
        }
        let mut name = ENVIRONMENT_ROOT.to_vec();
        if self.environment.is_empty() {
            name.extend_from_slice(&map_name[map_name.len().saturating_sub(2)..]);
            name.extend_from_slice(b".msenv");
        } else {
            name.extend_from_slice(&self.environment);
        }
        name
    }
}
