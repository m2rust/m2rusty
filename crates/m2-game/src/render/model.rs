//! Granny models with the client's material: the skeleton and meshes come
//! from [`m2_assets::gr2::spawn_gr2_model`], each part gets a
//! [`FixedFunctionMaterial`].
//!
//! Two things differ from drawing a `.gr2` as stored:
//! - **Skins** (`SourceSkin`/`TargetSkin` in a `.msm`): a part whose texture is
//!   `src` uses `dst` instead (`CGraphicThingInstance::SetMaterialImagePointer`).
//! - **Missing textures**: a texture that is not in the packs is left out, so the
//!   part draws untextured as in the C++ client. A Bevy material whose image
//!   failed to load is never drawn, so without this the whole part vanishes
//!   (e.g. `assassin.gr2` names `assassin_lord*.dds`, which the packs lack).

use std::collections::HashMap;

use bevy::animation::AnimatedBy;
use bevy::app::{AnimationSystems, App, Plugin, PostUpdate};
use bevy::asset::{AssetId, AssetServer, Assets, RenderAssetUsages};
use bevy::camera::primitives::{Aabb, Frustum, MeshAabb, Sphere};
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::world::CommandQueue;
use bevy::image::Image;
use bevy::log::warn;
use bevy::math::{Mat3, Mat4, Vec3, Vec4};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::{
    Commands, Component, Entity, GlobalTransform, Handle, IntoScheduleConfigs, Mesh3d, Name, Query, Resource, Transform, Visibility,
    With, World,
};
use m2_assets::M2Packs;
use m2_assets::gr2::{Gr2Mesh, Gr2MeshPart, Gr2Model, Gr2Skeleton, Gr2Spawn, spawn_gr2_meshes, spawn_gr2_model};
use m2_assets::paths;
use m2_assets::race::RaceSkin;
use m2_render::{Blend, CullMode, FixedFunctionMaterial, Fog, TextureArg, TextureOp, TextureStage};

use crate::SceneCamera;
use crate::render::lighting::MapLighting;

/// Resolves the texture paths of a model: skins first, then whether the file exists.
pub struct Textures<'a> {
    asset_server: AssetServer,
    packs: Option<M2Packs>,
    skins: &'a [RaceSkin],
}

impl<'a> Textures<'a> {
    pub fn new(world: &World, skins: &'a [RaceSkin]) -> Self {
        Textures { asset_server: world.resource::<AssetServer>().clone(), packs: world.get_resource::<M2Packs>().cloned(), skins }
    }

    /// The image for texture `path` of a part, or `None` when it is not in the packs.
    pub fn load(&self, path: &str) -> Option<Handle<Image>> {
        let path = skinned(self.skins, path);
        let exists = self.packs.as_ref().is_none_or(|p| p.packs.exists(paths::virtual_name(path)));
        if !exists {
            warn!("texture {path} is not in the packs; the part is drawn untextured");
            return None;
        }
        Some(self.asset_server.load(paths::asset_source_uri(path)))
    }
}

/// The texture a part uses after the skins: `dst` of the skin whose `src` names
/// `path`. The `.msm` names are matched as canonical paths (case and separators
/// do not matter).
pub fn skinned<'s>(skins: &'s [RaceSkin], path: &'s str) -> &'s str {
    let canonical = paths::canonical_asset_path(path);
    skins.iter().find(|s| paths::canonical_asset_path(&s.src) == canonical).map_or(path, |s| s.dst.as_str())
}

/// The material of one mesh part: its diffuse texture (a neutral colour when it
/// has none), the opacity map as a second stage with alpha blending, no culling
/// for `2x`-prefixed (two-sided) meshes, and the map fog when given.
pub fn part_material(textures: &Textures, mesh: &Gr2Mesh, part: &Gr2MeshPart, fog: Option<Fog>) -> FixedFunctionMaterial {
    let diffuse = part.material.diffuse.as_deref().and_then(|p| textures.load(p));
    let opacity = part.material.opacity.as_deref().and_then(|p| textures.load(p));
    let mut material = diffuse
        .map(FixedFunctionMaterial::opaque)
        .unwrap_or_else(|| FixedFunctionMaterial::untextured().diffuse(Vec4::new(0.82, 0.72, 0.58, 1.0)));
    if let Some(opacity) = opacity {
        let alpha_stage = TextureStage {
            color_op: TextureOp::SelectArg1,
            color_arg1: TextureArg::CURRENT,
            color_arg2: TextureArg::CURRENT,
            alpha_op: TextureOp::Modulate,
            alpha_arg1: TextureArg::TEXTURE,
            alpha_arg2: TextureArg::CURRENT,
        };
        material = material.stage1(opacity, alpha_stage, Default::default()).blend(Blend::ALPHA);
    }
    if mesh.two_sided {
        material = material.cull(CullMode::None);
    }
    if let Some(fog) = fog {
        material = material.fog(fog);
    }
    material
}

/// One material handle per mesh part, in `(mesh, part)` order. Actors are lit
/// by the environment's character light (`CPythonBackground::SetCharacterDirLight`
/// around the actor pass), map objects by the background light the map pass
/// keeps (`SetBackgroundDirLight`); the two share their direction and differ
/// in ambient. The environment material replaces the primary colour of
/// textured parts, as `STATEMANAGER.SetMaterial` does.
fn materials(
    world: &mut World,
    model: &Gr2Model,
    skins: &[RaceSkin],
    fog: Option<Fog>,
    background: bool,
) -> Vec<Handle<FixedFunctionMaterial>> {
    let lighting = world
        .get_resource::<MapLighting>()
        .map(|lighting| if background { lighting.background } else { lighting.character });
    let built: Vec<FixedFunctionMaterial> = {
        let textures = Textures::new(world, skins);
        model
            .meshes
            .iter()
            .flat_map(|mesh| mesh.parts.iter().map(move |part| (mesh, part)))
            .map(|(mesh, part)| {
                let mut material = part_material(&textures, mesh, part, fog);
                if let Some(lighting) = lighting {
                    if material.textures[0].is_some() {
                        material.diffuse = lighting.material_diffuse;
                    }
                    material = material.lit(lighting.fixed_function());
                }
                material
            })
            .collect()
    };
    built.into_iter().map(|material| super::cached_material(world, material)).collect()
}
/// Whether all of `model`'s mesh assets have loaded, so [`spawn_static_model`]
/// can bake them all at once instead of spawning a partial object.
pub fn static_model_ready(world: &World, model: &Gr2Model) -> bool {
    let meshes = world.resource::<Assets<Mesh>>();
    model.meshes.iter().flat_map(|mesh| &mesh.parts).all(|part| meshes.get(&part.mesh).is_some())
}

/// One material bucket of a baked static model.
#[derive(Default)]
struct StaticBucket {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// Rest-pose world matrix of every bone (model space; the model root is
/// identity). Parents are resolved recursively because a Granny file is not
/// guaranteed to list them before their children; a cycle falls back to its
/// local transform.
fn bone_world_matrices(skeleton: &Gr2Skeleton) -> Vec<Mat4> {
    let mut resolved: Vec<Option<Mat4>> = vec![None; skeleton.bones.len()];
    for start in 0..skeleton.bones.len() {
        let mut chain = Vec::new();
        let mut current = Some(start);
        while let Some(index) = current {
            if resolved[index].is_some() || chain.contains(&index) {
                break;
            }
            chain.push(index);
            current = skeleton.bones[index].parent;
        }
        let mut parent = current.and_then(|index| resolved[index]).unwrap_or(Mat4::IDENTITY);
        for &index in chain.iter().rev() {
            parent *= skeleton.bones[index].local.to_matrix();
            resolved[index] = Some(parent);
        }
    }
    resolved.into_iter().map(|matrix| matrix.unwrap_or(Mat4::IDENTITY)).collect()
}

/// Skinning matrix of one vertex of a static model.
fn rest_skin_matrix(
    joints: Option<&[[u16; 4]]>,
    weights: Option<&[[f32; 4]]>,
    index: usize,
    bone_world: &[Mat4],
    inverse_bind: &[Mat4],
) -> Mat4 {
    let (Some(joints), Some(weights)) = (joints, weights) else { return Mat4::IDENTITY };
    let (Some(joints), Some(weights)) = (joints.get(index), weights.get(index)) else { return Mat4::IDENTITY };
    let mut skin = Mat4::ZERO;
    for k in 0..4 {
        let weight = weights[k];
        let joint = usize::from(joints[k]);
        if weight == 0.0 || joint >= bone_world.len() || joint >= inverse_bind.len() {
            continue;
        }
        skin += (bone_world[joint] * inverse_bind[joint]) * weight;
    }
    if skin == Mat4::ZERO { Mat4::IDENTITY } else { skin }
}

/// One baked material bucket of a static model, in model space.
#[derive(Clone)]
struct StaticPart {
    name: Name,
    material: Handle<FixedFunctionMaterial>,
    /// Alpha blended: never a shadow receiver (the overlay would tint its gaps).
    blended: bool,
    mesh: Handle<Mesh>,
    aabb: Aabb,
}

/// Baked static models by asset id. Every instance of the same model shares
/// the baked meshes (and therefore Bevy's automatic batching) while carrying
/// its own placement transform.
#[derive(Resource, Default)]
pub struct StaticModelCache {
    models: HashMap<AssetId<Gr2Model>, Vec<StaticPart>>,
}

/// Drops baked geometry. Material handles embed the map's fog and lighting, so
/// a new map must not reuse the previous map's materials.
pub fn clear_static_model_cache(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<StaticModelCache>() {
        cache.models.clear();
    }
}

/// Bakes `model`'s rest pose into plain meshes, one per material, and stores
/// them in the cache. See [`spawn_static_model`].
fn bake_static_model(world: &mut World, model: &Gr2Model, skins: &[RaceSkin], fog: Option<Fog>) -> Vec<StaticPart> {
    let materials = materials(world, model, skins, fog, true);
    let bone_world = model.skeleton.as_ref().map(bone_world_matrices).unwrap_or_default();
    let inverse_bind: Vec<Mat4> =
        model.skeleton.as_ref().map(|skeleton| skeleton.bones.iter().map(|bone| bone.inverse_world).collect()).unwrap_or_default();
    let mut buckets: Vec<(Handle<FixedFunctionMaterial>, StaticBucket)> = Vec::new();

    {
        let meshes = world.resource::<Assets<Mesh>>();
        for (part, material) in model.meshes.iter().flat_map(|mesh| &mesh.parts).zip(materials) {
            let Some(mesh) = meshes.get(&part.mesh) else { continue };
            let positions: &[[f32; 3]] = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                Some(VertexAttributeValues::Float32x3(values)) => values,
                _ => continue,
            };
            let normals: &[[f32; 3]] = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
                Some(VertexAttributeValues::Float32x3(values)) => values,
                _ => &[],
            };
            let uvs: &[[f32; 2]] = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
                Some(VertexAttributeValues::Float32x2(values)) => values,
                _ => &[],
            };
            let joints: Option<&[[u16; 4]]> = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX) {
                Some(VertexAttributeValues::Uint16x4(values)) => Some(values),
                _ => None,
            };
            let weights: Option<&[[f32; 4]]> = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT) {
                Some(VertexAttributeValues::Float32x4(values)) => Some(values),
                _ => None,
            };
            let Some(indices) = mesh.indices() else { continue };

            let position = buckets.iter().position(|(handle, _)| *handle == material);
            let slot = position.unwrap_or_else(|| {
                buckets.push((material, StaticBucket::default()));
                buckets.len() - 1
            });
            let bucket = &mut buckets[slot].1;
            let base = bucket.positions.len() as u32;
            for (index, position) in positions.iter().enumerate() {
                let skin = rest_skin_matrix(joints, weights, index, &bone_world, &inverse_bind);
                let local = Vec3::from_array(*position);
                let normal = normals.get(index).map_or(Vec3::Y, |normal| Vec3::from_array(*normal));
                let (local, normal) = if skin == Mat4::IDENTITY {
                    (local, normal)
                } else {
                    let matrix = Mat3::from_mat4(skin);
                    let normal_matrix = if matrix.determinant().abs() > 1e-12 { matrix.inverse().transpose() } else { matrix };
                    (skin.transform_point3(local), normal_matrix * normal)
                };
                bucket.positions.push(local.to_array());
                bucket.normals.push(normal.try_normalize().unwrap_or(Vec3::Y).to_array());
                bucket.uvs.push(uvs.get(index).copied().unwrap_or([0.0, 0.0]));
            }
            bucket.indices.extend(indices.iter().map(|index| index as u32 + base));
        }
    }

    let mut parts = Vec::new();
    for (index, (material, bucket)) in buckets.into_iter().enumerate() {
        if bucket.positions.is_empty() || bucket.indices.is_empty() {
            continue;
        }
        let (min, max) = bucket.positions.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), position| {
                let position = Vec3::from_array(*position);
                (min.min(position), max.max(position))
            },
        );
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, bucket.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, bucket.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, bucket.uvs);
        mesh.insert_indices(Indices::U32(bucket.indices));
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        let blended = world.resource::<Assets<FixedFunctionMaterial>>().get(&material).is_some_and(|m| m.blend.is_some());
        parts.push(StaticPart {
            name: Name::new(format!("{} static #{index}", model.name)),
            blended,
            material,
            mesh,
            aabb: Aabb::from_min_max(min, max),
        });
    }
    parts
}

/// Spawns a static map object: the rest pose is baked into plain, unskinned
/// meshes, one per material, so the object keeps no bones, no skeleton and no
/// `AnimationPlayer`. Map buildings never play `.msa` motions, but the normal
/// [`spawn_model`] path pays for a full GPU-skinned skeleton for every one;
/// most of a map scene's entities are those bones.
///
/// The baked mesh is cached per model asset, so repeated buildings share one
/// mesh and one material and are batched by Bevy. `transform` is the object's
/// placement in map space.
pub fn spawn_static_model(
    world: &mut World,
    model_id: AssetId<Gr2Model>,
    model: &Gr2Model,
    transform: Transform,
    parent: Option<Entity>,
    receiver: bool,
    skins: &[RaceSkin],
    fog: Option<Fog>,
) -> Vec<Entity> {
    if let Some(entities) = spawn_cached_static_model(world, model_id, transform, parent, receiver) {
        return entities;
    }
    let parts = bake_static_model(world, model, skins, fog);
    world.get_resource_or_init::<StaticModelCache>().models.insert(model_id, parts);
    spawn_cached_static_model(world, model_id, transform, parent, receiver).unwrap_or_default()
}

/// Spawns another instance of a model [`spawn_static_model`] already baked,
/// without needing the model itself. `None` when it was not baked yet. A
/// `receiver` (a walkable object: bridge, platform) also gets the character
/// shadow drawn over its opaque parts ([`super::shadow::ShadowReceiverMaterial`]).
pub fn spawn_cached_static_model(
    world: &mut World,
    model_id: AssetId<Gr2Model>,
    transform: Transform,
    parent: Option<Entity>,
    receiver: bool,
) -> Option<Vec<Entity>> {
    let parts = world.get_resource::<StaticModelCache>()?.models.get(&model_id)?.clone();
    let receiver_material = receiver.then(|| super::shadow::character_shadow(world).receiver);
    let mut entities = Vec::with_capacity(parts.len());
    for part in parts {
        let mut entity = world.spawn((
            part.name,
            Mesh3d(part.mesh.clone()),
            MeshMaterial3d(part.material),
            transform,
            Visibility::Visible,
            part.aabb,
        ));
        if let Some(parent) = parent {
            entity.insert(ChildOf(parent));
        }
        entities.push(entity.id());
        if let (Some(material), false) = (&receiver_material, part.blended) {
            let mut overlay = world.spawn((
                Name::new("shadow receiver"),
                Mesh3d(part.mesh),
                MeshMaterial3d(material.clone()),
                transform,
                Visibility::Visible,
                part.aabb,
            ));
            if let Some(parent) = parent {
                overlay.insert(ChildOf(parent));
            }
            entities.push(overlay.id());
        }
    }
    Some(entities)
}

/// Frustum culling of an animated model (characters, mounts). Skinned meshes
/// carry `NoFrustumCulling` because a motion can leave their rest bounds, so
/// without this every actor in the streamed sectors is animated, propagated,
/// skinned and drawn even behind the camera. The model root holds a sphere
/// around its rest pose, inflated for motions; outside the scene camera's
/// frustum the root is hidden and its bones leave the `AnimatedBy` set, so
/// Bevy neither evaluates nor writes their curves. The `AnimationPlayer` keeps
/// advancing, so motion timing and `motion_finished` are unaffected.
#[derive(Component)]
pub struct AnimationCull {
    /// Sphere centre in the root's space.
    center: Vec3,
    radius: f32,
    bones: Vec<Entity>,
    culled: bool,
}

/// How much bigger than the rest-pose bounds a motion may reach (relative,
/// then absolute in world units): attack lunges, raised weapons, mount rears.
const CULL_SCALE: f32 = 1.5;
const CULL_MARGIN: f32 = 100.0;

impl AnimationCull {
    /// The cull sphere of `model` spawned as `spawned`, from its meshes' rest
    /// bounds. `None` without bones or loaded meshes.
    fn new(world: &World, model: &Gr2Model, spawned: &Gr2Spawn) -> Option<Self> {
        if spawned.bones.is_empty() {
            return None;
        }
        let meshes = world.resource::<Assets<Mesh>>();
        let (min, max) = model
            .meshes
            .iter()
            .flat_map(|mesh| &mesh.parts)
            .filter_map(|part| meshes.get(&part.mesh)?.get_aabb())
            .fold(None, |bounds: Option<(Vec3, Vec3)>, aabb| {
                let (min, max) = (Vec3::from(aabb.min()), Vec3::from(aabb.max()));
                Some(bounds.map_or((min, max), |(lo, hi)| (lo.min(min), hi.max(max))))
            })?;
        Some(AnimationCull {
            center: (min + max) / 2.0,
            radius: (max - min).length() / 2.0 * CULL_SCALE + CULL_MARGIN,
            bones: spawned.bones.clone(),
            culled: false,
        })
    }
}

/// Hides and stops evaluating the [`AnimationCull`] models outside the
/// [`SceneCamera`]'s frustum. It runs before animation with the previous
/// frame's frustum and transforms; the margin covers that frame of motion.
pub fn cull_animated_models(
    mut commands: Commands,
    cameras: Query<&Frustum, With<SceneCamera>>,
    mut models: Query<(Entity, &GlobalTransform, &mut AnimationCull, &mut Visibility)>,
) {
    let frustum = cameras.single().ok();
    for (root, transform, mut cull, mut visibility) in &mut models {
        let culled = frustum.is_some_and(|frustum| {
            let matrix = transform.affine().matrix3;
            let scale = matrix.x_axis.length().max(matrix.y_axis.length()).max(matrix.z_axis.length());
            let sphere = Sphere { center: transform.transform_point(cull.center).into(), radius: cull.radius * scale };
            !frustum.intersects_sphere(&sphere, true)
        });
        if culled == cull.culled {
            continue;
        }
        cull.culled = culled;
        *visibility = if culled { Visibility::Hidden } else { Visibility::Inherited };
        for &bone in &cull.bones {
            if culled {
                commands.entity(bone).try_remove::<AnimatedBy>();
            } else {
                commands.entity(bone).try_insert(AnimatedBy(root));
            }
        }
    }
}

/// Registers [`cull_animated_models`].
pub struct ModelCullPlugin;

impl Plugin for ModelCullPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, cull_animated_models.before(AnimationSystems));
    }
}

/// Actors draw on the main layer and cast the character shadow
/// ([`super::shadow`]).
fn actor_layers() -> RenderLayers {
    RenderLayers::from_layers(&[0, super::shadow::SHADOW_LAYER])
}

/// Spawns `model` (root with an `AnimationPlayer`, bones, skinned meshes with
/// [`part_material`]s), under `parent` when given, with `skins` applied.
pub fn spawn_model(world: &mut World, model: &Gr2Model, parent: Option<Entity>, skins: &[RaceSkin], fog: Option<Fog>) -> Gr2Spawn {
    let materials = materials(world, model, skins, fog, false);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    let mut next = materials.into_iter();
    let spawned = spawn_gr2_model(&mut commands, model, |entity, _, _| {
        if let Some(material) = next.next() {
            entity.insert((MeshMaterial3d(material), actor_layers()));
        }
    });
    if let Some(parent) = parent {
        commands.entity(spawned.root).insert(ChildOf(parent));
    }
    queue.apply(world);
    if let Some(cull) = AnimationCull::new(world, model, &spawned) {
        world.entity_mut(spawned.root).insert(cull);
    }
    spawned
}

/// Spawns the meshes of `model` (hair, armor parts) skinned to the bones of an
/// already spawned `body` (`CGraphicThingInstance::SetModelInstance` onto the main
/// part), with `skins` applied.
pub fn attach_model(world: &mut World, model: &Gr2Model, body: &Gr2Spawn, skins: &[RaceSkin], fog: Option<Fog>) -> Vec<Entity> {
    let materials = materials(world, model, skins, fog, false);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    let mut next = materials.into_iter();
    let parts = spawn_gr2_meshes(&mut commands, model, body.root, &body.bone_by_name, body.root, |entity, _, _| {
        if let Some(material) = next.next() {
            entity.insert((MeshMaterial3d(material), actor_layers()));
        }
    });
    queue.apply(world);
    parts
}

/// Spawns `model`'s meshes skinned to one bone of `body`, which is how
/// `CActorInstance::AttachWeapon` hangs a weapon on `PART_WEAPON` /
/// `PART_WEAPON_LEFT`: the weapon's own skeleton has no bones in the body, so
/// all of its joints collapse to `bone` and its inverse bind pose places it
/// relative to that hand. A model whose skeleton names a bone the body also has
/// keeps that bone (hair-like attachments).
pub fn attach_model_to_bone(
    world: &mut World,
    model: &Gr2Model,
    body: &Gr2Spawn,
    bone: &str,
    skins: &[RaceSkin],
    fog: Option<Fog>,
) -> Vec<Entity> {
    let lowered = bone.to_ascii_lowercase();
    let Some(&target) = body
        .bone_by_name
        .get(bone)
        .or_else(|| body.bone_by_name.iter().find(|(name, _)| name.to_ascii_lowercase() == lowered).map(|(_, e)| e))
    else {
        warn!("model {}: body has no attachment bone {bone}", model.name);
        return Vec::new();
    };
    let materials = materials(world, model, skins, fog, false);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    let mut next = materials.into_iter();
    let parts = spawn_gr2_meshes(&mut commands, model, body.root, &body.bone_by_name, target, |entity, _, _| {
        if let Some(material) = next.next() {
            entity.insert((MeshMaterial3d(material), actor_layers()));
        }
    });
    queue.apply(world);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skins_replace_by_canonical_name() {
        let skins = vec![RaceSkin {
            part: 0,
            src: "ymir work/pc2/assassin/assassin_novice_red.dds".into(),
            dst: "ymir work/pc2/assassin/assassin_novice_green.dds".into(),
        }];
        assert_eq!(skinned(&skins, "D:\\Ymir Work\\pc2\\Assassin\\Assassin_Novice_Red.DDS"), skins[0].dst);
        assert_eq!(skinned(&skins, "ymir work/pc2/assassin/other.dds"), "ymir work/pc2/assassin/other.dds");
    }
}
