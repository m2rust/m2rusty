//! An outdoor map on screen: [`prepare_map`] loads it with
//! [`m2_world::load_map`] and builds what is slow to build (the ground texture
//! array, the effect instances) off the main thread; [`MapScene::spawn`] puts
//! it in the world under one root, and [`MapScene::update`] runs it every frame
//! (objects as their assets arrive, effects, animated water, tree leaves).
//!
//! Terrain is LOD 0 everywhere, every terrain of the map is spawned and there
//! is no quadtree culling. The game's 3×3 streaming around the player and the
//! LOD 1/2 index buffers come with the game world.

use std::sync::Arc;

use bevy::asset::{AssetServer, Assets, LoadState, RenderAssetUsages, UntypedAssetId};
use bevy::ecs::hierarchy::ChildOf;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use m2_assets::{coords, gr2::Gr2Model, paths, spt::SptTree};
use m2_effect::EffectInstance;
use m2_formats::map::{TERRAIN_TEX_COORD_BASE, splat_alpha_matrix};
use m2_formats::pack::PackManager;
use m2_render::{Blend, CullMode, DepthState, FixedFunctionMaterial, Fog, TextureArg, TextureOp, TextureStage};
use m2_world::LoadedMap;
use m2_world::area::{PlacedKind, PlacedObject, rotation_yaw_pitch_roll};
use m2_world::mesh::{DEFAULT_OPAQUE_WATER_DEPTH, build_index_buffers, build_terrain_patches};

use crate::render::effect::{EffectRenderer, load_instance};
use crate::render::terrain::{TerrainMaterial, ground, masks, patch_uniform, set_fog};
use crate::render::tree::Tree;
use crate::render::{cloud, environment_fog, model, sky};

/// The water texture animation: 30 frames, one every 70 ms.
const WATER_FRAMES: usize = 30;
const WATER_FRAME_SECONDS: f32 = 0.07;

/// A map ready to spawn.
pub struct PreparedMap {
    pub map: LoadedMap,
    /// The texture set as one 2D array ([`crate::render::terrain::ground`]).
    pub ground_image: Image,
    /// The map's placed effects, loaded.
    pub effects: Vec<EffectInstance>,
    /// Effect and ground-texture errors (the load's own are in `map`).
    pub errors: Vec<String>,
}

/// Loads and prepares map `name`. Blocking and slow (seconds): run it on
/// `AsyncComputeTaskPool`.
pub fn prepare_map(packs: &PackManager, name: &str) -> Result<PreparedMap, String> {
    let map = m2_world::load_map(packs, name)?;
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
    let (ground_image, texture_errors) = ground(packs, &map.textures);
    errors.extend(texture_errors);
    Ok(PreparedMap { map, ground_image, effects, errors })
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
    water: Handle<FixedFunctionMaterial>,
    water_frames: Vec<Handle<Image>>,
}

impl MapScene {
    /// Spawns the terrain, water, sky and clouds now; buildings and trees are
    /// spawned by [`MapScene::update`] once their assets have loaded. With
    /// `fog`, the environment's fog applies to everything.
    pub fn spawn(world: &mut World, map: Arc<PreparedMap>, fog: bool) -> Self {
        let loaded = &map.map;
        let fog = loaded.environment.as_ref().filter(|_| fog).and_then(environment_fog);
        let root = world.spawn((Name::new(loaded.name.clone()), Transform::IDENTITY, Visibility::Visible)).id();
        if let Some(environment) = &loaded.environment {
            sky::install_sky(world, environment);
            cloud::spawn_cloud(world, environment, Some(root));
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
        let stats = spawn_terrain(world, root, loaded, &map.ground_image, &water, fog);
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
            water,
            water_frames,
        }
    }

    /// Objects still waiting for their asset.
    pub fn pending_objects(&self) -> usize {
        self.pending_objects.len()
    }

    /// Per frame: spawns objects whose assets arrived, steps the effects,
    /// advances the water animation and turns the tree leaves to the camera.
    pub fn update(&mut self, world: &mut World) {
        self.update_objects(world);
        self.effects.update(world);
        let frame = (world.resource::<Time>().elapsed_secs() / WATER_FRAME_SECONDS) as usize % self.water_frames.len();
        if let Some(mut water) = world.resource_mut::<Assets<FixedFunctionMaterial>>().get_mut(&self.water) {
            water.textures[0] = Some(self.water_frames[frame].clone());
        }
        for tree in &mut self.trees {
            tree.orient_leaves(world);
        }
    }

    fn update_objects(&mut self, world: &mut World) {
        let mut pending = Vec::new();
        for (object, asset) in self.pending_objects.drain(..) {
            let transform = object_transform(&object);
            let (ready, id): (bool, UntypedAssetId) = match &asset {
                ObjectAsset::Model(handle) => {
                    let model = world.resource::<Assets<Gr2Model>>().get(handle).cloned();
                    if let Some(model) = &model {
                        let spawned = model::spawn_model(world, model, Some(self.root), &[], self.fog);
                        world.entity_mut(spawned.root).insert(transform);
                    }
                    (model.is_some(), handle.id().untyped())
                }
                ObjectAsset::Tree(handle) => {
                    let tree = world.resource::<Assets<SptTree>>().get(handle).cloned();
                    let ready = tree.is_some();
                    if let Some(tree) = tree {
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

/// One mesh per terrain patch (and one per patch with water).
fn spawn_terrain(
    world: &mut World,
    root: Entity,
    map: &LoadedMap,
    ground_image: &Image,
    water: &Handle<FixedFunctionMaterial>,
    fog: Option<Fog>,
) -> SpawnStats {
    let splat_alpha = splat_alpha_matrix();
    let indices = strip_to_triangles(&build_index_buffers()[0]);
    let ground = world.resource_mut::<Assets<Image>>().add(ground_image.clone());
    let mut stats = SpawnStats::default();
    for cell in &map.terrains {
        let masks = world.resource_mut::<Assets<Image>>().add(masks(&cell.terrain, map.textures.len()));
        let patches = build_terrain_patches(&cell.terrain, cell.x, cell.y, DEFAULT_OPAQUE_WATER_DEPTH);
        for (index, patch) in patches.iter().enumerate() {
            if !patch.water_vertices.is_empty() {
                let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    patch.water_vertices.iter().map(|v| coords::vec3(v.position).to_array()).collect::<Vec<_>>(),
                );
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_COLOR,
                    patch.water_vertices.iter().map(|v| m2_render::d3d_color(v.diffuse).to_array()).collect::<Vec<_>>(),
                );
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    patch.water_vertices.iter().map(|v| [v.position[0] / 800.0, -v.position[1] / 800.0]).collect::<Vec<_>>(),
                );
                let mut water_indices: Vec<u32> = (0..patch.water_vertices.len() as u32).collect();
                coords::flip_winding(&mut water_indices);
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
            let positions: Vec<[f32; 3]> = patch.vertices.iter().map(|v| coords::vec3(v.position).to_array()).collect();
            let normals: Vec<[f32; 3]> = patch.vertices.iter().map(|v| coords::vec3(v.normal).to_array()).collect();
            let mut params = patch_uniform(&map.textures, &cell.terrain.patch_tile_count[index]);
            set_fog(&mut params, fog);
            let material = world.resource_mut::<Assets<TerrainMaterial>>().add(TerrainMaterial {
                params,
                ground: ground.clone(),
                masks: masks.clone(),
            });
            let uvs = m2_world::lighting::vertex_uvs(
                &patch.vertices,
                u32::from(cell.x),
                u32::from(cell.y),
                TERRAIN_TEX_COORD_BASE,
                &splat_alpha,
            );
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs.iter().map(|uv| uv.tile).collect::<Vec<_>>());
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs.iter().map(|uv| uv.alpha).collect::<Vec<_>>());
            mesh.insert_indices(Indices::U32(indices.clone()));
            let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);

            stats.patches += 1;
            stats.vertices += patch.vertices.len();
            stats.triangles += indices.len() / 3;
            world.spawn((
                Name::new(format!("patch {:03}{:03} #{}", cell.x, cell.y, index)),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                Visibility::Visible,
                ChildOf(root),
            ));
        }
    }
    stats
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
        assert_eq!(prepared.ground_image.texture_descriptor.size.depth_or_array_layers as usize, prepared.map.textures.len());
    }
}
