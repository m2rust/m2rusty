//! Loading a whole outdoor map: `MapProperty.txt`, `Setting.txt`, the
//! environment, every terrain and the placed area objects.
//!
//! This follows `CMapOutdoor::LoadMap` closely enough for drawing the map: a
//! file that fails is recorded and skipped, the rest still loads. It loads every
//! terrain at once; the game's 3×3 streaming around the player is not here yet.
//! Rendering (meshes, textures, materials) belongs to the Bevy side.

use m2_formats::map::{
    Area, AreaPropertyError, EnvironmentData, MapProperty, MapSetting, PropertyError, SettingError, Terrain,
    TerrainTexture, TextureSet, area_dir_name, load_environment,
};
use m2_formats::pack::PackManager;
use m2_formats::property::PropertyManager;

use crate::TerrainCell;
use crate::area::{PlacedObject, place_object};

/// One map's data. Errors that did not stop the load are kept as text for
/// the UI.
#[derive(Debug, Clone)]
pub struct LoadedMap {
    /// The map directory name, e.g. `metin2_map_a1`.
    pub name: String,
    pub terrain_count: (i16, i16),
    pub terrain_failed: usize,
    /// The first terrain that failed to load.
    pub terrain_error: Option<String>,
    pub height_scale: f32,
    /// The texture set; entry 0 is the client's unused placeholder slot.
    pub textures: Vec<TerrainTexture>,
    /// Buildings, trees, effects, ambience and dungeon blocks of every area.
    pub objects: Vec<PlacedObject>,
    pub environment: Option<EnvironmentData>,
    pub environment_error: Option<String>,
    /// Unresolved properties, area errors and missing terrain textures.
    pub object_errors: Vec<String>,
    pub terrains: Vec<TerrainCell>,
    pub property_error: Option<String>,
    pub setting_error: Option<String>,
}

/// Loads map `name` (a map directory such as `metin2_map_a1`; its
/// `MapProperty.txt` may redirect the data to its `ParentMapName`). Fails only
/// when the map has no size or no terrain loads.
pub fn load_map(packs: &PackManager, name: &str) -> Result<LoadedMap, String> {
    let map_name = name.as_bytes().to_vec();
    let property = MapProperty::load(packs, &map_name);
    let property_error = property.error.as_ref().map(describe_property_error);
    let data_directory = property.data_directory(&map_name).to_vec();

    let mut setting = MapSetting::load(packs, &data_directory);
    let setting_error = setting.error.take().map(|e| describe_setting_error(&e));
    let environment_name = setting.environment_file_name(&map_name, &data_directory, |n| packs.exists(n));
    let (environment, environment_error) = match load_environment(packs, &environment_name) {
        Ok(environment) => (Some(environment), None),
        Err(error) => (None, Some(format!("Environment {}: {error}", String::from_utf8_lossy(&environment_name)))),
    };
    let (count_x, count_y) = setting.terrain_count;
    if count_x <= 0 || count_y <= 0 {
        return Err(setting_error.unwrap_or_else(|| format!("{name}: MapSize is {count_x}x{count_y}")));
    }

    let mut props = PropertyManager::default();
    props.initialize(packs);
    let mut objects = Vec::new();
    let mut object_errors = Vec::new();
    let mut terrains = Vec::new();
    let mut terrain_failed = 0usize;
    let mut terrain_error = None;
    for ty in 0..count_y as u16 {
        for tx in 0..count_x as u16 {
            let dir = area_dir_name(&data_directory, tx, ty);
            let area = Area::load(packs, &dir, |_| true);
            if let Some(error) = area.object_error {
                object_errors.push(format!("Area {tx},{ty}: {error:?}"));
            }
            for object in area.objects {
                if let Some(placed) = place_object(&object, &props, &|path| packs.exists(path)) {
                    objects.push(placed);
                } else {
                    object_errors.push(format!("Unresolved property CRC {}", object.crc));
                }
            }
            match Terrain::load(packs, &data_directory, tx, ty, setting.height_scale) {
                Ok((terrain, _load)) => terrains.push(TerrainCell { x: tx, y: ty, terrain }),
                Err(e) => {
                    terrain_failed += 1;
                    if terrain_error.is_none() {
                        terrain_error = Some(format!("{tx:03}{ty:03}: {}", describe_area_error(&e)));
                    }
                }
            }
        }
    }
    if terrains.is_empty() {
        return Err(terrain_error.unwrap_or_else(|| format!("{name}: no terrain loaded")));
    }

    let TextureSet { textures, .. } = setting.texture_set;
    for texture in &textures {
        if !texture.file_name.is_empty() && !packs.exists(&texture.file_name) {
            object_errors.push(format!("Missing terrain texture: {}", String::from_utf8_lossy(&texture.file_name)));
        }
    }
    Ok(LoadedMap {
        name: name.to_owned(),
        terrain_count: (count_x, count_y),
        terrain_failed,
        terrain_error,
        height_scale: setting.height_scale,
        textures,
        objects,
        environment,
        environment_error,
        object_errors,
        terrains,
        property_error,
        setting_error,
    })
}

/// The map file errors only derive `Debug`, and their pack payload is a path
/// that reads much better through `Display`.
fn describe_property_error(error: &PropertyError) -> String {
    match error {
        PropertyError::Pack(e) => format!("MapProperty.txt: {e}"),
        PropertyError::MissingKey(key) => format!("MapProperty.txt: missing {key}"),
        PropertyError::ScriptType => "MapProperty.txt: ScriptType is not MapProperty".into(),
    }
}

fn describe_area_error(error: &AreaPropertyError) -> String {
    match error {
        AreaPropertyError::Pack(e) => format!("AreaProperty.txt: {e}"),
        AreaPropertyError::MissingKey(key) => format!("AreaProperty.txt: missing {key}"),
        AreaPropertyError::ScriptType => "AreaProperty.txt: ScriptType is not AreaProperty".into(),
    }
}

fn describe_setting_error(error: &SettingError) -> String {
    match error {
        SettingError::Pack(e) => format!("Setting.txt: {e}"),
        SettingError::MissingKey(key) => format!("Setting.txt: missing {key}"),
        SettingError::ScriptType => "Setting.txt: ScriptType is not MapSetting".into(),
        SettingError::ViewRadius => "Setting.txt: ViewRadius must be above 0".into(),
        SettingError::TextureSet(e) => format!("Setting.txt: texture set: {e:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::area::PlacedKind;

    #[test]
    #[ignore = "requires the bundled client packs under bin/"]
    fn bundled_a1_loads_terrains_objects_and_environment() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin");
        let mut packs = PackManager::new(root);
        m2_formats::pack::load_index(&mut packs, "pack", "pack/Index.dev").unwrap();
        let map = load_map(&packs, "metin2_map_a1").unwrap();
        assert_eq!(map.terrains.len(), 20);
        assert_eq!(map.terrain_failed, 0);
        assert!(map.property_error.is_none(), "{:?}", map.property_error);
        assert!(map.setting_error.is_none(), "{:?}", map.setting_error);
        assert!(map.object_errors.is_empty(), "{:?}", map.object_errors);
        assert!(map.objects.iter().any(|o| matches!(o.kind, PlacedKind::Building { .. })));
        assert!(map.objects.iter().any(|o| matches!(o.kind, PlacedKind::Tree { .. })));
        assert!(map.objects.iter().any(|o| matches!(o.kind, PlacedKind::Effect { .. })));
        let environment = map.environment.as_ref().expect("a1 has an environment");
        assert!(environment.fog.enable);
        assert!(environment.fog.far_distance > environment.fog.near_distance);
        for texture in map.textures.iter().skip(1) {
            assert!(packs.exists(&texture.file_name), "{:?}", texture.file_name);
        }
        assert!(map.terrains.iter().any(|cell| crate::splat::generate_splats(&cell.terrain, map.textures.len()).len() > 1));
    }
}
