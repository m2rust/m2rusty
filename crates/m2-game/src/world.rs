//! An outdoor map on screen: [`prepare_map`] loads it with
//! [`m2_world::load_map`] and builds what is slow to build (the ground texture
//! array, the effect instances) off the main thread; [`MapScene::spawn`] puts
//! it in the world under one root, and [`MapScene::update`] runs it every frame
//! (objects as their assets arrive, effects, animated water).
//!
//! The viewer can spawn a full-map overview. In game, sector data and scene
//! entities stream around the player and terrain patches select one of the
//! original three LOD index buffers.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::asset::{AssetServer, Assets, LoadState, RenderAssetUsages, UntypedAssetId};
use bevy::ecs::hierarchy::ChildOf;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use m2_assets::{coords, gr2::Gr2Model, paths, spt::SptTree};
use m2_effect::EffectInstance;
use m2_formats::map::{TERRAIN_TEX_COORD_BASE, splat_alpha_matrix};
use m2_formats::pack::PackManager;
use m2_render::{Blend, CullMode, DepthState, FixedFunctionMaterial, Fog, TextureArg, TextureOp, TextureStage};
use m2_world::LoadedMap;
use m2_world::area::{PlacedKind, PlacedObject, rotation_yaw_pitch_roll};
use m2_world::mesh::{DEFAULT_OPAQUE_WATER_DEPTH, build_index_buffers, build_terrain_patches};

use crate::battle::{self, D3dTransform, StaticCollision};
use crate::render::effect::{EffectRenderer, load_instance};
use crate::render::lighting::{self, MapLighting};
use crate::render::terrain::{ATTRIBUTE_SHADOW_UV, TerrainMaterial, ground, masks, patch_uniform, set_fog, set_light, shadow_image};
use crate::render::tree::Tree;
use crate::render::{cloud, environment_fog, model, sky, sun};

/// The water texture animation: 30 frames, one every 70 ms.
const WATER_FRAMES: usize = 30;
const WATER_FRAME_SECONDS: f32 = 0.07;

// A broad phase only: exact tests below still determine collision and height.
const GRID_CELL: f32 = 1024.0;
/// `TerrainMaterial`'s texture-array bound (`MAX_LAYERS` in `terrain.rs`).
const MAX_TERRAIN_LAYERS: usize = 256;
const MAX_GRID_CELLS: i64 = 4096;

#[derive(Default)]
struct SpatialGrid {
    cells: HashMap<(i32, i32), Vec<usize>>,
    large: Vec<usize>,
    len: usize,
}

impl SpatialGrid {
    fn cell_range(min: Vec2, max: Vec2) -> Option<(i32, i32, i32, i32)> {
        if !min.is_finite() || !max.is_finite() { return None; }
        let (x0, y0) = ((min.x / GRID_CELL).floor() as i32, (min.y / GRID_CELL).floor() as i32);
        let (x1, y1) = ((max.x / GRID_CELL).floor() as i32, (max.y / GRID_CELL).floor() as i32);
        let width = i64::from(x1) - i64::from(x0) + 1;
        let height = i64::from(y1) - i64::from(y0) + 1;
        if width <= 0 || height <= 0 || width.saturating_mul(height) > MAX_GRID_CELLS { return None; }
        Some((x0, y0, x1, y1))
    }

    fn insert(&mut self, min: Vec2, max: Vec2) {
        let id = self.len;
        self.len += 1;
        if let Some((x0, y0, x1, y1)) = Self::cell_range(min, max) {
            for y in y0..=y1 { for x in x0..=x1 { self.cells.entry((x, y)).or_default().push(id); } }
        } else {
            self.large.push(id);
        }
    }

    fn candidates(&self, min: Vec2, max: Vec2) -> Vec<usize> {
        let Some((x0, y0, x1, y1)) = Self::cell_range(min, max) else { return (0..self.len).collect() };
        let mut result = self.large.clone();
        for y in y0..=y1 { for x in x0..=x1 {
            if let Some(ids) = self.cells.get(&(x, y)) { result.extend_from_slice(ids); }
        } }
        result.sort_unstable();
        result.dedup();
        result
    }
}

fn index_collision(grid: &mut SpatialGrid, shape: &StaticCollision) {
    let (center, radius) = shape.bounds();
    grid.insert(center.truncate() - radius, center.truncate() + radius);
}

/// A map ready to spawn.
pub struct PreparedMap {
    pub map: LoadedMap,
    /// The texture set as one 2D array ([`crate::render::terrain::ground`]).
    pub ground_image: Arc<Image>,
    /// The map's placed effects, loaded.
    pub effects: Vec<EffectInstance>,
    /// Effect and ground-texture errors (the load's own are in `map`).
    pub errors: Vec<String>,
    /// The buildings' collision shapes (`.mdatr`), Metin2 world space.
    pub collisions: Vec<StaticCollision>,
    /// Triangles from `.mdatr` height meshes, in the collision coordinate
    /// system (x, y on the ground plane; z up).
    pub walkable: Vec<[Vec3; 3]>,
    /// The `.mdatr` files with height data: their objects are walkable and
    /// receive the character shadow (`m_ShadowReceiverVector`).
    pub shadow_receivers: HashSet<Vec<u8>>,
    walkable_grid: SpatialGrid,
    collision_grid: SpatialGrid,
}

/// Loads and prepares map `name`. Blocking and slow (seconds): run it on
/// `AsyncComputeTaskPool`.
pub fn prepare_map(packs: &PackManager, name: &str) -> Result<PreparedMap, String> {
    let map = m2_world::load_map(packs, name)?;
    prepare_loaded_map(packs, map, None)
}

fn prepare_sector(packs: &PackManager, name: &str, x: u16, y: u16, ground_image: Option<Arc<Image>>) -> Result<PreparedMap, String> {
    let map = m2_world::load_map_sector(packs, name, x, y)?;
    prepare_loaded_map(packs, map, ground_image)
}

fn prepare_loaded_map(packs: &PackManager, map: LoadedMap, ground_image: Option<Arc<Image>>) -> Result<PreparedMap, String> {
    let mut errors = Vec::new();
    let mut effects = Vec::new();
    for object in &map.objects {
        if let PlacedKind::Effect { file, matrix, .. } = &object.kind {
            match load_instance(packs, file, *matrix) {
                Ok(effect) => effects.push(effect),
                Err(error) => errors.push(error),
            }
        }
    }
    let ground_image = ground_image.unwrap_or_else(|| {
        let (image, texture_errors) = ground(packs, &map.textures);
        errors.extend(texture_errors);
        Arc::new(image)
    });
    let (collisions, walkable, shadow_receivers) = building_collisions(packs, &map.objects, &mut errors);
    let mut collision_grid = SpatialGrid::default();
    for shape in &collisions { index_collision(&mut collision_grid, shape); }
    let mut walkable_grid = SpatialGrid::default();
    for triangle in &walkable {
        let [a, b, c] = triangle.map(|v| v.truncate());
        // Match triangle_height's small barycentric tolerance at edges.
        let pad = (b - a).abs().max((c - a).abs()) * 0.0002 + Vec2::splat(0.001);
        walkable_grid.insert(a.min(b).min(c) - pad, a.max(b).max(c) + pad);
    }
    Ok(PreparedMap { map, ground_image, effects, errors, collisions, walkable, shadow_receivers, walkable_grid, collision_grid })
}

/// `CArea::__UpdateAniThingList`'s `UpdateCollisionData` for buildings and
/// dungeon blocks: each `.mdatr` shape placed by the object's matrix.
fn building_collisions(
    packs: &PackManager,
    objects: &[PlacedObject],
    errors: &mut Vec<String>,
) -> (Vec<StaticCollision>, Vec<[Vec3; 3]>, HashSet<Vec<u8>>) {
    let mut files: HashMap<Vec<u8>, Option<m2_formats::attribute::AttributeData>> = HashMap::new();
    let mut out = Vec::new();
    let mut walkable = Vec::new();
    let mut receivers = HashSet::new();
    for object in objects {
        let (PlacedKind::Building { attribute_file, attribute_exists: true, .. }
        | PlacedKind::DungeonBlock { attribute_file, attribute_exists: true, .. }) = &object.kind
        else {
            continue;
        };
        let data = files.entry(attribute_file.clone()).or_insert_with(|| {
            let parsed = packs.get(attribute_file).map_err(|e| e.to_string()).and_then(|bytes| {
                m2_formats::attribute::parse_attribute_data(&bytes).map_err(|e| format!("{e:?}"))
            });
            match parsed {
                Ok(data) => Some(data),
                Err(error) => {
                    errors.push(format!("{}: {error}", String::from_utf8_lossy(attribute_file)));
                    None
                }
            }
        });
        let Some(data) = data else { continue };
        let [yaw, pitch, roll] = object.rotation.map(f32::to_radians);
        let mut m = rotation_yaw_pitch_roll(yaw, pitch, roll);
        (m[12], m[13], m[14]) = (object.position[0], object.position[1], object.position[2]);
        let transform = D3dTransform::from_row_major(&m);
        for c in &data.collisions {
            out.extend(StaticCollision::build(
                c.kind,
                Vec3::from_array(c.position),
                c.dimensions,
                c.rotation,
                &transform,
            ));
        }
        if !data.heights.is_empty() {
            receivers.insert(attribute_file.clone());
        }
        for height in &data.heights {
            // Height meshes are triangle lists (`THeightData` stores the
            // already tessellated vertices). Ignore malformed trailing
            // vertices and vertical/underside faces.
            for vertices in height.vertices.chunks_exact(3) {
                let triangle = std::array::from_fn(|i| transform.apply(Vec3::from_array(vertices[i])));
                if (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]).z > 0.001 {
                    walkable.push(triangle);
                }
            }
        }
    }
    (out, walkable, receivers)
}

/// Interpolates a triangle's Z at a point in its XY projection. The caller
/// only stores upward-facing walkable faces.
fn triangle_height(triangle: [Vec3; 3], point: Vec2) -> Option<f32> {
    let [a, b, c] = triangle;
    let ab = b.truncate() - a.truncate();
    let ac = c.truncate() - a.truncate();
    let ap = point - a.truncate();
    let denominator = ab.perp_dot(ac);
    if denominator.abs() < 1e-5 {
        return None;
    }
    let u = ap.perp_dot(ac) / denominator;
    let v = ab.perp_dot(ap) / denominator;
    (u >= -1e-4 && v >= -1e-4 && u + v <= 1.0001)
        .then_some(a.z + u * (b.z - a.z) + v * (c.z - a.z))
}

/// A map chosen by a global position: the client's `net.Warp`.
pub struct WarpedMap {
    /// The map at that position, ready to spawn.
    pub prepared: PreparedMap,
    /// The map directory name (`AtlasInfo.txt`).
    pub name: String,
    /// The map's origin in global coordinates (`CPythonBackground::m_dwBaseX/Y`).
    pub base_x: u32,
    pub base_y: u32,
    /// The position local to the map (`CPythonBackground::Warp`).
    pub local_x: i32,
    pub local_y: i32,
}

/// Loads the map that holds global position `(x, y)`
/// (`CPythonBackground::GlobalPositionToMapInfo`) and the position inside it.
/// Blocking and slow: run it on `AsyncComputeTaskPool`.
pub fn prepare_map_at(packs: &PackManager, locale_path: &str, x: i32, y: i32) -> Result<WarpedMap, String> {
    let atlas = m2_world::load_atlas(packs, locale_path);
    let entry =
        m2_world::map_at(&atlas, x, y).ok_or_else(|| format!("no map in AtlasInfo holds global position {x},{y}"))?;
    let name = entry.name_utf8(m2_formats::utf8::DEFAULT_NAME_CODE_PAGE).into_owned();
    let local_x = x.wrapping_sub(entry.base_x as i32);
    let local_y = y.wrapping_sub(entry.base_y as i32);
    let cell_x = (local_x.max(0) / m2_world::TERRAIN_WORLD) as u16;
    let cell_y = (local_y.max(0) / m2_world::TERRAIN_WORLD) as u16;
    let prepared = prepare_sector(packs, &name, cell_x, cell_y, None)?;
    Ok(WarpedMap {
        prepared,
        name,
        base_x: entry.base_x,
        base_y: entry.base_y,
        local_x,
        local_y,
    })
}

/// Counts of what [`MapScene::spawn`] built, for logs and inspectors.
#[derive(Default, Clone, Copy, Debug)]
pub struct SpawnStats {
    pub patches: usize,
    pub vertices: usize,
    pub triangles: usize,
}

enum ObjectAsset {
    Model(Handle<Gr2Model>),
    Tree(Handle<SptTree>),
}

/// One patch inside a merged terrain group. `base` is its vertex offset in the
/// group's shared vertex buffer.
struct TerrainPatchLod {
    center: Vec2,
    base: u32,
    level: usize,
}

/// Patches of one terrain cell that share a texture-layer set, merged into one
/// mesh and one material (one draw instead of one per patch).
struct TerrainGroup {
    mesh: Handle<Mesh>,
    patches: Vec<TerrainPatchLod>,
}

/// The merged terrain meshes of one terrain cell.
struct TerrainCellLod {
    groups: Vec<TerrainGroup>,
}

struct MapStream {
    packs: Arc<PackManager>,
    scenes: HashMap<(u16, u16), Box<MapScene>>,
    pending: HashMap<(u16, u16), Task<Result<PreparedMap, String>>>,
    failed: HashSet<(u16, u16)>,
}

/// A spawned map. Despawning [`MapScene::root`] removes all of it except the
/// sky, which lives on the camera ([`sky::clear_sky`]).
pub struct MapScene {
    pub root: Entity,
    pub map: Arc<PreparedMap>,
    pub fog: Option<Fog>,
    pub stats: SpawnStats,
    pub objects_loaded: usize,
    /// Objects whose asset failed to load.
    pub object_errors: Vec<String>,
    pub effects: EffectRenderer,
    pending_objects: Vec<(PlacedObject, ObjectAsset)>,
    trees: Vec<Tree>,
    /// The SpeedTrees' collision shapes, added as the trees spawn.
    tree_collisions: Vec<StaticCollision>,
    tree_collision_grid: SpatialGrid,
    water: Handle<FixedFunctionMaterial>,
    water_frames: Vec<Handle<Image>>,
    water_frame: Option<usize>,
    ground: Handle<Image>,
    terrain_cells: Vec<TerrainCellLod>,
    lod_indices: [Vec<u32>; 3],
    stream: Option<MapStream>,
}

impl MapScene {
    /// Spawns the terrain, water, sky and clouds now; buildings and trees are
    /// spawned by [`MapScene::update`] once their assets have loaded. With
    /// `fog`, the environment's fog applies to everything.
    pub fn spawn(world: &mut World, map: Arc<PreparedMap>, fog: bool) -> Self {
        let fog = map.map.environment.as_ref().filter(|_| fog).and_then(environment_fog);
        Self::spawn_with_fog(world, map, fog)
    }

    /// [`MapScene::spawn`] with an explicit fog (the game lightens the
    /// environment's own).
    pub fn spawn_with_fog(world: &mut World, map: Arc<PreparedMap>, fog: Option<Fog>) -> Self {
        Self::spawn_inner(world, map, fog, None, None)
    }

    fn spawn_inner(world: &mut World, map: Arc<PreparedMap>, fog: Option<Fog>, parent: Option<Entity>, ground: Option<Handle<Image>>) -> Self {
        if parent.is_none() {
            model::clear_static_model_cache(world);
        }
        let loaded = &map.map;
        let root = world.spawn((Name::new(loaded.name.clone()), Transform::IDENTITY, Visibility::Visible)).id();
        // `CMapManager::SetEnvironmentDataPtr` + `BeginEnvironment`: the map's
        // lights and material own the next frame's lighting. The Bevy lights
        // hang off the map root, so despawning the map removes them; the
        // resource is replaced by the next map (or by the neutral fallback
        // when there is no environment).
        if let Some(parent) = parent {
            world.entity_mut(root).insert(ChildOf(parent));
        } else {
            lighting::install(world, loaded.environment.as_ref(), root);
            if let Some(environment) = &loaded.environment {
                sky::install_sky(world, environment);
                cloud::spawn_cloud(world, environment, Some(root));
                sun::spawn_sun(world, environment, root);
            }
        }

        let water_frames: Vec<Handle<Image>> = (1..=WATER_FRAMES)
            .map(|frame| {
                world
                    .resource::<AssetServer>()
                    .load(paths::asset_source_uri(&format!("d:/ymir work/special/water/{frame:02}.dds")))
            })
            .collect();
        let water = FixedFunctionMaterial::opaque(water_frames[0].clone())
            .blend(Blend::ALPHA)
            .cull(CullMode::None)
            .depth(DepthState { write: false, ..Default::default() })
            .stage0(TextureStage {
                color_op: TextureOp::SelectArg1,
                color_arg1: TextureArg::TEXTURE,
                color_arg2: TextureArg::TEXTURE,
                alpha_op: TextureOp::SelectArg1,
                alpha_arg1: TextureArg::DIFFUSE,
                alpha_arg2: TextureArg::DIFFUSE,
            });
        let water = if let Some(fog) = fog { water.fog(fog) } else { water };
        let water = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(water);
        let ground = ground.unwrap_or_else(|| world.resource_mut::<Assets<Image>>().add((*map.ground_image).clone()));
        let raw_indices = build_index_buffers();
        let lod_indices = [strip_to_triangles(&raw_indices[0]), list_to_triangles(&raw_indices[1]), list_to_triangles(&raw_indices[2])];
        let (stats, terrain_cells) = spawn_terrain(world, root, loaded, &ground, &water, fog, &lod_indices);
        info!(
            "map {}: {} patch(es), {} vertices, {} triangles, texture set of {}",
            loaded.name,
            stats.patches,
            stats.vertices,
            stats.triangles,
            loaded.textures.len()
        );

        let mut pending_objects = Vec::new();
        for object in &loaded.objects {
            let server = world.resource::<AssetServer>();
            let asset = match &object.kind {
                PlacedKind::Building { file, .. } | PlacedKind::DungeonBlock { file, .. } => {
                    ObjectAsset::Model(server.load(paths::asset_source_uri(&String::from_utf8_lossy(file))))
                }
                PlacedKind::Tree { file } => {
                    ObjectAsset::Tree(server.load(paths::asset_source_uri(&String::from_utf8_lossy(file))))
                }
                PlacedKind::Effect { .. } | PlacedKind::Ambience { .. } => continue,
            };
            pending_objects.push((object.clone(), asset));
        }
        let mut effects = EffectRenderer::default();
        effects.instances = map.effects.clone();
        effects.fog = fog;
        effects.root = Some(root);
        MapScene {
            root,
            map,
            fog,
            stats,
            objects_loaded: 0,
            object_errors: Vec::new(),
            effects,
            pending_objects,
            trees: Vec::new(),
            tree_collisions: Vec::new(),
            tree_collision_grid: SpatialGrid::default(),
            water,
            water_frames,
            water_frame: None,
            ground,
            terrain_cells,
            lod_indices,
            stream: None,
        }
    }

    /// A player scene keeps only the player's terrain cell and its neighbours
    /// resident. The initial cell is already prepared by `prepare_map_at`.
    pub fn spawn_streamed(world: &mut World, mut prepared: PreparedMap, fog: Option<Fog>, packs: Arc<PackManager>) -> Self {
        let initial = prepared.map.terrains.first().map(|c| (c.x, c.y)).expect("prepared sector has terrain");
        let terrains = std::mem::take(&mut prepared.map.terrains);
        let objects = std::mem::take(&mut prepared.map.objects);
        let metadata = prepared.map.clone();
        prepared.map.terrains = terrains;
        prepared.map.objects = objects;
        let metadata = PreparedMap {
            map: metadata, ground_image: Arc::clone(&prepared.ground_image), effects: Vec::new(), errors: Vec::new(),
            collisions: Vec::new(), walkable: Vec::new(), shadow_receivers: HashSet::new(),
            walkable_grid: SpatialGrid::default(), collision_grid: SpatialGrid::default(),
        };
        let mut scene = Self::spawn_inner(world, Arc::new(metadata), fog, None, None);
        let child = Self::spawn_inner(world, Arc::new(prepared), fog, Some(scene.root), Some(scene.ground.clone()));
        let mut scenes = HashMap::new();
        scenes.insert(initial, Box::new(child));
        scene.stream = Some(MapStream { packs, scenes, pending: HashMap::new(), failed: HashSet::new() });
        scene
    }

    /// Objects still waiting for their asset.
    pub fn pending_objects(&self) -> usize {
        self.pending_objects.len() + self.stream.as_ref().map_or(0, |s| s.pending.len() + s.scenes.values().map(|s| s.pending_objects()).sum::<usize>())
    }

    /// The building and tree collision shapes whose bounds come within
    /// `radius` of `point` (Metin2 space) in the ground plane
    /// (`CCullingManager::ForInRange` around the actor's bounding sphere).
    pub fn collisions_near(&self, point: Vec3, radius: f32) -> Vec<&StaticCollision> {
        if let Some(stream) = &self.stream {
            let mut scenes: Vec<_> = stream.scenes.iter().collect();
            scenes.sort_unstable_by_key(|((x, y), _)| (*y, *x));
            return scenes.into_iter().flat_map(|(_, s)| s.collisions_near(point, radius)).collect();
        }
        let min = point.truncate() - radius;
        let max = point.truncate() + radius;
        self.map.collision_grid.candidates(min, max).into_iter().map(|i| &self.map.collisions[i])
            .chain(self.tree_collision_grid.candidates(min, max).into_iter().map(|i| &self.tree_collisions[i]))
            .filter(|shape| {
                let (centre, extent) = shape.bounds();
                (centre.truncate() - point.truncate()).length() <= extent + radius
            })
            .collect()
    }

    /// Terrain height plus the highest walkable object surface at map-local
    /// `(x, y)`. `.mdatr` height vertices use collision coordinates, where
    /// the ground-plane Y coordinate is `-y`.
    pub fn ground_height(&self, x: f32, y: f32) -> f32 {
        if let Some(stream) = &self.stream {
            let cell = sector_at(Vec2::new(x, y));
            let terrain = stream.scenes.get(&cell).map_or(0.0, |s| s.map.map.terrain_height(x, y));
            let point = Vec2::new(x, -y);
            return stream.scenes.values().fold(terrain, |height, scene| {
                scene.map.walkable_grid.candidates(point, point).into_iter()
                    .filter_map(|i| triangle_height(scene.map.walkable[i], point)).fold(height, f32::max)
            });
        }
        let terrain = self.map.map.terrain_height(x, y);
        let point = Vec2::new(x, -y);
        self.map.walkable_grid.candidates(point, point).into_iter()
            .filter_map(|i| triangle_height(self.map.walkable[i], point))
            .fold(terrain, f32::max)
    }

    pub fn contains_position(&self, position: Vec2) -> bool {
        self.stream.as_ref().is_none_or(|stream| stream.scenes.get(&sector_at(position)).is_some_and(|scene| scene.pending_objects.is_empty()))
    }

    pub fn is_attr_on(&self, position: Vec2, flag: u8) -> bool {
        if position.x < 0.0 || position.y < 0.0 { return false; }
        if let Some(stream) = &self.stream {
            return stream.scenes.get(&sector_at(position)).is_some_and(|s| s.is_attr_on(position, flag));
        }
        let x = position.x.floor() as i32;
        let y = position.y.floor() as i32;
        let size = m2_world::TERRAIN_WORLD;
        self.map.map.terrains.iter()
            .find(|terrain| (terrain.x, terrain.y) == ((x / size) as u16, (y / size) as u16))
            .is_some_and(|terrain| terrain.is_attr_on(((x % size) / 100) as u16, ((y % size) / 100) as u16, flag))
    }

    /// Per frame: spawns objects whose assets arrived, steps the effects,
    /// and advances the water animation (tree leaves face the camera on the GPU).
    pub fn update(&mut self, world: &mut World) {
        self.update_objects(world);
        self.effects.update(world);
        let frame = (world.resource::<Time>().elapsed_secs() / WATER_FRAME_SECONDS) as usize % self.water_frames.len();
        if self.water_frame != Some(frame) {
            if let Some(mut water) = world.resource_mut::<Assets<FixedFunctionMaterial>>().get_mut(&self.water) {
                water.textures[0] = Some(self.water_frames[frame].clone());
                self.water_frame = Some(frame);
            }
        }
    }

    /// Advances the streaming window around the player and updates its LODs.
    pub fn update_streamed(&mut self, world: &mut World, player: Vec2) {
        let Some(mut stream) = self.stream.take() else { self.update(world); return };
        let wanted = surrounding_sectors(sector_at(player), self.map.map.terrain_count);
        let removed: Vec<_> = stream.scenes.keys().copied().filter(|c| !wanted.contains(c)).collect();
        for cell in removed {
            if let Some(scene) = stream.scenes.remove(&cell) { world.despawn(scene.root); }
        }
        stream.pending.retain(|cell, _| wanted.contains(cell));
        stream.failed.retain(|cell| wanted.contains(cell));
        for cell in &wanted {
            if stream.scenes.contains_key(cell) || stream.pending.contains_key(cell) || stream.failed.contains(cell) { continue; }
            let packs = Arc::clone(&stream.packs);
            let name = self.map.map.name.clone();
            let image = Arc::clone(&self.map.ground_image);
            let (x, y) = *cell;
            stream.pending.insert(*cell, AsyncComputeTaskPool::get().spawn(async move {
                prepare_sector(&packs, &name, x, y, Some(image))
            }));
        }
        // Spawning a terrain cell creates many meshes. Spread completed work
        // over frames instead of stalling on every finished task at once.
        let ready: Vec<_> = stream.pending.iter_mut().filter_map(|(cell, task)| check_ready(task).map(|result| (*cell, result))).take(1).collect();
        for (cell, result) in ready {
            stream.pending.remove(&cell);
            match result {
                Ok(map) => {
                    let scene = Self::spawn_inner(world, Arc::new(map), self.fog, Some(self.root), Some(self.ground.clone()));
                    stream.scenes.insert(cell, Box::new(scene));
                }
                Err(error) => { self.object_errors.push(format!("Sector {},{}: {error}", cell.0, cell.1)); stream.failed.insert(cell); }
            }
        }
        for scene in stream.scenes.values_mut() {
            scene.update_lod(world, player);
            scene.update(world);
        }
        self.stream = Some(stream);
        self.update(world);
    }

    fn update_lod(&mut self, world: &mut World, player: Vec2) {
        let MapScene { terrain_cells, lod_indices, map, .. } = self;
        for cell in terrain_cells {
            for group in &mut cell.groups {
                let mut changed = false;
                for patch in &mut group.patches {
                    let level = terrain_lod_step(patch.level, patch.center.distance(player), map.map.view_radius);
                    if level != patch.level {
                        patch.level = level;
                        changed = true;
                    }
                }
                if !changed {
                    continue;
                }
                let mut merged = Vec::new();
                for patch in &group.patches {
                    merged.extend(lod_indices[patch.level].iter().map(|i| i + patch.base));
                }
                if let Some(mut mesh) = world.resource_mut::<Assets<Mesh>>().get_mut(&group.mesh) {
                    mesh.insert_indices(Indices::U32(merged));
                }
            }
        }
    }

    fn update_objects(&mut self, world: &mut World) {
        let mut pending = Vec::new();
        for (object, asset) in self.pending_objects.drain(..) {
            let transform = object_transform(&object);
            let receiver = matches!(
                &object.kind,
                PlacedKind::Building { attribute_file, .. } | PlacedKind::DungeonBlock { attribute_file, .. }
                    if self.map.shadow_receivers.contains(attribute_file)
            );
            let (ready, id): (bool, UntypedAssetId) = match &asset {
                // Repeated buildings reuse the baked meshes without cloning the model.
                ObjectAsset::Model(handle)
                    if model::spawn_cached_static_model(world, handle.id(), transform, Some(self.root), receiver).is_some() =>
                {
                    (true, handle.id().untyped())
                }
                ObjectAsset::Model(handle) => {
                    let model = world.resource::<Assets<Gr2Model>>().get(handle).cloned();
                    let ready = model.as_ref().is_some_and(|model| model::static_model_ready(world, model));
                    if ready && let Some(model) = &model {
                        model::spawn_static_model(world, handle.id(), model, transform, Some(self.root), receiver, &[], self.fog);
                    }
                    (ready, handle.id().untyped())
                }
                ObjectAsset::Tree(handle) => {
                    let tree = world.resource::<Assets<SptTree>>().get(handle).cloned();
                    let ready = tree.is_some();
                    if let Some(tree) = tree {
                        // `CSpeedTreeWrapper::OnUpdateCollisionData`: spheres
                        // and cylinders at the tree's position (boxes skipped).
                        let at = D3dTransform {
                            rotation: bevy::math::Mat3::IDENTITY,
                            translation: Vec3::from_array(object.position),
                        };
                        for c in &tree.collisions {
                            let position = Vec3::from_array(coords::vec3_back(c.position));
                            let d = coords::vec3_back(c.dimensions);
                            let kind = match c.kind {
                                0 => battle::COLLISION_SPHERE,
                                1 => battle::COLLISION_CYLINDER,
                                _ => continue,
                            };
                            if let Some(shape) = StaticCollision::build(kind, position, d, [0.0, 0.0, 0.0, 1.0], &at) {
                                index_collision(&mut self.tree_collision_grid, &shape);
                                self.tree_collisions.push(shape);
                            }
                        }
                        self.trees.push(Tree::spawn(world, tree, transform, Some(self.root), self.fog));
                    }
                    (ready, handle.id().untyped())
                }
            };
            if ready {
                self.objects_loaded += 1;
            } else if let Some(LoadState::Failed(error)) = world.resource::<AssetServer>().get_load_state(id) {
                self.object_errors.push(format!("Object {}: {error}", object.crc));
            } else {
                pending.push((object, asset));
            }
        }
        self.pending_objects = pending;
    }
}

/// Merges the terrain patches of one cell that share a texture-layer set into
/// one mesh and one material. A cell is a 64-patch 8×8 grid; as separate
/// entities those were 64 draws (and 64 bind groups) per cell, which dominated
/// the frame at the game's 3×3 streaming window. The per-patch LOD index
/// buffers are kept and concatenated whenever a patch changes level.
fn spawn_terrain(
    world: &mut World,
    root: Entity,
    map: &LoadedMap,
    ground: &Handle<Image>,
    water: &Handle<FixedFunctionMaterial>,
    fog: Option<Fog>,
    lod_indices: &[Vec<u32>; 3],
) -> (SpawnStats, Vec<TerrainCellLod>) {
    let splat_alpha = splat_alpha_matrix();
    // `CMapManager::BeginEnvironment` sets the background light for the map
    // pass; the terrain bakes it into its vertex lighting.
    let map_lighting = *world.resource::<MapLighting>();
    let character_shadow = crate::render::shadow::character_shadow(world);
    let mut stats = SpawnStats::default();
    let mut cells = Vec::new();
    for cell in &map.terrains {
        let masks = world.resource_mut::<Assets<Image>>().add(masks(&cell.terrain, map.textures.len()));
        let shadow = world.resource_mut::<Assets<Image>>().add(shadow_image(&cell.terrain));
        let patches = build_terrain_patches(&cell.terrain, cell.x, cell.y, DEFAULT_OPAQUE_WATER_DEPTH);
        // Patches that sample the same layers can share a material.
        struct Builder {
            key: Vec<u16>,
            positions: Vec<[f32; 3]>,
            normals: Vec<[f32; 3]>,
            tile: Vec<[f32; 2]>,
            alpha: Vec<[f32; 2]>,
            shadow: Vec<[f32; 2]>,
            patches: Vec<TerrainPatchLod>,
        }
        let mut builders: Vec<Builder> = Vec::new();
        let mut water_positions: Vec<[f32; 3]> = Vec::new();
        let mut water_colors: Vec<[f32; 4]> = Vec::new();
        let mut water_uvs: Vec<[f32; 2]> = Vec::new();
        let mut water_indices: Vec<u32> = Vec::new();
        for (index, patch) in patches.iter().enumerate() {
            if !patch.water_vertices.is_empty() {
                let base = water_positions.len() as u32;
                water_positions.extend(patch.water_vertices.iter().map(|v| coords::vec3(v.position).to_array()));
                water_colors.extend(patch.water_vertices.iter().map(|v| m2_render::d3d_color(v.diffuse).to_array()));
                water_uvs.extend(patch.water_vertices.iter().map(|v| [v.position[0] / 800.0, -v.position[1] / 800.0]));
                let mut indices: Vec<u32> = (0..patch.water_vertices.len() as u32).collect();
                coords::flip_winding(&mut indices);
                water_indices.extend(indices.into_iter().map(|index| index + base));
            }
            // Layer 0 is the always-present base; the rest are this patch's
            // non-zero tile counts (the same rule as `patch_uniform`).
            let counts = &cell.terrain.patch_tile_count[index];
            let key: Vec<u16> = (0..map.textures.len().min(MAX_TERRAIN_LAYERS))
                .filter(|&id| {
                    id == 0 || (counts.get(id).copied().unwrap_or(0) > 0 && !map.textures[id].file_name.is_empty())
                })
                .map(|id| id as u16)
                .collect();
            let builder = match builders.iter_mut().position(|b| b.key == key) {
                Some(position) => &mut builders[position],
                None => {
                    builders.push(Builder {
                        key,
                        positions: Vec::new(),
                        normals: Vec::new(),
                        tile: Vec::new(),
                        alpha: Vec::new(),
                        shadow: Vec::new(),
                        patches: Vec::new(),
                    });
                    builders.last_mut().unwrap()
                }
            };
            let base = builder.positions.len() as u32;
            builder.positions.extend(patch.vertices.iter().map(|v| coords::vec3(v.position).to_array()));
            builder.normals.extend(patch.vertices.iter().map(|v| coords::vec3(v.normal).to_array()));
            let uvs = m2_world::lighting::vertex_uvs(
                &patch.vertices,
                u32::from(cell.x),
                u32::from(cell.y),
                TERRAIN_TEX_COORD_BASE,
                &splat_alpha,
            );
            builder.tile.extend(uvs.iter().map(|uv| uv.tile));
            builder.alpha.extend(uvs.iter().map(|uv| uv.alpha));
            builder.shadow.extend(uvs.iter().map(|uv| uv.static_shadow));
            builder.patches.push(TerrainPatchLod {
                center: Vec2::new((patch.min_x + patch.max_x) * 0.5, (patch.min_y + patch.max_y) * 0.5),
                base,
                level: 0,
            });
        }

        if !water_indices.is_empty() {
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, water_positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, water_colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, water_uvs);
            mesh.insert_indices(Indices::U32(water_indices));
            let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
            world.spawn((
                Name::new("Water"),
                Mesh3d(mesh),
                MeshMaterial3d(water.clone()),
                Transform::IDENTITY,
                Visibility::Visible,
                ChildOf(root),
            ));
        }

        let mut groups = Vec::new();
        for builder in builders {
            let patch_count = builder.patches.len();
            let vertex_count = builder.positions.len();
            let mut counts = [0u32; 256];
            for &id in &builder.key {
                counts[id as usize] = 1;
            }
            let mut params = patch_uniform(&map.textures, &counts);
            set_fog(&mut params, fog);
            set_light(&mut params, &map_lighting.background);
            let material = world.resource_mut::<Assets<TerrainMaterial>>().add(TerrainMaterial {
                params,
                ground: ground.clone(),
                masks: masks.clone(),
                shadow: shadow.clone(),
                character_shadow_view: character_shadow.view.clone(),
                character_shadow: character_shadow.texture.clone(),
            });
            let mut merged = Vec::new();
            for patch in &builder.patches {
                merged.extend(lod_indices[0].iter().map(|index| index + patch.base));
            }
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, builder.positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, builder.normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, builder.tile);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, builder.alpha);
            mesh.insert_attribute(ATTRIBUTE_SHADOW_UV, builder.shadow);
            mesh.insert_indices(Indices::U32(merged));
            let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);

            stats.patches += patch_count;
            stats.vertices += vertex_count;
            stats.triangles += patch_count * lod_indices[0].len() / 3;
            world.spawn((
                Name::new(format!("terrain group {:03}{:03}", cell.x, cell.y)),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                Visibility::Visible,
                ChildOf(root),
            ));
            groups.push(TerrainGroup { mesh, patches: builder.patches });
        }
        cells.push(TerrainCellLod { groups });
    }
    (stats, cells)
}

fn sector_at(position: Vec2) -> (u16, u16) {
    let size = m2_world::TERRAIN_WORLD as f32;
    ((position.x / size).floor().clamp(0.0, u16::MAX as f32) as u16,
     (position.y / size).floor().clamp(0.0, u16::MAX as f32) as u16)
}

fn surrounding_sectors(center: (u16, u16), count: (i16, i16)) -> HashSet<(u16, u16)> {
    let mut cells = HashSet::new();
    for dy in -1..=1 { for dx in -1..=1 {
        let x = i32::from(center.0) + dx;
        let y = i32::from(center.1) + dy;
        if x >= 0 && y >= 0 && x < i32::from(count.0) && y < i32::from(count.1) {
            cells.insert((x as u16, y as u16));
        }
    } }
    cells
}

fn list_to_triangles(indices: &[u16]) -> Vec<u32> {
    let mut out: Vec<_> = indices.iter().map(|&i| u32::from(i)).collect();
    coords::flip_winding(&mut out);
    out
}

fn terrain_lod_level(distance: f32, view_radius: i64) -> usize {
    let (near, far) = m2_world::mesh::lod_distances(view_radius);
    if distance >= far { 2 } else if distance >= near { 1 } else { 0 }
}

/// How far past a LOD boundary a patch must be before it switches, so walking
/// along a boundary does not rebuild its group's index buffer back and forth.
const LOD_HYSTERESIS: f32 = 400.0;

/// [`terrain_lod_level`] for a patch now at `current`: it changes only once the
/// distance is [`LOD_HYSTERESIS`] beyond the boundary it crossed.
fn terrain_lod_step(current: usize, distance: f32, view_radius: i64) -> usize {
    let level = terrain_lod_level(distance, view_radius);
    let back = if level > current { distance - LOD_HYSTERESIS } else { distance + LOD_HYSTERESIS };
    if level != current && terrain_lod_level(back, view_radius) == current { current } else { level }
}

/// The world transform of a placed object (`CArea`'s yaw/pitch/roll and
/// position, converted to Bevy axes).
pub fn object_transform(object: &PlacedObject) -> Transform {
    let [yaw, pitch, roll] = object.rotation.map(f32::to_radians);
    let m = rotation_yaw_pitch_roll(yaw, pitch, roll);
    let rows = std::array::from_fn(|r| std::array::from_fn(|c| m[r * 4 + c]));
    let mut transform = Transform::from_matrix(coords::mat4_from_d3d(rows));
    transform.translation = coords::vec3(object.position);
    transform
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spatial_grid_keeps_overlapping_items_once_in_original_order() {
        let mut grid = SpatialGrid::default();
        grid.insert(Vec2::new(-1100.0, -2.0), Vec2::new(1100.0, 2.0));
        grid.insert(Vec2::splat(5000.0), Vec2::splat(5001.0));
        // Very large objects go in the fallback list.
        grid.insert(Vec2::splat(-1_000_000.0), Vec2::splat(1_000_000.0));
        assert_eq!(grid.candidates(Vec2::splat(-1.0), Vec2::splat(1.0)), vec![0, 2]);
        assert_eq!(grid.candidates(Vec2::splat(5000.0), Vec2::splat(5000.0)), vec![1, 2]);
        assert_eq!(grid.candidates(Vec2::splat(-1_000_000.0), Vec2::splat(1_000_000.0)), vec![0, 1, 2]);
    }

    #[test]
    fn stream_window_moves_and_clips_at_map_edges() {
        assert_eq!(sector_at(Vec2::new(25_599.0, 25_600.0)), (0, 1));
        let corner = surrounding_sectors((0, 0), (4, 5));
        assert_eq!(corner.len(), 4);
        assert!(corner.contains(&(1, 1)));
        let interior = surrounding_sectors((1, 1), (4, 5));
        assert_eq!(interior.len(), 9);
        let next = surrounding_sectors((2, 1), (4, 5));
        assert_eq!(interior.difference(&next).count(), 3);
        assert_eq!(next.difference(&interior).count(), 3);
    }

    #[test]
    fn terrain_lod_indices_reduce_triangles_and_keep_winding() {
        let indices = build_index_buffers();
        let lod0 = strip_to_triangles(&indices[0]);
        let lod1 = list_to_triangles(&indices[1]);
        let lod2 = list_to_triangles(&indices[2]);
        assert!(lod0.len() > lod1.len() && lod1.len() > lod2.len());
        let mut winding = None;
        for lod in [&lod0, &lod1, &lod2] {
            assert_eq!(lod.len() % 3, 0);
            assert!(lod.iter().all(|i| *i < 17 * 17));
            let signs: Vec<_> = lod.chunks_exact(3).filter_map(|tri| {
                let point = |i: u32| (i as i32 % 17, i as i32 / 17);
                let (a, b, c) = (point(tri[0]), point(tri[1]), point(tri[2]));
                let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
                (cross != 0).then_some(cross.signum())
            }).collect();
            assert!(!signs.is_empty() && signs.iter().all(|sign| *sign == signs[0]));
            assert_eq!(*winding.get_or_insert(signs[0]), signs[0]);
        }
        assert_eq!(terrain_lod_level(12_799.0, 128), 0);
        assert_eq!(terrain_lod_level(12_800.0, 128), 1);
        assert_eq!(terrain_lod_level(19_200.0, 128), 2);
        // Hysteresis: a patch keeps its level until it is well past the boundary.
        assert_eq!(terrain_lod_step(0, 12_900.0, 128), 0);
        assert_eq!(terrain_lod_step(0, 13_300.0, 128), 1);
        assert_eq!(terrain_lod_step(1, 12_500.0, 128), 1);
        assert_eq!(terrain_lod_step(1, 12_300.0, 128), 0);
        assert_eq!(terrain_lod_step(0, 30_000.0, 128), 2);
    }

    #[test]
    fn collision_grid_matches_full_scan_across_cell_boundaries() {
        let shapes = [
            StaticCollision::Sphere { position: Vec3::new(-1024.0, 0.0, 0.0), radius: 200.0 },
            StaticCollision::Cylinder { position: Vec3::new(1020.0, 1100.0, 0.0), radius: 500.0, height: 50.0 },
            StaticCollision::Sphere { position: Vec3::new(4000.0, -1024.0, 0.0), radius: 80.0 },
        ];
        let mut grid = SpatialGrid::default();
        for shape in &shapes { index_collision(&mut grid, shape); }
        for x in (-5000..=5000).step_by(237) {
            for y in (-3000..=3000).step_by(283) {
                let point = Vec2::new(x as f32, y as f32);
                let radius = 175.0;
                let exact = |i: &usize| {
                    let (center, extent) = shapes[*i].bounds();
                    (center.truncate() - point).length() <= extent + radius
                };
                let expected: Vec<_> = (0..shapes.len()).filter(exact).collect();
                let actual: Vec<_> = grid.candidates(point - radius, point + radius).into_iter().filter(exact).collect();
                assert_eq!(actual, expected, "point {point:?}");
            }
        }
    }
    use m2_formats::property::PropertyManager;
    use m2_world::area::place_object;

    #[test]
    fn area_property_places_building_with_height_bias_and_rotation() {
        let property = m2_formats::property::Property::read_from_memory(
            b"property/house.prb",
            b"YPRT\n123\nPropertyName house\nPropertyType Building\nBuildingFile \"d:/ymir work/house.gr2\"\n",
        )
        .unwrap();
        let mut props = PropertyManager::default();
        props.by_crc.insert(123, property);
        let objects = m2_formats::map::parse_area_data(
            b"AreaDataFile\nObjectCount 1\nStart Object000\n100 -200 300\n123\n0.000000#0.000000#90.000000\n25\nEnd Object\n",
            |_| true,
        )
        .unwrap();
        let placed = place_object(&objects[0], &props, &|_| false).unwrap();
        let transform = object_transform(&placed);
        assert_eq!(transform.translation, Vec3::new(100.0, 325.0, -200.0));
        assert!((transform.rotation * Vec3::X - Vec3::Z).length() < 1e-5);
        assert!(matches!(placed.kind, PlacedKind::Building { ref file, .. } if file == b"d:/ymir work/house.gr2"));
    }

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
    #[ignore = "requires the bundled client packs under bin/"]
    fn bundled_a1_prepares_effects_and_ground() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin");
        let mut packs = PackManager::new(root);
        m2_formats::pack::load_index(&mut packs, "pack", "pack/Index.dev").unwrap();
        let prepared = prepare_map(&packs, "metin2_map_a1").unwrap();
        assert!(prepared.errors.is_empty(), "{:?}", prepared.errors);
        assert!(!prepared.effects.is_empty());
        assert_eq!(
            prepared.ground_image.texture_descriptor.size.depth_or_array_layers as usize,
            prepared.map.textures.len()
        );
    }

    #[test]
    #[ignore = "requires the bundled client packs under bin/"]
    fn bundled_position_finds_the_map_the_server_put_the_character_in() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin");
        let mut packs = PackManager::new(root);
        m2_formats::pack::load_index(&mut packs, "pack", "pack/Index.dev").unwrap();
        // The global position the local server sends for a new `admin` character.
        let warped = prepare_map_at(&packs, "locale/en", 66382, 164633).unwrap();
        assert_eq!(warped.name, "metin2_map_b1");
        assert_eq!((warped.local_x, warped.local_y), (66382, 62233));
        assert_eq!((warped.prepared.map.terrain_count), (4, 5));
        let height = warped.prepared.map.terrain_height(warped.local_x as f32, warped.local_y as f32);
        // World units are centimetres: the terrain must be inside the map's scale.
        assert!((0.0..100_000.0).contains(&height), "height {height} is not a sensible terrain height");
    }

    #[test]
    #[ignore = "requires the bundled client packs under bin/"]
    fn bundled_sector_matches_the_same_cell_in_a_full_map() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin");
        let mut packs = PackManager::new(root);
        m2_formats::pack::load_index(&mut packs, "pack", "pack/Index.dev").unwrap();
        let full = m2_world::load_map(&packs, "metin2_map_b1").unwrap();
        let sector = m2_world::load_map_sector(&packs, "metin2_map_b1", 2, 2).unwrap();
        assert_eq!(sector.terrains.len(), 1);
        let expected = full.terrains.iter().find(|cell| (cell.x, cell.y) == (2, 2)).unwrap();
        assert_eq!(sector.terrains[0].terrain.height_map, expected.terrain.height_map);
        assert!(sector.objects.iter().all(|object| full.objects.contains(object)));
        assert_eq!(sector.terrain_count, full.terrain_count);
        assert_eq!(sector.view_radius, full.view_radius);
    }

    #[test]
    #[ignore = "requires the bundled client packs under bin/"]
    fn bundled_stream_loads_and_unloads_sectors_when_player_moves() {
        use bevy::asset::{AssetApp, AssetPlugin};
        use m2_assets::M2AssetsPlugin;
        use std::time::{Duration, Instant};

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin");
        let mut packs = PackManager::new(root);
        m2_formats::pack::load_index(&mut packs, "pack", "pack/Index.dev").unwrap();
        let packs = Arc::new(packs);
        let prepared = prepare_sector(&packs, "metin2_map_b1", 0, 0, None).unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(M2AssetsPlugin::from_packs(Arc::clone(&packs)))
            .add_plugins(AssetPlugin::default())
            .init_asset::<FixedFunctionMaterial>()
            .init_asset::<TerrainMaterial>()
            .init_asset::<bevy::render::storage::ShaderBuffer>()
            .init_asset::<crate::render::shadow::ShadowReceiverMaterial>()
            .init_asset::<crate::render::cloud::CloudMaterial>()
            .init_resource::<MapLighting>();
        app.finish();
        app.cleanup();
        let mut scene = MapScene::spawn_streamed(app.world_mut(), prepared, None, packs);
        let mut previous_roots = HashMap::new();

        for (player, expected) in [
            (Vec2::new(1.0, 1.0), (0, 0)),
            (Vec2::new(25_601.0, 1.0), (1, 0)),
            (Vec2::new(76_801.0, 102_401.0), (3, 4)),
        ] {
            let wanted = surrounding_sectors(expected, scene.map.map.terrain_count);
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                scene.update_streamed(app.world_mut(), player);
                app.update();
                let stream = scene.stream.as_ref().unwrap();
                if stream.pending.is_empty() {
                    assert!(stream.failed.is_empty(), "failed sectors: {:?}", stream.failed);
                    assert_eq!(stream.scenes.keys().copied().collect::<HashSet<_>>(), wanted);
                    for (cell, root) in &previous_roots {
                        if !wanted.contains(cell) { assert!(app.world().get_entity(*root).is_err()); }
                    }
                    for sector in stream.scenes.values() {
                        for cell in &sector.terrain_cells {
                            for group in &cell.groups {
                                let expected: usize = group
                                    .patches
                                    .iter()
                                    .map(|patch| {
                                        let level = terrain_lod_level(
                                            patch.center.distance(player),
                                            sector.map.map.view_radius,
                                        );
                                        assert_eq!(patch.level, level);
                                        sector.lod_indices[level].len()
                                    })
                                    .sum();
                                let mesh = app.world().resource::<Assets<Mesh>>().get(&group.mesh).unwrap();
                                assert_eq!(mesh.indices().unwrap().len(), expected);
                            }
                        }
                    }
                    previous_roots = stream.scenes.iter().map(|(cell, sector)| (*cell, sector.root)).collect();
                    break;
                }
                assert!(Instant::now() < deadline, "stream timed out");
                std::thread::yield_now();
            }
        }
    }
}

