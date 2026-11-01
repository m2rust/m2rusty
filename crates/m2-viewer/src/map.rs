//! Outdoor map viewer (`--map`): prepares a map with
//! [`m2_game::world::prepare_map`] off the main thread and shows it as a
//! [`m2_game::world::MapScene`]. This module adds only the viewer side: the
//! preload for `--map`, the inspector, and the overview camera.
//!
//! The map is addressed like `CMapOutdoor`: a map directory name
//! (`metin2_map_a1`) whose `MapProperty.txt` may redirect every other file to
//! its `ParentMapName`.

use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy_egui::egui;
use m2_assets::M2Packs;
use m2_formats::pack::PackManager;
use m2_game::render::sky;
use m2_game::world::{MapScene, PreparedMap, prepare_map};
use m2_world::TERRAIN_WORLD;

use crate::index::FileKind;
use crate::viewer::{ContentViewer, MainCamera, OpenFile, PendingOpen, ViewerEntity};

/// The `map` the viewer handles. The shell routes by extension, and a map has
/// no file of its own in the browser, so `--map` opens a synthetic
/// `<map>/map.map` entry (see [`map_open_file`]).
const MAP_EXT: &str = "map";

type MapTask = Task<Result<Arc<PreparedMap>, String>>;

fn spawn_prepare(packs: Arc<PackManager>, name: String) -> MapTask {
    AsyncComputeTaskPool::get().spawn(async move { prepare_map(&packs, &name).map(Arc::new) })
}

/// `--map`: the background load started before the viewer opens. The shell
/// needs a `PendingOpen` before the screenshot job starts counting frames, so
/// the map is loaded first and the viewer opens on the finished data.
#[derive(Resource, Default)]
pub struct MapPreload {
    pub map: Option<String>,
    /// `--fog`: apply the environment fog from the start.
    pub fog: bool,
    task: Option<MapTask>,
    loaded: Option<Arc<PreparedMap>>,
    error: Option<String>,
    /// Whether the synthetic `PendingOpen` was already queued.
    opened: bool,
}

pub struct MapViewerPlugin;

impl Plugin for MapViewerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapPreload>().add_systems(Startup, start_preload).add_systems(Update, poll_preload);
    }
}

fn start_preload(mut preload: ResMut<MapPreload>, packs: Res<M2Packs>) {
    let Some(name) = preload.map.clone() else {
        return;
    };
    preload.task = Some(spawn_prepare(Arc::clone(&packs.packs), name));
}

fn poll_preload(mut preload: ResMut<MapPreload>, mut pending: ResMut<PendingOpen>, packs: Res<M2Packs>) {
    if preload.opened {
        return;
    }
    let Some(task) = &mut preload.task else {
        return;
    };
    let Some(result) = check_ready(task) else {
        return;
    };
    preload.task = None;
    let name = preload.map.clone().unwrap_or_default();
    match result {
        Ok(prepared) => {
            let map = &prepared.map;
            info!(
                "loaded map {}: {}x{} terrains ({} loaded, {} failed), {} texture(s)",
                map.name,
                map.terrain_count.0,
                map.terrain_count.1,
                map.terrains.len(),
                map.terrain_failed,
                map.textures.len()
            );
            preload.loaded = Some(prepared);
        }
        Err(e) => {
            error!("map {name}: {e}");
            preload.error = Some(e);
        }
    }
    pending.0 = Some(map_open_file(&name, Arc::clone(&packs.packs)));
    preload.opened = true;
}

/// The map a browser entry stands for: a map has no single file of its own, so
/// selecting its `MapProperty.txt` (or `Setting.txt`) opens the whole map.
pub fn map_dir_of_entry(name: &str) -> Option<String> {
    let (dir, file) = name.rsplit_once('/')?;
    if dir.is_empty() {
        return None;
    }
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem).to_ascii_lowercase();
    matches!(stem.as_str(), "mapproperty" | "setting").then(|| dir.to_owned())
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

#[derive(Default)]
pub struct MapViewer {
    path: String,
    scene: Option<MapScene>,
    task: Option<MapTask>,
    error: Option<String>,
    fit_camera: bool,
    /// Apply the environment fog. Off for the overview camera, which sits far
    /// outside the map's fog range; on when the view is near the ground.
    fog_enabled: bool,
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
        self.fog_enabled = world.resource::<MapPreload>().fog;
        self.path = m2_assets::paths::canonical_game_path(m2_assets::paths::parent(&file.name));
        let (loaded, error) = {
            let mut preload = world.resource_mut::<MapPreload>();
            (preload.loaded.take(), preload.error.take())
        };
        if let Some(map) = loaded.filter(|prepared| prepared.map.name == self.path) {
            self.spawn(world, map);
        } else if let Some(error) = error {
            self.error = Some(error);
        } else {
            // Viewing a map without the CLI preload (a browser entry).
            let packs = Arc::clone(&world.resource::<M2Packs>().packs);
            self.task = Some(spawn_prepare(packs, self.path.clone()));
        }
    }

    fn update(&mut self, world: &mut World) {
        if self.scene.is_none() {
            let Some(task) = &mut self.task else { return };
            let Some(result) = check_ready(task) else {
                return;
            };
            self.task = None;
            match result {
                Ok(map) => self.spawn(world, map),
                Err(e) => self.error = Some(e),
            }
        }
        let Some(scene) = &mut self.scene else { return };
        scene.update(world);
        if self.fit_camera {
            fit_camera(world, &scene.map);
            self.fit_camera = false;
        }
    }

    fn ui(&mut self, world: &mut World, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("Outdoor map");
        ui.label(&self.path);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let Some(scene) = &self.scene else {
            if self.error.is_none() {
                ui.spinner();
                ui.label("Loading map setting and terrain tiles…");
            }
            return;
        };
        let map = &scene.map.map;
        let mut refog = false;
        if let Some(error) = &map.property_error {
            ui.colored_label(egui::Color32::YELLOW, error);
        }
        if let Some(error) = &map.setting_error {
            ui.colored_label(egui::Color32::YELLOW, error);
        }
        match &map.environment {
            Some(environment) if environment.fog.enable => {
                let fog = &environment.fog;
                ui.label(format!(
                    "Fog: {} · {:.0}–{:.0}",
                    if fog.density { "exp" } else { "linear" },
                    fog.near_distance,
                    fog.far_distance
                ));
            }
            Some(_) => {
                ui.label("Fog: off");
            }
            None => {
                if let Some(error) = &map.environment_error {
                    ui.colored_label(egui::Color32::YELLOW, error);
                }
            }
        }
        if map.environment.is_some() {
            refog = ui.checkbox(&mut self.fog_enabled, "Environment fog").changed();
        }
        ui.label(format!("Map size: {} × {} terrains", map.terrain_count.0, map.terrain_count.1));
        ui.label(format!("Loaded terrains: {} ({} failed)", map.terrains.len(), map.terrain_failed));
        if let Some(error) = &map.terrain_error {
            ui.label(format!("First terrain error: {error}"));
        }
        ui.label(format!("Height scale: {}", map.height_scale));
        ui.label(format!("Texture set: {} entries", map.textures.len()));
        ui.label(format!("Objects: {} loaded · {} pending", scene.objects_loaded, scene.pending_objects()));
        ui.label(format!("Effects: {}", scene.effects.instances.len()));
        let errors = map.object_errors.iter().chain(&scene.map.errors).chain(&scene.object_errors);
        ui.label(format!("Load errors: {}", errors.clone().count()));
        for error in errors.take(12) {
            ui.colored_label(egui::Color32::YELLOW, error);
        }
        let stats = scene.stats;
        ui.label(format!("Patches: {} · {} vertices · {} triangles", stats.patches, stats.vertices, stats.triangles));
        ui.label("Terrain LOD 0 · texture splats · animated water");
        if ui.button("Fit map").clicked() {
            self.fit_camera = true;
        }
        ui.label("Orbit: drag · pan: right/middle drag · zoom: wheel");
        if refog {
            self.rebuild(world);
        }
    }

    fn close(&mut self, world: &mut World) {
        sky::clear_sky(world);
    }
}

impl MapViewer {
    fn spawn(&mut self, world: &mut World, map: Arc<PreparedMap>) {
        // The map is 25 600 world units per terrain: a few terrain sides away
        // from the default camera's far plane is already clipped.
        set_far_plane(world, 400_000.0);
        if let Some(environment) = &map.map.environment {
            set_clear_color(world, environment.fog.color);
        }
        let scene = MapScene::spawn(world, Arc::clone(&map), self.fog_enabled);
        world.entity_mut(scene.root).insert(ViewerEntity);
        self.scene = Some(scene);
        fit_camera(world, &map);
    }

    /// Rebuilds the scene after a render-state change (the fog toggle).
    fn rebuild(&mut self, world: &mut World) {
        let Some(scene) = self.scene.take() else { return };
        crate::viewer::despawn_viewer_entities(world);
        self.spawn(world, scene.map);
    }
}

/// The clear colour behind the sky and terrain: the environment fog colour
/// keeps the horizon seamless.
fn set_clear_color(world: &mut World, color: [f32; 4]) {
    let mut query = world.query_filtered::<&mut Camera, With<MainCamera>>();
    if let Ok(mut camera) = query.single_mut(world) {
        camera.clear_color = ClearColorConfig::Custom(Color::srgb(color[0], color[1], color[2]));
    }
}

fn set_far_plane(world: &mut World, far: f32) {
    let mut query = world.query_filtered::<&mut Projection, With<MainCamera>>();
    if let Ok(mut projection) = query.single_mut(world)
        && let Projection::Perspective(perspective) = &mut *projection
    {
        perspective.near = 0.5;
        perspective.far = far;
    }
}

/// Frames every loaded terrain: `radius` is the diagonal of the loaded area,
/// which the 45° vertical field of view covers at pitch ~55°.
fn fit_camera(world: &mut World, map: &PreparedMap) {
    let size = TERRAIN_WORLD as f32;
    let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
    let (mut min_z, mut max_z) = (f32::MAX, f32::MIN);
    let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
    for cell in &map.map.terrains {
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
    fn a_map_is_opened_from_its_property_or_setting_file() {
        assert_eq!(map_dir_of_entry("metin2_map_b1/mapproperty.txt").as_deref(), Some("metin2_map_b1"));
        assert_eq!(map_dir_of_entry("metin2_map_b1/Setting.TXT").as_deref(), Some("metin2_map_b1"));
        // Anything else stays an ordinary file.
        assert_eq!(map_dir_of_entry("metin2_map_b1/0000001.tga"), None);
        assert_eq!(map_dir_of_entry("mapproperty.txt"), None);
    }
}
