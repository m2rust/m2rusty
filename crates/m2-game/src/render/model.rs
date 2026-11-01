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

use bevy::asset::{AssetServer, Assets};
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::world::CommandQueue;
use bevy::image::Image;
use bevy::log::warn;
use bevy::math::Vec4;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::{Commands, Entity, Handle, World};
use m2_assets::M2Packs;
use m2_assets::gr2::{Gr2Mesh, Gr2MeshPart, Gr2Model, Gr2Spawn, spawn_gr2_meshes, spawn_gr2_model};
use m2_assets::paths;
use m2_assets::race::RaceSkin;
use m2_render::{Blend, CullMode, FixedFunctionMaterial, Fog, TextureArg, TextureOp, TextureStage};

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

/// One material handle per mesh part, in `(mesh, part)` order.
fn materials(world: &mut World, model: &Gr2Model, skins: &[RaceSkin], fog: Option<Fog>) -> Vec<Handle<FixedFunctionMaterial>> {
    // `CPythonBackground::SetCharacterDirLight` puts the environment's
    // character light on actor passes. Every `.gr2` model goes through this
    // path (placed buildings too); a map's two lights share their direction
    // and differ only in ambient, so the character light is the close enough
    // and more important one. The environment material replaces the primary
    // colour of textured parts, as `STATEMANAGER.SetMaterial` does.
    let lighting = world.get_resource::<MapLighting>().map(|lighting| lighting.character);
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
    let mut assets = world.resource_mut::<Assets<FixedFunctionMaterial>>();
    built.into_iter().map(|m| assets.add(m)).collect()
}

/// Spawns `model` (root with an `AnimationPlayer`, bones, skinned meshes with
/// [`part_material`]s), under `parent` when given, with `skins` applied.
pub fn spawn_model(world: &mut World, model: &Gr2Model, parent: Option<Entity>, skins: &[RaceSkin], fog: Option<Fog>) -> Gr2Spawn {
    let materials = materials(world, model, skins, fog);
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

/// Spawns the meshes of `model` (hair, armor parts) skinned to the bones of an
/// already spawned `body` (`CGraphicThingInstance::SetModelInstance` onto the main
/// part), with `skins` applied.
pub fn attach_model(world: &mut World, model: &Gr2Model, body: &Gr2Spawn, skins: &[RaceSkin], fog: Option<Fog>) -> Vec<Entity> {
    let materials = materials(world, model, skins, fog);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    let mut next = materials.into_iter();
    let parts = spawn_gr2_meshes(&mut commands, model, body.root, &body.bone_by_name, body.root, |entity, _, _| {
        if let Some(material) = next.next() {
            entity.insert(MeshMaterial3d(material));
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
    let materials = materials(world, model, skins, fog);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    let mut next = materials.into_iter();
    let parts = spawn_gr2_meshes(&mut commands, model, body.root, &body.bone_by_name, target, |entity, _, _| {
        if let Some(material) = next.next() {
            entity.insert(MeshMaterial3d(material));
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
