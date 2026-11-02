//! SpeedTree instances.
//!
//! The tree geometry comes pre-built from [`m2_assets::spt::SptTree`]: branch
//! and frond meshes with static vertex colours, plus leaf quads that face the
//! camera, as the original renderer re-oriented them every frame. The leaves
//! are turned on the GPU ([`m2_render::ATTRIBUTE_BILLBOARD`]), so every
//! instance of a `.spt` shares one leaf mesh (cached in [`LeafMeshCache`]) and
//! Bevy batches all trees of a species into one draw per part.

use std::collections::HashMap;

use bevy::camera::primitives::Aabb;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use m2_assets::paths;
use m2_assets::spt::{SptPart, SptTree};
use m2_render::{ATTRIBUTE_BILLBOARD, CullMode, FixedFunctionMaterial, Fog, MipFilter, SamplerState};

/// Leaf cards are alpha tested against `ALPHAREF 128`. Filtering across mip
/// levels averages the transparent gaps of the atlas into the leaf texels and
/// pulls their alpha below the reference, so a distant tree loses its whole
/// canopy. Sampling level 0 keeps the cutout intact; minification is still
/// linear.
fn leaf_sampler() -> SamplerState {
    SamplerState { mip: MipFilter::None, ..SamplerState::default() }
}

/// GPU-billboarded leaf meshes by the source `.spt` leaf mesh.
#[derive(Resource, Default)]
pub struct LeafMeshCache {
    meshes: HashMap<AssetId<Mesh>, Handle<Mesh>>,
}

/// The leaf mesh of `tree` as camera-facing cards: `POSITION` is each card's
/// centre and [`ATTRIBUTE_BILLBOARD`] its corner in the camera basis, the
/// card's angle folded in (`r * x + u * y` of `leaf_billboards` with
/// `r = right cos + up sin`, `u = up cos - right sin`). UVs, colours and indices
/// are the asset's.
fn billboard_leaf_mesh(tree: &SptTree, source: &Mesh) -> Mesh {
    let mut mesh = source.clone();
    let count = tree.leaves.len() * 4;
    let mut centers = Vec::with_capacity(count);
    let mut corners = Vec::with_capacity(count);
    for leaf in &tree.leaves {
        let center = m2_assets::coords::vec3(leaf.center).to_array();
        centers.extend([center; 4]);
        corners.extend(leaf_corners(leaf));
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(centers));
    mesh.insert_attribute(ATTRIBUTE_BILLBOARD, VertexAttributeValues::Float32x2(corners));
    mesh
}

/// The four corners of `leaf` in the camera basis, in `leaf_billboards` order.
fn leaf_corners(leaf: &m2_formats::spt::Leaf) -> [[f32; 2]; 4] {
    let (cos, sin) = (leaf.angle.cos(), leaf.angle.sin());
    let (w, h) = (leaf.half_width, leaf.half_height);
    [(-w, h), (w, h), (w, -h), (-w, -h)].map(|(x, y)| [cos * x - sin * y, sin * x + cos * y])
}

/// The shared billboard leaf mesh of `tree`, built on first use.
fn shared_leaf_mesh(world: &mut World, tree: &SptTree) -> Option<Handle<Mesh>> {
    let source = tree.leaf_mesh.as_ref()?;
    let id = source.id();
    if let Some(mesh) = world.get_resource::<LeafMeshCache>().and_then(|cache| cache.meshes.get(&id)) {
        return Some(mesh.clone());
    }
    let mesh = billboard_leaf_mesh(tree, world.resource::<Assets<Mesh>>().get(source)?);
    let handle = world.resource_mut::<Assets<Mesh>>().add(mesh);
    world.get_resource_or_init::<LeafMeshCache>().meshes.insert(id, handle.clone());
    Some(handle)
}

/// One spawned tree.
pub struct Tree {
    pub root: Entity,
    pub tree: SptTree,
}

impl Tree {
    /// Spawns `tree` at `transform` (under `parent` when given).
    pub fn spawn(world: &mut World, tree: SptTree, transform: Transform, parent: Option<Entity>, fog: Option<Fog>) -> Self {
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

        if let Some(leaf_mesh) = shared_leaf_mesh(world, &tree) {
            let base = match composite {
                Some(texture) => FixedFunctionMaterial::opaque(texture).sampler(leaf_sampler()),
                // Vertex colours only, as when a tree ships no atlas.
                None => FixedFunctionMaterial::untextured(),
            };
            let base = if let Some(fog) = fog { base.fog(fog) } else { base };
            let material = crate::render::cached_material(world, alpha_tested(base, tree.leaf_alpha_test));
            // A card can rotate in any direction, so bound each centre by
            // the diagonal of its quad rather than its initial orientation.
            let (min, max) = tree.leaves.iter().fold(tree.bounding_box, |(min, max), leaf| {
                let center = m2_assets::coords::vec3(leaf.center);
                let radius = leaf.half_width.hypot(leaf.half_height);
                (min.min(center - radius), max.max(center + radius))
            });
            world.spawn((
                Name::new("leaves"),
                Mesh3d(leaf_mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                Visibility::Visible,
                Aabb::from_min_max(min, max),
                ChildOf(root),
            ));
        }
        Tree { root, tree }
    }
}

fn spawn_part(world: &mut World, root: Entity, name: &str, part: &SptPart, texture: Option<Handle<Image>>, fog: Option<Fog>) {
    let base = match texture {
        Some(texture) => FixedFunctionMaterial::opaque(texture),
        None => FixedFunctionMaterial::untextured().diffuse(Vec4::new(0.42, 0.36, 0.24, 1.0)),
    };
    let base = if let Some(fog) = fog { base.fog(fog) } else { base };
    let material = crate::render::cached_material(world, alpha_tested(base, part.alpha_test));
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

    /// `billboard_vertex` in `fixed_function.wesl`, transcribed: the corner of
    /// a card around `center` for a camera at `camera`, both in tree space.
    fn shader_corner(center: Vec3, corner: [f32; 2], camera: Vec3) -> Vec3 {
        let camera = Vec3::new(camera.x, camera.z, camera.y);
        let v = -camera;
        let v = if v.length() < 1e-4 { Vec3::Y } else { v / v.length() };
        let right = Vec3::new(-v.y, v.x, 0.0);
        let right = if right.length() < 1e-4 { Vec3::X } else { right / right.length() };
        let up = v.cross(right);
        let offset = right * corner[0] + up * corner[1];
        center + Vec3::new(offset.x, offset.z, offset.y)
    }

    #[test]
    fn gpu_leaf_cards_match_the_cpu_billboards() {
        let leaf = |center: [f32; 3], angle: f32, half_width: f32, half_height: f32| m2_formats::spt::Leaf {
            center,
            color: 0,
            size: 1.0,
            half_width,
            half_height,
            angle,
            tex_coords: [0.0; 8],
            map_index: 0,
            cluster_index: 0,
        };
        let leaves = [leaf([10.0, -4.0, 300.0], 0.0, 40.0, 25.0), leaf([-60.0, 20.0, 180.0], 1.1, 15.0, 30.0)];
        // Level, oblique, from below and straight above (the degenerate basis).
        for camera in [Vec3::new(0.0, 150.0, -900.0), Vec3::new(700.0, 400.0, 300.0), Vec3::new(-50.0, -200.0, 80.0), Vec3::new(0.0, 900.0, 0.0)] {
            let offsets = m2_formats::spt::leaf_billboards(&leaves, [0.0; 3], m2_assets::coords::vec3_back(camera));
            for (leaf, offset) in leaves.iter().zip(&offsets) {
                for (k, corner) in leaf_corners(leaf).into_iter().enumerate() {
                    let cpu = m2_assets::coords::vec3(std::array::from_fn(|i| leaf.center[i] + offset[k * 4 + i]));
                    let gpu = shader_corner(m2_assets::coords::vec3(leaf.center), corner, camera);
                    assert!(cpu.distance(gpu) < 1e-3, "camera {camera}: corner {k} {cpu} vs {gpu}");
                }
            }
        }
    }
}
