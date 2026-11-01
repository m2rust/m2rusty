//! SpeedTree instances.
//!
//! The tree geometry comes pre-built from [`m2_assets::spt::SptTree`]: branch
//! and frond meshes with static vertex colours, plus one leaf-quad mesh that
//! has to be re-oriented towards the camera every time the camera moves, as
//! the original renderer did per frame. Each [`Tree`] owns its leaf mesh.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use m2_assets::paths;
use m2_assets::spt::{SptPart, SptTree};
use m2_render::{CullMode, FixedFunctionMaterial, Fog, MipFilter, SamplerState};

use crate::SceneCamera;

/// Leaf cards are alpha tested against `ALPHAREF 128`. Filtering across mip
/// levels averages the transparent gaps of the atlas into the leaf texels and
/// pulls their alpha below the reference, so a distant tree loses its whole
/// canopy. Sampling level 0 keeps the cutout intact; minification is still
/// linear.
fn leaf_sampler() -> SamplerState {
    SamplerState { mip: MipFilter::None, ..SamplerState::default() }
}

/// One spawned tree.
pub struct Tree {
    pub root: Entity,
    pub tree: SptTree,
    leaf_mesh: Option<Handle<Mesh>>,
    /// Camera position the leaf quads were last oriented towards.
    leaf_camera: Option<Vec3>,
}

impl Tree {
    /// Spawns `tree` at `transform` (under `parent` when given). The leaf mesh is
    /// copied, so instances of the same `.spt` do not fight over billboards.
    pub fn spawn(world: &mut World, mut tree: SptTree, transform: Transform, parent: Option<Entity>, fog: Option<Fog>) -> Self {
        if let Some(mesh) = tree.leaf_mesh.as_ref().and_then(|h| world.resource::<Assets<Mesh>>().get(h)).cloned() {
            tree.leaf_mesh = Some(world.resource_mut::<Assets<Mesh>>().add(mesh));
        }
        let root = world.spawn((Name::new("tree"), transform, Visibility::Visible)).id();
        if let Some(parent) = parent {
            world.entity_mut(root).insert(ChildOf(parent));
        }

        let composite = tree.textures.composite.as_deref().map(|p| load_texture(world, p));
        let bark = tree.textures.branch.as_deref().map(|p| load_texture(world, p));
        for (name, part, texture) in [("branches", &tree.branches, bark), ("fronds", &tree.fronds, composite.clone())] {
            if let Some(part) = part {
                spawn_part(world, root, name, part, texture, fog);
            }
        }

        if let Some(leaf_mesh) = &tree.leaf_mesh {
            let base = match composite {
                Some(texture) => FixedFunctionMaterial::opaque(texture).sampler(leaf_sampler()),
                // Vertex colours only, as when a tree ships no atlas.
                None => FixedFunctionMaterial::untextured(),
            };
            let base = if let Some(fog) = fog { base.fog(fog) } else { base };
            let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(alpha_tested(base, tree.leaf_alpha_test));
            world.spawn((
                Name::new("leaves"),
                Mesh3d(leaf_mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                Visibility::Visible,
                // The quads are rewritten to face the camera every frame, so
                // the AABB computed at spawn says nothing about where they
                // are now; culling them against it makes the canopy vanish.
                NoFrustumCulling,
                ChildOf(root),
            ));
        }
        let leaf_mesh = tree.leaf_mesh.clone();
        Tree { root, tree, leaf_mesh, leaf_camera: None }
    }

    /// Leaves are camera-facing quads, so their positions are rewritten
    /// whenever the [`SceneCamera`] moved since the last update.
    pub fn orient_leaves(&mut self, world: &mut World) {
        let Some(handle) = &self.leaf_mesh else { return };
        let Some(camera) =
            world.query_filtered::<&GlobalTransform, With<SceneCamera>>().single(world).ok().map(|t| t.translation())
        else {
            return;
        };
        if self.leaf_camera == Some(camera) {
            return;
        }
        // The root's own `Transform` (not its `GlobalTransform`, which is only
        // propagated after the spawn frame) under its parent's.
        let local = world.get::<Transform>(self.root).map_or(Mat4::IDENTITY, Transform::to_matrix);
        let parent = world
            .get::<ChildOf>(self.root)
            .and_then(|p| world.get::<GlobalTransform>(p.parent()))
            .map_or(Mat4::IDENTITY, GlobalTransform::to_matrix);
        let local_camera = (parent * local).inverse().transform_point3(camera);
        if let Some(mut mesh) = world.resource_mut::<Assets<Mesh>>().get_mut(handle) {
            self.tree.update_leaf_billboards(&mut mesh, Vec3::ZERO, local_camera);
            self.leaf_camera = Some(camera);
        }
    }
}

fn spawn_part(world: &mut World, root: Entity, name: &str, part: &SptPart, texture: Option<Handle<Image>>, fog: Option<Fog>) {
    let base = match texture {
        Some(texture) => FixedFunctionMaterial::opaque(texture),
        None => FixedFunctionMaterial::untextured().diffuse(Vec4::new(0.42, 0.36, 0.24, 1.0)),
    };
    let base = if let Some(fog) = fog { base.fog(fog) } else { base };
    let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(alpha_tested(base, part.alpha_test));
    world.spawn((
        Name::new(name.to_owned()),
        Mesh3d(part.mesh.clone()),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Visibility::Visible,
        ChildOf(root),
    ));
}

/// SpeedTree materials never cull: branch cards and leaves are two-sided.
///
/// `reference` is `D3DRS_ALPHAREF` as the compat layer reports it, already in
/// 0..=255 (leaves default to 128); 0 means the part has no alpha test.
fn alpha_tested(material: FixedFunctionMaterial, reference: f32) -> FixedFunctionMaterial {
    let material = material.cull(CullMode::None);
    match alpha_reference(reference) {
        Some(reference) => material.alpha_tested(reference),
        None => material,
    }
}

/// `None` when the part has no alpha test.
fn alpha_reference(reference: f32) -> Option<u8> {
    (reference > 0.0).then(|| reference.round().clamp(0.0, 255.0) as u8)
}

fn load_texture(world: &World, path: &str) -> Handle<Image> {
    world.resource::<AssetServer>().load(paths::asset_source_uri(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_reference_is_a_d3d_alpharef_not_a_fraction() {
        // The compat layer reports D3DRS_ALPHAREF in 0..=255; leaves default
        // to 128. Scaling it like a 0..1 factor would cut the whole canopy.
        assert_eq!(alpha_reference(128.0), Some(128));
        assert_eq!(alpha_reference(1.0), Some(1));
        assert_eq!(alpha_reference(0.0), None);
        assert_eq!(alpha_reference(300.0), Some(255));
    }

    #[test]
    fn a_leaf_material_without_a_test_is_not_alpha_tested() {
        let plain = alpha_tested(FixedFunctionMaterial::untextured(), 0.0);
        assert!(plain.alpha_test.is_none());
        assert_eq!(plain.cull, CullMode::None);
        let cut = alpha_tested(FixedFunctionMaterial::untextured(), 128.0);
        assert_eq!(cut.alpha_test.map(|t| t.reference), Some(128));
    }
}
