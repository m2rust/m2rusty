//! `.spt` SpeedTree loader: [`SptTree`].
//!
//! A tree is geometry the (clean-room) SpeedTree compatibility layer in
//! `m2-formats` computes from the `.spt` parameters (or reads from a baked
//! `treecache/<hash>.tree` when one exists next to the packs). The asset
//! exposes:
//!
//! * `branches` and `fronds`: static [`Mesh`]es (triangle lists converted from
//!   the stitched strips, degenerate triangles removed). Attributes:
//!   `ATTRIBUTE_POSITION`, `ATTRIBUTE_UV_0` (diffuse), `ATTRIBUTE_UV_1` (the
//!   self-shadow coordinates, normally unused) and `ATTRIBUTE_COLOR` (the
//!   static vertex lighting, `RGBA` floats in gamma space). There are no
//!   normals: the original lit trees with vertex colours only.
//! * `leaf_mesh`: one quad (four vertices, two triangles) per leaf with
//!   `POSITION` (rest placement, camera independent), `UV_0` and `COLOR`.
//!   Leaves face the camera: call [`SptTree::update_leaf_billboards`] with the
//!   current camera whenever it moved (it rewrites `POSITION`; the original
//!   did it every frame for every tree). Material: alpha test with
//!   [`SptTree::leaf_alpha_test`], no culling.
//! * texture asset paths (the `.dds` next to the `.spt`, as the original
//!   derives them), the bounding box and the collision objects.
//!
//! Alpha testing: `alpha_test` values are the reference as the compat layer
//! reports them (`D3DRS_ALPHAREF / 255`).
//!
//! All geometry is in Bevy space ([`crate::coords`]); the tree stands at the
//! origin with `+Y` up.

use bevy::asset::{Asset, AssetLoader, Handle, LoadContext, RenderAssetUsages, io::Reader};
use bevy::math::Vec3;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::reflect::TypePath;
use m2_formats::spt as fmt;

use crate::{Error, coords, paths};

/// One static part (branches or fronds).
#[derive(Debug, Clone)]
pub struct SptPart {
    pub mesh: Handle<Mesh>,
    /// Alpha-test reference reported for the part (0 = no test).
    pub alpha_test: f32,
}

/// A collision object (`GetCollisionObject`), in Bevy space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SptCollision {
    /// `0` sphere, `1` cylinder, `2` box (`m2_formats::spt::CO_*`).
    pub kind: i32,
    pub position: Vec3,
    /// Radius / height / extents as stored, axes swapped like positions.
    pub dimensions: Vec3,
}

/// Texture asset paths (canonical, see [`crate::paths`]); `None` when the tree
/// has none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SptTextures {
    /// Bark, for branches.
    pub branch: Option<String>,
    /// Leaf and frond atlas.
    pub composite: Option<String>,
    pub self_shadow: Option<String>,
}

/// A loaded tree.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct SptTree {
    pub textures: SptTextures,
    pub branches: Option<SptPart>,
    pub fronds: Option<SptPart>,
    /// Quads of the leaves; `None` for a tree without leaves.
    pub leaf_mesh: Option<Handle<Mesh>>,
    pub leaf_alpha_test: f32,
    /// Leaves in source space, kept to orient them towards the camera.
    pub leaves: Vec<fmt::Leaf>,
    /// Bevy-space bounds `(min, max)`.
    pub bounding_box: (Vec3, Vec3),
    pub collisions: Vec<SptCollision>,
    /// `GetTreeSize` (metres).
    pub size: f32,
    /// Whether the geometry came from a baked cache.
    pub baked: bool,
}

impl SptTree {
    /// Rewrites the `POSITION` attribute of a mesh built from this tree's
    /// leaves ([`SptTree::leaf_mesh`], cloned per instance if instances must
    /// face different cameras) so that every leaf faces `camera`.
    ///
    /// `tree_position` and `camera` are world positions in Bevy space of the
    /// tree's origin and of the camera; the tree is not rotated or scaled
    /// (leaves of rotated trees need the camera brought into tree space first).
    pub fn update_leaf_billboards(&self, mesh: &mut Mesh, tree_position: Vec3, camera: Vec3) {
        let positions = leaf_positions(&self.leaves, tree_position, camera);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    }
}

/// Leaf quad corners: centre plus the camera-facing corner offset.
fn leaf_positions(leaves: &[fmt::Leaf], tree_position: Vec3, camera: Vec3) -> Vec<[f32; 3]> {
    let offsets = fmt::leaf_billboards(leaves, coords::vec3_back(tree_position), coords::vec3_back(camera));
    let mut out = Vec::with_capacity(leaves.len() * 4);
    for (leaf, c) in leaves.iter().zip(&offsets) {
        for k in 0..4 {
            let p = [
                leaf.center[0] + c[k * 4],
                leaf.center[1] + c[k * 4 + 1],
                leaf.center[2] + c[k * 4 + 2],
            ];
            out.push(coords::vec3(p).to_array());
        }
    }
    out
}

/// `0xAABBGGRR` (the compat layer's `AGBR`) to RGBA floats.
fn color(abgr: u32) -> [f32; 4] {
    let c = |shift: u32| ((abgr >> shift) & 0xff) as f32 / 255.0;
    [c(0), c(8), c(16), c(24)]
}

/// Triangle list from one stitched strip: D3D alternates the winding of every
/// second triangle and the stitching adds degenerate triangles, which are
/// dropped. The result is in Bevy winding.
fn strip_to_triangles(strip: &[u16], vertex_count: usize) -> Result<Vec<u32>, String> {
    if let Some(bad) = strip.iter().find(|&&i| usize::from(i) >= vertex_count) {
        return Err(format!("strip index {bad} outside {vertex_count} vertices"));
    }
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
    Ok(out)
}

fn indexed_mesh(part: &fmt::Indexed, with_uv1: bool) -> Result<Option<Mesh>, String> {
    if part.vertices.is_empty() || part.strip_indices().len() < 3 {
        return Ok(None);
    }
    let indices = strip_to_triangles(part.strip_indices(), part.vertices.len())?;
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        part.vertices.iter().map(|v| coords::vec3(v.position).to_array()).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, part.vertices.iter().map(|v| v.tex_coords0).collect::<Vec<_>>());
    if with_uv1 {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, part.vertices.iter().map(|v| v.tex_coords1).collect::<Vec<_>>());
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, part.vertices.iter().map(|v| color(v.color)).collect::<Vec<_>>());
    mesh.insert_indices(Indices::U32(indices));
    Ok(Some(mesh))
}

fn leaf_mesh(leaves: &[fmt::Leaf]) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    // Rest placement: facing the -Y axis of the source space (as seen from +Y).
    let rest = leaf_positions(leaves, Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, rest);
    let mut uvs = Vec::with_capacity(leaves.len() * 4);
    let mut colors = Vec::with_capacity(leaves.len() * 4);
    let mut indices = Vec::with_capacity(leaves.len() * 6);
    for (i, leaf) in leaves.iter().enumerate() {
        for k in 0..4 {
            uvs.push([leaf.tex_coords[k * 2], leaf.tex_coords[k * 2 + 1]]);
            colors.push(color(leaf.color));
        }
        // The original draws the corners 0,1,2 and 0,2,3.
        let b = (i * 4) as u32;
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    coords::flip_winding(&mut indices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// `<spt dir>/<name without extension>.dds`, the way the original derives the
/// texture files (`CSpeedTreeWrapper::LoadTree`).
fn texture_path(dir: &str, name: &[u8]) -> String {
    let name = String::from_utf8_lossy(name);
    let name = paths::canonical_asset_path(&name);
    let stem = name.rsplit('/').next().unwrap_or(&name);
    let stem = stem.rsplit_once('.').map_or(stem, |(s, _)| s);
    paths::join(dir, &format!("{stem}.dds"))
}

/// Loads `.spt` files into [`SptTree`].
#[derive(Default, TypePath)]
pub struct SptLoader;

impl AssetLoader for SptLoader {
    type Asset = SptTree;
    type Settings = ();
    type Error = Error;

    async fn load(&self, reader: &mut dyn Reader, _: &(), load_context: &mut LoadContext<'_>) -> Result<SptTree, Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let path = paths::canonical_asset_path(&load_context.path().path().to_string_lossy());
        let ctx = |e: &dyn std::fmt::Display| Error::Decode(format!("{path}: {e}"));

        // Optional baked geometry next to the packs.
        let cache_name = fmt::tree_cache_file_name(fmt::fnv1a32(&bytes));
        let cache_path = load_context.path().resolve(&bevy::asset::AssetPath::parse(&format!("/treecache/{cache_name}")));
        let cache = load_context.read_asset_bytes(cache_path).await.ok();

        let tree = fmt::Tree::load(&bytes, cache.as_deref(), &fmt::LoadParams::default()).map_err(|e| ctx(&e))?;
        let dir = paths::parent(&path).to_owned();
        let geometry = &tree.geometry;

        let textures = SptTextures {
            branch: tree.spt.textures.branch.as_deref().map(|n| texture_path(&dir, n)),
            composite: tree.spt.textures.composite.as_deref().map(|n| texture_path(&dir, n)),
            self_shadow: tree.spt.textures.self_shadow.as_deref().map(|n| texture_path(&dir, n)),
        };

        let part = |load_context: &mut LoadContext<'_>, label: &str, indexed: &fmt::Indexed| {
            indexed_mesh(indexed, true).map(|m| {
                m.map(|mesh| SptPart {
                    mesh: load_context.add_labeled_asset(label.to_owned(), mesh),
                    alpha_test: indexed.alpha_test,
                })
            })
        };
        let branches = part(load_context, "branches", &geometry.branches).map_err(|e| ctx(&e))?;
        let fronds = part(load_context, "fronds", &geometry.fronds).map_err(|e| ctx(&e))?;
        let leaf_mesh = (!geometry.leaves.is_empty())
            .then(|| load_context.add_labeled_asset("leaves".to_owned(), leaf_mesh(&geometry.leaves)));

        Ok(SptTree {
            textures,
            branches,
            fronds,
            leaf_mesh,
            leaf_alpha_test: geometry.leaf_alpha_test,
            leaves: geometry.leaves.clone(),
            bounding_box: coords::aabb(geometry.bounding_box),
            collisions: geometry
                .collisions
                .iter()
                .map(|c| SptCollision {
                    kind: c.kind,
                    position: coords::vec3(c.position),
                    dimensions: coords::vec3(c.dimensions),
                })
                .collect(),
            size: tree.spt.size,
            baked: tree.baked,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["spt"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_become_triangles_without_degenerates() {
        // Two quads stitched by two degenerate triangles: 0 1 2 3 | 3 4 | 4 5 6 7.
        let strip = [0u16, 1, 2, 3, 3, 4, 4, 5, 6, 7];
        let tris = strip_to_triangles(&strip, 8).unwrap();
        // First quad: D3D triangles (0,1,2) and (2,1,3); Bevy winding is reversed.
        assert_eq!(&tris[..6], &[0, 2, 1, 2, 3, 1][..]);
        assert!(tris.chunks(3).all(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]));
        assert!(strip_to_triangles(&[0, 1, 9], 8).is_err());
    }

    #[test]
    fn abgr_colours() {
        assert_eq!(color(0xff00_00ff), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(color(0x80ff_0000), [0.0, 0.0, 1.0, 128.0 / 255.0]);
    }

    #[test]
    fn texture_names_follow_the_spt() {
        assert_eq!(texture_path("ymir work/tree", b"Bark.TGA"), "ymir work/tree/bark.dds");
        assert_eq!(texture_path("ymir work/tree", b"x"), "ymir work/tree/x.dds");
    }
}
