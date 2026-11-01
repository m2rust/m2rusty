//! Granny models with the client's material: the skeleton and meshes come
//! from [`m2_assets::gr2::spawn_gr2_model`], each part gets a
//! [`FixedFunctionMaterial`].

use bevy::asset::{AssetServer, Assets};
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::world::CommandQueue;
use bevy::image::Image;
use bevy::math::Vec4;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::{Commands, Entity, Handle, World};
use m2_assets::gr2::{Gr2Mesh, Gr2MeshPart, Gr2Model, Gr2Spawn, spawn_gr2_model};
use m2_assets::paths;
use m2_render::{Blend, CullMode, FixedFunctionMaterial, Fog, TextureArg, TextureOp, TextureStage};

/// The material of one mesh part: its diffuse texture (a neutral colour when it
/// has none), the opacity map as a second stage with alpha blending, no culling
/// for `2x`-prefixed (two-sided) meshes, and the map fog when given.
pub fn part_material(asset_server: &AssetServer, mesh: &Gr2Mesh, part: &Gr2MeshPart, fog: Option<Fog>) -> FixedFunctionMaterial {
    let load = |path: &str| asset_server.load::<Image>(paths::asset_source_uri(path));
    let diffuse: Option<Handle<Image>> = part.material.diffuse.as_deref().map(load);
    let opacity = part.material.opacity.as_deref().map(load);
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

/// Spawns `model` (root with an `AnimationPlayer`, bones, skinned meshes with
/// [`part_material`]s), under `parent` when given.
pub fn spawn_model(world: &mut World, model: &Gr2Model, parent: Option<Entity>, fog: Option<Fog>) -> Gr2Spawn {
    let materials: Vec<Handle<FixedFunctionMaterial>> = {
        let asset_server = world.resource::<AssetServer>().clone();
        let built: Vec<FixedFunctionMaterial> = model
            .meshes
            .iter()
            .flat_map(|mesh| mesh.parts.iter().map(move |part| (mesh, part)))
            .map(|(mesh, part)| part_material(&asset_server, mesh, part, fog))
            .collect();
        let mut assets = world.resource_mut::<Assets<FixedFunctionMaterial>>();
        built.into_iter().map(|m| assets.add(m)).collect()
    };
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    let mut next = materials.into_iter();
    let spawned = spawn_gr2_model(&mut commands, model, |entity, _, _| {
        if let Some(material) = next.next() {
            entity.insert(MeshMaterial3d(material));
        }
    });
    if let Some(parent) = parent {
        commands.entity(spawned.root).insert(ChildOf(parent));
    }
    queue.apply(world);
    spawned
}
