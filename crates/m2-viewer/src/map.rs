//! Outdoor map viewer (`--map`): loads a map through `m2-formats`/`m2-world`
//! off the main thread and draws every terrain patch as its own mesh.
//!
//! The map is addressed like `CMapOutdoor`: a map directory name
//! (`metin2_map_a1`) whose `MapProperty.txt` may redirect every other file to
//! its `ParentMapName`. Terrain patches are built by [`m2_world::mesh`] and
//! textured with the first texture of the map's texture set.

use std::sync::Arc;

use bevy::asset::{AssetServer, Assets, RenderAssetUsages};
use bevy::ecs::hierarchy::ChildOf;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy_egui::egui;
use m2_assets::M2Packs;
use m2_assets::coords;
use m2_assets::paths;
use m2_formats::map::{
    AreaPropertyError, MapProperty, MapSetting, PropertyError, SettingError, TERRAIN_TEX_COORD_BASE, Terrain,
    TerrainTexture, TextureSet, splat_alpha_matrix,
};
use m2_formats::pack::PackManager;
use m2_render::{FixedFunctionMaterial, Lighting};
use m2_world::mesh::{DEFAULT_OPAQUE_WATER_DEPTH, build_index_buffers, build_terrain_patches};
use m2_world::{TERRAIN_WORLD, TerrainCell};

use crate::index::FileKind;
use crate::viewer::{ContentViewer, MainCamera, OpenFile, PendingOpen, ViewerEntity};

/// The `map` the viewer handles. The shell routes by extension, and a map has
/// no file of its own in the browser, so `--map` opens a synthetic
/// `<map>/map.map` entry (see [`map_open_file`]).
const MAP_EXT: &str = "map";

/// One map, loaded off the main thread.
pub struct LoadedMap {
    pub name: String,
    pub terrain_count: (i16, i16),
    pub terrain_loaded: usize,
    pub terrain_failed: usize,
    pub terrain_error: Option<String>,
    pub height_scale: f32,
    pub texture_set_len: usize,
    /// First texture of the set; the ones the client splats on top need the
    /// second UV set the material does not have yet.
    pub texture: Option<TerrainTexture>,
    pub terrains: Vec<TerrainCell>,
    pub property_error: Option<String>,
    pub setting_error: Option<String>,
}

/// `--map`: the background load started before the viewer opens. The shell
/// needs a `PendingOpen` before the screenshot job starts counting frames, so
/// the map is loaded first and the viewer opens on the finished data.
#[derive(Resource, Default)]
pub struct MapPreload {
    pub map: Option<String>,
    task: Option<Task<Result<Arc<LoadedMap>, String>>>,
    loaded: Option<Arc<LoadedMap>>,
    error: Option<String>,
    /// Whether the synthetic `PendingOpen` was already queued.
    opened: bool,
}

pub struct MapViewerPlugin;

impl Plugin for MapViewerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapPreload>()
            .add_systems(Startup, start_preload)
            .add_systems(Update, poll_preload);
    }
}

fn start_preload(mut preload: ResMut<MapPreload>, packs: Res<M2Packs>) {
    let Some(name) = preload.map.clone() else { return };
    let packs = Arc::clone(&packs.packs);
    preload.task = Some(AsyncComputeTaskPool::get().spawn(async move { load_map(packs, name).map(Arc::new) }));
}

fn poll_preload(mut preload: ResMut<MapPreload>, mut pending: ResMut<PendingOpen>, packs: Res<M2Packs>) {
    if preload.opened {
        return;
    }
    let Some(task) = &mut preload.task else { return };
    let Some(result) = check_ready(task) else { return };
    preload.task = None;
    let name = preload.map.clone().unwrap_or_default();
    match result {
        Ok(map) => {
            info!(
                "loaded map {}: {}x{} terrains ({} loaded, {} failed), {} texture(s)",
                map.name,
                map.terrain_count.0,
                map.terrain_count.1,
                map.terrain_loaded,
                map.terrain_failed,
                map.texture_set_len
            );
            preload.loaded = Some(map);
        }
        Err(e) => {
            error!("map {name}: {e}");
            preload.error = Some(e);
        }
    }
    pending.0 = Some(map_open_file(&name, Arc::clone(&packs.packs)));
    preload.opened = true;
}

/// Synthetic shell entry that routes to [`MapViewer`].
pub fn map_open_file(name: &str, manager: Arc<PackManager>) -> OpenFile {
    OpenFile {
        name: format!("{name}/map.{MAP_EXT}"),
        display: name.to_owned(),
        ext: MAP_EXT.to_owned(),
        kind: FileKind::Other,
        bytes: Arc::new(Vec::new()),
        manager,
    }
}

/// Loads the property, setting and every terrain of the map, mirroring
/// `CMapOutdoor::LoadMap` closely enough for a preview: a file that fails is
/// skipped, the rest still loads.
fn load_map(packs: Arc<PackManager>, name: String) -> Result<LoadedMap, String> {
    let map_name = name.as_bytes().to_vec();
    let property = MapProperty::load(&packs, &map_name);
    let property_error = property.error.as_ref().map(describe_property_error);
    let data_directory = property.data_directory(&map_name).to_vec();

    let mut setting = MapSetting::load(&packs, &data_directory);
    let setting_error = setting.error.take().map(|e| describe_setting_error(&e));
    let (count_x, count_y) = setting.terrain_count;
    if count_x <= 0 || count_y <= 0 {
        return Err(setting_error.unwrap_or_else(|| format!("{name}: MapSize is {}x{count_y}", count_x)));
    }

    let mut terrains = Vec::new();
    let mut terrain_failed = 0usize;
    let mut terrain_error = None;
    for ty in 0..count_y as u16 {
        for tx in 0..count_x as u16 {
            match Terrain::load(&packs, &data_directory, tx, ty, setting.height_scale) {
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
    // Index 0 is the client's eraser (a default texture with no file); the
    // ground is the first texture that names one.
    let texture = textures.iter().find(|t| !t.file_name.is_empty()).cloned();
    Ok(LoadedMap {
        name,
        terrain_count: (count_x, count_y),
        terrain_loaded: terrains.len(),
        terrain_failed,
        terrain_error,
        height_scale: setting.height_scale,
        texture_set_len: textures.len(),
        texture,
        terrains,
        property_error,
        setting_error,
    })
}

/// Error text for the inspector: the map file errors only derive `Debug`, and
/// their pack payload is a path that reads much better through `Display`.
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

#[derive(Default, Clone, Copy)]
struct SpawnStats {
    patches: usize,
    vertices: usize,
    triangles: usize,
}

#[derive(Default)]
pub struct MapViewer {
    path: String,
    map: Option<Arc<LoadedMap>>,
    task: Option<Task<Result<Arc<LoadedMap>, String>>>,
    error: Option<String>,
    stats: SpawnStats,
    fit_camera: bool,
}

impl ContentViewer for MapViewer {
    fn id(&self) -> &'static str {
        "map"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[MAP_EXT]
    }

    fn open(&mut self, world: &mut World, file: &OpenFile) {
        *self = Self::default();
        self.path = paths::canonical_game_path(paths::parent(&file.name));
        let (loaded, error) = {
            let mut preload = world.resource_mut::<MapPreload>();
            (preload.loaded.take(), preload.error.take())
        };
        if let Some(map) = loaded.filter(|map| map.name == self.path) {
            self.spawn(world, map);
        } else if let Some(error) = error {
            self.error = Some(error);
        } else {
            // Viewing a map without the CLI preload (future browser entry).
            let packs = Arc::clone(&world.resource::<M2Packs>().packs);
            let name = self.path.clone();
            self.task = Some(AsyncComputeTaskPool::get().spawn(async move { load_map(packs, name).map(Arc::new) }));
        }
    }

    fn update(&mut self, world: &mut World) {
        if self.map.is_none() {
            let Some(task) = &mut self.task else { return };
            let Some(result) = check_ready(task) else { return };
            self.task = None;
            match result {
                Ok(map) => self.spawn(world, map),
                Err(e) => self.error = Some(e),
            }
        }
        if self.fit_camera {
            if let Some(map) = &self.map {
                fit_camera(world, map);
                self.fit_camera = false;
            }
        }
    }

    fn ui(&mut self, world: &mut World, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("Outdoor map");
        ui.label(&self.path);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let Some(map) = &self.map else {
            if self.error.is_none() {
                ui.spinner();
                ui.label("Loading map setting and terrain tiles…");
            }
            return;
        };
        if let Some(error) = &map.property_error {
            ui.colored_label(egui::Color32::YELLOW, error);
        }
        if let Some(error) = &map.setting_error {
            ui.colored_label(egui::Color32::YELLOW, error);
        }
        ui.label(format!("Map size: {} × {} terrains", map.terrain_count.0, map.terrain_count.1));
        ui.label(format!("Loaded terrains: {} ({} failed)", map.terrain_loaded, map.terrain_failed));
        if let Some(error) = &map.terrain_error {
            ui.label(format!("First terrain error: {error}"));
        }
        ui.label(format!("Height scale: {}", map.height_scale));
        ui.label(format!("Texture set: {} entries", map.texture_set_len));
        match &map.texture {
            Some(texture) => ui.label(format!(
                "Ground texture: {}",
                String::from_utf8_lossy(&texture.file_name).replace('\\', "/")
            )),
            None => ui.label("Ground texture: none in the texture set"),
        };
        ui.label(format!(
            "Patches: {} · {} vertices · {} triangles",
            self.stats.patches, self.stats.vertices, self.stats.triangles
        ));
        ui.label("LOD 0 only; splat layers and water are not drawn yet.");
        if ui.button("Fit map").clicked() {
            self.fit_camera = true;
        }
        let _ = world;
        ui.label("Orbit: drag · pan: right/middle drag · zoom: wheel");
    }
}

impl MapViewer {
    fn spawn(&mut self, world: &mut World, map: Arc<LoadedMap>) {
        // The map is 25 600 world units per terrain: a few terrain sides away
        // from the default camera's far plane is already clipped.
        set_far_plane(world, 400_000.0);

        let texture = map.texture.as_ref().map(|texture| {
            let path = String::from_utf8_lossy(&texture.file_name).into_owned();
            world.resource::<AssetServer>().load(paths::asset_source_uri(&path))
        });
        let base = match &texture {
            Some(texture) => FixedFunctionMaterial::opaque(texture.clone()),
            None => FixedFunctionMaterial::untextured().diffuse(Vec4::new(0.42, 0.46, 0.32, 1.0)),
        };
        let lighting = Lighting::directional(
            Vec3::new(0.35, -0.85, 0.4).normalize(),
            Vec4::new(0.95, 0.93, 0.88, 1.0),
            Vec4::splat(0.35),
        );
        let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(base.lit(lighting));

        let root =
            world.spawn((ViewerEntity, Name::new(map.name.clone()), Transform::IDENTITY, Visibility::Visible)).id();
        let splat_alpha = splat_alpha_matrix();
        let indices = strip_to_triangles(&build_index_buffers()[0]);
        let u_scale = map.texture.as_ref().map_or(0.0, |t| t.u_scale);
        let v_scale = map.texture.as_ref().map_or(0.0, |t| t.v_scale);
        let (u_offset, v_offset) = map.texture.as_ref().map_or((0.0, 0.0), |t| (t.u_offset, t.v_offset));

        let mut stats = SpawnStats::default();
        for cell in &map.terrains {
            let patches = build_terrain_patches(&cell.terrain, cell.x, cell.y, DEFAULT_OPAQUE_WATER_DEPTH);
            for (index, patch) in patches.iter().enumerate() {
                let positions: Vec<[f32; 3]> =
                    patch.vertices.iter().map(|v| coords::vec3(v.position).to_array()).collect();
                let normals: Vec<[f32; 3]> = patch.vertices.iter().map(|v| coords::vec3(v.normal).to_array()).collect();
                // Stage 0 uses the repeating ground UV. TODO(phase 3): the
                // material samples UV_0 in both stages, so the splat alpha map
                // (which needs `VertexUv::alpha`, a different UV set) cannot
                // be bound yet; m2-render needs a per-stage UV or texture
                // matrix before `m2_world::splat` can be drawn.
                let uvs: Vec<[f32; 2]> = m2_world::lighting::vertex_uvs(
                    &patch.vertices,
                    u32::from(cell.x),
                    u32::from(cell.y),
                    TERRAIN_TEX_COORD_BASE,
                    &splat_alpha,
                )
                .iter()
                .map(|uv| tile_uv(uv.tile, u_scale, v_scale, u_offset, v_offset))
                .collect();

                let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
                mesh.insert_indices(Indices::U32(indices.clone()));
                let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);

                stats.patches += 1;
                stats.vertices += patch.vertices.len();
                stats.triangles += indices.len() / 3;
                world.spawn((
                    ViewerEntity,
                    Name::new(format!("patch {:03}{:03} #{}", cell.x, cell.y, index)),
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    Visibility::Visible,
                    ChildOf(root),
                ));
            }
        }
        info!(
            "map {}: {} patch(es), {} vertices, {} triangles, texture set of {}",
            map.name, stats.patches, stats.vertices, stats.triangles, map.texture_set_len
        );
        self.stats = stats;
        self.map = Some(map);
        fit_camera(world, self.map.as_ref().unwrap());
    }
}

/// Terrain UV of `1/640` per unit (5 repeats per 3200-unit patch) scaled by
/// the texture set entry, whose scale is relative to that 5-repeat default.
fn tile_uv(tile: [f32; 2], u_scale: f32, v_scale: f32, u_offset: f32, v_offset: f32) -> [f32; 2] {
    [tile[0] * u_scale * 0.2 + u_offset, tile[1] * v_scale * 0.2 - v_offset]
}

/// The LOD 0 terrain index buffer is a triangle strip; Bevy meshes are
/// triangle lists with counter-clockwise front faces, which is what
/// [`m2_assets::coords::flip_winding`] gives a Metin2 (clockwise-front) list.
fn strip_to_triangles(strip: &[u16]) -> Vec<u32> {
    let mut out = Vec::with_capacity(strip.len().saturating_sub(2) * 3);
    for (i, w) in strip.windows(3).enumerate() {
        let (a, b, c) = (u32::from(w[0]), u32::from(w[1]), u32::from(w[2]));
        if a == b || b == c || a == c {
            continue;
        }
        if i % 2 == 0 {
            out.extend_from_slice(&[a, b, c]);
        } else {
            out.extend_from_slice(&[b, a, c]);
        }
    }
    coords::flip_winding(&mut out);
    out
}

fn set_far_plane(world: &mut World, far: f32) {
    let mut query = world.query_filtered::<&mut Projection, With<MainCamera>>();
    if let Ok(mut projection) = query.single_mut(world) {
        if let Projection::Perspective(perspective) = &mut *projection {
            perspective.near = 0.5;
            perspective.far = far;
        }
    }
}

/// Frames every loaded terrain: `radius` is the diagonal of the loaded area,
/// which the 45° vertical field of view covers at pitch ~55°.
fn fit_camera(world: &mut World, map: &LoadedMap) {
    let size = TERRAIN_WORLD as f32;
    let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
    let (mut min_z, mut max_z) = (f32::MAX, f32::MIN);
    let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
    for cell in &map.terrains {
        let x = f32::from(cell.x) * size;
        // Game Y is negated for display, so terrain rows run towards -Z.
        let z = -(f32::from(cell.y) + 1.0) * size;
        min_x = min_x.min(x);
        max_x = max_x.max(x + size);
        min_z = min_z.min(z);
        max_z = max_z.max(z + size);
        for &height in &cell.terrain.height_map {
            let height = f32::from(height) * cell.terrain.height_scale;
            min_y = min_y.min(height);
            max_y = max_y.max(height);
        }
    }
    if min_x > max_x {
        return;
    }
    let focus = Vec3::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5, (min_z + max_z) * 0.5);
    let radius = ((max_x - min_x).hypot(max_z - min_z) * 1.2).max(100.0);
    let mut query = world.query_filtered::<(&mut crate::camera::OrbitCamera, &mut Transform), With<MainCamera>>();
    if let Ok((mut orbit, mut transform)) = query.single_mut(world) {
        orbit.focus = focus;
        orbit.yaw = 0.6;
        orbit.pitch = 0.95;
        orbit.radius = radius;
        *transform = orbit.transform();
    }
    set_far_plane(world, (radius * 4.0).max(400_000.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_becomes_a_list_with_flipped_winding() {
        // Two strip triangles, the second one reversed by the strip itself.
        let tris = strip_to_triangles(&[0, 1, 2, 3]);
        assert_eq!(tris.len(), 6);
        // `[0,1,2]` keeps its order before the Metin2 winding flip, so the
        // stored triangle is `[0,2,1]`; the odd `[1,2,3]` is `[2,1,3]` first.
        assert_eq!(&tris[..3], &[0, 2, 1]);
        assert_eq!(&tris[3..], &[2, 3, 1]);
    }

    #[test]
    fn texture_scale_is_relative_to_five_repeats_per_patch() {
        // A patch is 3200 units wide; UV 1/640 per unit repeats 5 times.
        let tile = [1.0, -1.0];
        assert_eq!(tile_uv(tile, 5.0, 5.0, 0.0, 0.0), [1.0, -1.0]);
        assert_eq!(tile_uv(tile, 10.0, 10.0, 0.0, 0.0), [2.0, -2.0]);
        assert_eq!(tile_uv(tile, 5.0, 5.0, 0.25, 0.5), [1.25, -1.5]);
    }
}
