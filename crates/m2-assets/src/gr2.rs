//! Granny `.gr2` models ([`Gr2Model`]) and motions ([`AnimationClip`]).
//!
//! # Models
//!
//! [`Gr2Loader`] reads the first model of a file. Every material group of every
//! mesh becomes its own Bevy [`Mesh`] (a labeled sub-asset `mesh{m}_part{g}`),
//! with the vertices it uses only. The client draws the same way, one draw
//! call per triangle group.
//!
//! Vertex attributes of every mesh, **the contract for materials and viewers**:
//!
//! | attribute | type | notes |
//! |-----------|------|-------|
//! | `Mesh::ATTRIBUTE_POSITION` | `Float32x3` | Bevy space ([`crate::coords`]), model rest pose |
//! | `Mesh::ATTRIBUTE_NORMAL`   | `Float32x3` | unit length, Bevy space |
//! | `Mesh::ATTRIBUTE_UV_0`     | `Float32x2` | D3D convention, `v` down, same as wgpu: no flip |
//! | `Mesh::ATTRIBUTE_JOINT_INDEX`  | `Uint16x4` | **only if the model has a skeleton**: index into the skeleton's bones, the entity list of `SkinnedMesh::joints` |
//! | `Mesh::ATTRIBUTE_JOINT_WEIGHT` | `Float32x4` | idem; weights sum to 1 |
//!
//! There are no tangents and no vertex colours. Triangle lists, `u32` indices,
//! winding reversed for Bevy. Meshes whose name starts with `2x` are
//! two-sided ([`Gr2Mesh::two_sided`]). "Rigid" Granny meshes (at most one bone
//! binding) follow their bone like the original does, so they are skinned with
//! weight 1 on that bone.
//!
//! Joint indices are indices into the *skeleton* of the file (not into the
//! per-mesh bone binding list the file stores): the loader remaps them. All
//! meshes of a model therefore share one [`SkinnedMeshInverseBindposes`]
//! ([`Gr2Model::inverse_bindposes`]) and the same joint list.
//!
//! Materials are not created here. [`Gr2Material`] names the textures
//! (canonical `m2://` asset paths, relative names resolved against the model's
//! folder as the client does for local textures) and says whether the
//! material is a blend material (a second "opacity" map).
//!
//! # Skeleton
//!
//! [`Gr2Skeleton`] has bone names, parent indices, local transforms (Bevy
//! space) and inverse-world matrices. [`spawn_gr2_model`] spawns the bone
//! entity hierarchy (named, with [`AnimationTargetId`] from the bone name, so
//! any motion applies to any skeleton with the same bone names) and the mesh
//! entities. Equipment and hair that share the body's bone names are spawned
//! on the body's skeleton with [`spawn_gr2_meshes`].
//!
//! # Motions
//!
//! [`Gr2MotionLoader`] loads the first animation of a `.gr2` as an
//! [`AnimationClip`] (ask for it by type: `asset_server.load::<AnimationClip>`).
//! Each transform track becomes translation / rotation / scale curves on the
//! target [`bone_target_id`] of the track name. Curves are evaluated exactly as
//! the project's C++ Granny replacement (`GrannyCompat.h`) does: the
//! controls are keyframes at the knot times, linearly interpolated, rotations
//! spherically; the degree stored in the file is not used (a true Granny
//! evaluates degree-2 B-splines, which the C++ port also does not do). Scale
//! uses the diagonal of the scale/shear matrix. Root motion
//! (`loop_translation`) is ignored.

use std::collections::HashMap;

use bevy::animation::prelude::*;
use bevy::animation::{AnimatedBy, AnimationTargetId, animated_field};
use bevy::asset::{Asset, AssetLoader, Handle, LoadContext, RenderAssetUsages, io::Reader};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::system::EntityCommands;
use bevy::math::{Mat4, Vec3};
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::{Commands, Entity, Name, Transform, Visibility};
use bevy::reflect::TypePath;
use m2_formats::gr2 as fmt;

use crate::{Error, coords, paths};

/// A bone of a [`Gr2Skeleton`].
#[derive(Debug, Clone)]
pub struct Gr2Bone {
    pub name: String,
    /// Index of the parent bone; `None` for a root.
    pub parent: Option<usize>,
    /// Rest pose relative to the parent, in Bevy space.
    pub local: Transform,
    /// World-to-bone matrix of the bind pose, in Bevy space.
    pub inverse_world: Mat4,
}

/// The skeleton of a model.
#[derive(Debug, Clone, Default)]
pub struct Gr2Skeleton {
    pub name: String,
    pub bones: Vec<Gr2Bone>,
}

impl Gr2Skeleton {
    /// Index of the bone called `name` (exact match first, then ignoring ASCII case).
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones
            .iter()
            .position(|b| b.name == name)
            .or_else(|| self.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name)))
    }
}

/// Texture references of a material (canonical asset paths).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gr2Material {
    pub name: String,
    /// "Diffuse Color" map.
    pub diffuse: Option<String>,
    /// "Opacity" map: present for blend materials (`Blend*` with two maps, or
    /// an explicit opacity map); the client then draws alpha blended with the
    /// second texture's alpha.
    pub opacity: Option<String>,
}

/// One drawable piece: a triangle group of a mesh.
#[derive(Debug, Clone)]
pub struct Gr2MeshPart {
    pub mesh: Handle<Mesh>,
    pub material: Gr2Material,
}

/// A Granny mesh: one part per material group.
#[derive(Debug, Clone, Default)]
pub struct Gr2Mesh {
    pub name: String,
    /// Name starts with `2x`: render without back-face culling.
    pub two_sided: bool,
    pub parts: Vec<Gr2MeshPart>,
}

/// A loaded `.gr2` model.
#[derive(Asset, TypePath, Debug, Clone, Default)]
pub struct Gr2Model {
    pub name: String,
    pub skeleton: Option<Gr2Skeleton>,
    /// Present with a skeleton: one matrix per bone, in bone order.
    pub inverse_bindposes: Option<Handle<SkinnedMeshInverseBindposes>>,
    pub meshes: Vec<Gr2Mesh>,
    /// Rest-pose bounds `(min, max)` in Bevy space.
    pub bounds: (Vec3, Vec3),
}

impl Gr2Model {
    /// Every texture asset path the model refers to, without duplicates.
    pub fn texture_paths(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for part in self.meshes.iter().flat_map(|m| &m.parts) {
            for t in [&part.material.diffuse, &part.material.opacity].into_iter().flatten() {
                if !out.contains(&t.as_str()) {
                    out.push(t);
                }
            }
        }
        out
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Texture name as stored in a model, resolved the way
/// `CGrannyMaterial::__GetImagePointer` does: names without a drive prefix are
/// relative to the model's folder.
fn texture_asset_path(model_dir: &str, from_file: &[u8]) -> String {
    let raw = text(from_file);
    let has_drive = raw.as_bytes().get(1) == Some(&b':');
    let canonical = paths::canonical_asset_path(&raw);
    if has_drive || raw.len() <= 2 { canonical } else { paths::join(model_dir, &canonical) }
}

/// First texture of a material (`GrannyGetMaterialTextureByType`).
fn material_texture<'a>(m: &'a fmt::Material, usages: &[&str]) -> Option<&'a fmt::Texture> {
    if let Some(t) = &m.texture {
        return Some(t);
    }
    m.maps.iter().find_map(|map| {
        let usage = String::from_utf8_lossy(&map.usage);
        let ok = usages.iter().any(|u| usage.eq_ignore_ascii_case(u));
        ok.then(|| map.material.as_ref().and_then(|x| x.texture.as_ref())).flatten()
    })
}

fn convert_material(model_dir: &str, m: &fmt::Material) -> Gr2Material {
    let name = text(&m.name);
    let (diffuse, opacity) = if m.map_count > 1 && name.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("Blend")) {
        let tex = |i: usize| m.maps.get(i).and_then(|map| map.material.as_ref()).and_then(|x| x.texture.as_ref());
        (tex(0), tex(1))
    } else {
        (
            material_texture(m, &["Diffuse Color", "color"]),
            material_texture(m, &["Opacity", "Filter Color"]).filter(|_| m.texture.is_none()),
        )
    };
    let path = |t: &fmt::Texture| (!t.from_file_name.is_empty()).then(|| texture_asset_path(model_dir, &t.from_file_name));
    Gr2Material { name, diffuse: diffuse.and_then(path), opacity: opacity.and_then(path) }
}

fn local_transform(t: &fmt::Transform) -> Transform {
    const HAS_POSITION: u32 = 1;
    const HAS_ORIENTATION: u32 = 2;
    const HAS_SCALE_SHEAR: u32 = 4;
    let mut out = Transform::IDENTITY;
    if t.flags & HAS_POSITION != 0 {
        out.translation = coords::vec3(t.position);
    }
    if t.flags & HAS_ORIENTATION != 0 {
        let q = coords::quat(t.orientation);
        if q.length_squared() > 1e-12 {
            out.rotation = q.normalize();
        }
    }
    if t.flags & HAS_SCALE_SHEAR != 0 {
        out.scale = coords::scale([t.scale_shear[0][0], t.scale_shear[1][1], t.scale_shear[2][2]]);
    }
    out
}

fn convert_skeleton(s: &fmt::Skeleton) -> Gr2Skeleton {
    let n = s.bones.len();
    Gr2Skeleton {
        name: text(&s.name),
        bones: s
            .bones
            .iter()
            .enumerate()
            .map(|(i, b)| Gr2Bone {
                name: text(&b.name),
                // Only parents that precede the bone are accepted: a cycle or a
                // dangling index in a corrupt file would otherwise loop.
                parent: usize::try_from(b.parent_index).ok().filter(|&p| p < n && p != i),
                local: local_transform(&b.local_transform),
                inverse_world: coords::mat4_from_d3d(b.inverse_world),
            })
            .collect(),
    }
}

/// Builds the Bevy meshes of one Granny mesh: `(material index, mesh)` per group.
fn build_mesh_parts(
    mesh: &fmt::Mesh,
    skeleton: Option<&Gr2Skeleton>,
    bounds: &mut Option<(Vec3, Vec3)>,
) -> Result<Vec<(i32, Mesh)>, String> {
    let (Some(vd), Some(topology)) = (&mesh.vertex_data, &mesh.topology) else {
        return Ok(Vec::new());
    };
    let Some(vertices) = &vd.vertices else {
        return Ok(Vec::new());
    };

    // All indices as u32.
    let all: Vec<u32> = match &topology.indices {
        fmt::Indices::None => return Ok(Vec::new()),
        fmt::Indices::U16(v) => v.iter().map(|&i| u32::from(i)).collect(),
        fmt::Indices::I32(v) => v
            .iter()
            .map(|&i| u32::try_from(i).map_err(|_| format!("negative index {i}")))
            .collect::<Result<_, _>>()?,
    };
    if let Some(&bad) = all.iter().find(|&&i| i as usize >= vertices.len()) {
        return Err(format!("index {bad} outside {} vertices", vertices.len()));
    }

    // Binding index -> skeleton bone index.
    let binding_bones: Vec<u16> = mesh
        .bone_bindings
        .iter()
        .map(|b| {
            let name = text(&b.bone_name);
            skeleton.and_then(|s| s.bone_index(&name)).unwrap_or(0).min(usize::from(u16::MAX)) as u16
        })
        .collect();

    // Per-vertex data in Bevy space.
    struct V {
        pos: [f32; 3],
        normal: [f32; 3],
        uv: [f32; 2],
        joints: [u16; 4],
        weights: [f32; 4],
    }
    let rigid_bone = binding_bones.first().copied().unwrap_or(0);
    let convert = |position: [f32; 3], normal: [f32; 3], uv: [f32; 2], joints: [u16; 4], weights: [f32; 4]| {
        let n = coords::vec3(normal);
        V {
            pos: coords::vec3(position).to_array(),
            normal: if n.length_squared() > 1e-12 { n.normalize().to_array() } else { [0.0, 1.0, 0.0] },
            uv,
            joints,
            weights,
        }
    };
    let verts: Vec<V> = match vertices {
        fmt::Vertices::Rigid(v) => v
            .iter()
            .map(|v| convert(v.position, v.normal, v.uv, [rigid_bone, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]))
            .collect(),
        fmt::Vertices::Skinned(v) => v
            .iter()
            .map(|v| {
                let mut joints = [0u16; 4];
                let mut weights = [0f32; 4];
                for k in 0..4 {
                    if v.bone_weights[k] != 0 {
                        // An index outside the binding list falls back to bone 0, like a missing name.
                        joints[k] = binding_bones.get(usize::from(v.bone_indices[k])).copied().unwrap_or(0);
                        weights[k] = f32::from(v.bone_weights[k]) / 255.0;
                    }
                }
                let sum: f32 = weights.iter().sum();
                if sum > 0.0 {
                    weights.iter_mut().for_each(|w| *w /= sum);
                } else {
                    weights = [1.0, 0.0, 0.0, 0.0];
                }
                convert(v.position, v.normal, v.uv, joints, weights)
            })
            .collect(),
    };

    for v in &verts {
        let p = Vec3::from(v.pos);
        *bounds = Some(match *bounds {
            Some((lo, hi)) => (lo.min(p), hi.max(p)),
            None => (p, p),
        });
    }

    // Triangle groups; a file without groups is one group over everything.
    let groups: Vec<(i32, usize, usize)> = if topology.groups.is_empty() {
        vec![(0, 0, all.len() / 3)]
    } else {
        topology
            .groups
            .iter()
            .map(|g| (g.material_index, g.tri_first.max(0) as usize, g.tri_count.max(0) as usize))
            .collect()
    };

    let skinned = skeleton.is_some_and(|s| !s.bones.is_empty());
    let mut out = Vec::new();
    for (material, first, count) in groups {
        let (start, end) = (first * 3, (first + count) * 3);
        if count == 0 || end > all.len() {
            if count != 0 {
                return Err(format!("triangle group {first}+{count} outside {} indices", all.len() / 3));
            }
            continue;
        }
        // Compact the vertices this group uses.
        let mut remap: HashMap<u32, u32> = HashMap::new();
        let mut used: Vec<u32> = Vec::new();
        let mut indices: Vec<u32> = Vec::with_capacity(end - start);
        for &i in &all[start..end] {
            let new = *remap.entry(i).or_insert_with(|| {
                used.push(i);
                (used.len() - 1) as u32
            });
            indices.push(new);
        }
        coords::flip_winding(&mut indices);

        let used_vertices: Vec<&V> = used.iter().map(|&u| &verts[u as usize]).collect();
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, used_vertices.iter().map(|v| v.pos).collect::<Vec<_>>());
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, used_vertices.iter().map(|v| v.normal).collect::<Vec<_>>());
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, used_vertices.iter().map(|v| v.uv).collect::<Vec<_>>());
        if skinned {
            m.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                VertexAttributeValues::Uint16x4(used_vertices.iter().map(|v| v.joints).collect()),
            );
            m.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, used_vertices.iter().map(|v| v.weights).collect::<Vec<_>>());
        }
        m.insert_indices(Indices::U32(indices));
        out.push((material, m));
    }
    Ok(out)
}

/// Converts a parsed model, registering meshes as labeled sub-assets.
pub fn convert_model(
    model: &fmt::Model,
    model_dir: &str,
    load_context: &mut LoadContext<'_>,
) -> Result<Gr2Model, String> {
    let skeleton = model.skeleton.as_ref().map(convert_skeleton).filter(|s| !s.bones.is_empty());
    let inverse_bindposes = skeleton.as_ref().map(|s| {
        let poses: Vec<Mat4> = s.bones.iter().map(|b| b.inverse_world).collect();
        load_context.add_labeled_asset("inverse_bindposes".to_owned(), SkinnedMeshInverseBindposes::from(poses))
    });

    let mut bounds = None;
    let mut meshes = Vec::new();
    for (mi, binding) in model.mesh_bindings.iter().enumerate() {
        let Some(mesh) = binding else { continue };
        let materials: Vec<Gr2Material> =
            mesh.material_bindings.iter().map(|m| m.as_ref().map(|m| convert_material(model_dir, m)).unwrap_or_default()).collect();
        let name = text(&mesh.name);
        let parts = build_mesh_parts(mesh, skeleton.as_ref(), &mut bounds).map_err(|e| format!("mesh {name:?}: {e}"))?;
        let mut out = Gr2Mesh { two_sided: name.starts_with("2x"), name, parts: Vec::new() };
        for (gi, (material, bevy_mesh)) in parts.into_iter().enumerate() {
            // `LoadTriGroupNodeList`: an invalid material index uses material 0.
            let material = usize::try_from(material).ok().and_then(|i| materials.get(i)).or(materials.first());
            out.parts.push(Gr2MeshPart {
                mesh: load_context.add_labeled_asset(format!("mesh{mi}_part{gi}"), bevy_mesh),
                material: material.cloned().unwrap_or_default(),
            });
        }
        meshes.push(out);
    }

    Ok(Gr2Model {
        name: text(&model.name),
        skeleton,
        inverse_bindposes,
        meshes,
        bounds: bounds.unwrap_or((Vec3::ZERO, Vec3::ZERO)),
    })
}

/// Loads the first model of a `.gr2` file into a [`Gr2Model`].
#[derive(Default, TypePath)]
pub struct Gr2Loader;

impl AssetLoader for Gr2Loader {
    type Asset = Gr2Model;
    type Settings = ();
    type Error = Error;

    async fn load(&self, reader: &mut dyn Reader, _: &(), load_context: &mut LoadContext<'_>) -> Result<Gr2Model, Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let path = paths::canonical_asset_path(&load_context.path().path().to_string_lossy());
        let ctx = |e: String| Error::Decode(format!("{path}: {e}"));

        let file = fmt::parse(&bytes).map_err(|e| ctx(e.to_string()))?;
        let model = file.models.first().ok_or_else(|| ctx("file has no model".into()))?;
        convert_model(model, paths::parent(&path), load_context).map_err(ctx)
    }

    fn extensions(&self) -> &[&str] {
        &["gr2"]
    }
}

// ---------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------

/// Entities created by [`spawn_gr2_model`].
#[derive(Debug, Clone)]
pub struct Gr2Spawn {
    /// Root entity: has an [`AnimationPlayer`]; bones and meshes are its descendants.
    pub root: Entity,
    /// Bone entities, in skeleton order (empty for a model without skeleton).
    pub bones: Vec<Entity>,
    pub bone_by_name: HashMap<String, Entity>,
    /// Mesh entities, in `(mesh, part)` order.
    pub parts: Vec<Entity>,
}

/// The [`AnimationTargetId`] of a bone (a motion's track name is the bone name).
pub fn bone_target_id(bone_name: &str) -> AnimationTargetId {
    AnimationTargetId::from_name(&Name::new(bone_name.to_owned()))
}

/// Spawns the bone hierarchy of `skeleton` under `root`: each bone entity is
/// named, has its rest `Transform`, an [`AnimationTargetId`] and `AnimatedBy(root)`.
/// Returns the bone entities in skeleton order.
pub fn spawn_gr2_skeleton(commands: &mut Commands, root: Entity, skeleton: &Gr2Skeleton) -> Vec<Entity> {
    let bones: Vec<Entity> = skeleton
        .bones
        .iter()
        .map(|b| {
            commands
                .spawn((Name::new(b.name.clone()), b.local, Visibility::default(), bone_target_id(&b.name), AnimatedBy(root)))
                .id()
        })
        .collect();
    for (i, b) in skeleton.bones.iter().enumerate() {
        commands.entity(bones[i]).insert(ChildOf(b.parent.map_or(root, |p| bones[p])));
    }
    bones
}

/// Spawns the mesh entities of `model` under `parent`, skinned to the entities
/// in `bone_by_name` (looked up by the model's own bone names, ASCII
/// case-insensitive; a missing bone falls back to `fallback`). Use it to put
/// hair or equipment on a body skeleton. `decorate` receives each mesh part's
/// entity to insert the material (and anything else).
pub fn spawn_gr2_meshes(
    commands: &mut Commands,
    model: &Gr2Model,
    parent: Entity,
    bone_by_name: &HashMap<String, Entity>,
    fallback: Entity,
    mut decorate: impl FnMut(&mut EntityCommands, &Gr2Mesh, &Gr2MeshPart),
) -> Vec<Entity> {
    let lowered: HashMap<String, Entity> = bone_by_name.iter().map(|(k, v)| (k.to_ascii_lowercase(), *v)).collect();
    let joints: Option<Vec<Entity>> = model.skeleton.as_ref().map(|s| {
        s.bones
            .iter()
            .map(|b| bone_by_name.get(&b.name).or_else(|| lowered.get(&b.name.to_ascii_lowercase())).copied().unwrap_or(fallback))
            .collect()
    });

    let mut out = Vec::new();
    for mesh in &model.meshes {
        for part in &mesh.parts {
            let mut entity = commands.spawn((
                Name::new(mesh.name.clone()),
                bevy::prelude::Mesh3d(part.mesh.clone()),
                Transform::IDENTITY,
                Visibility::default(),
                ChildOf(parent),
            ));
            if let (Some(joints), Some(bindposes)) = (&joints, &model.inverse_bindposes) {
                // The bounds of a skinned mesh are those of the rest pose: skip culling.
                entity.insert((
                    SkinnedMesh { inverse_bindposes: bindposes.clone(), joints: joints.clone() },
                    NoFrustumCulling,
                ));
            }
            decorate(&mut entity, mesh, part);
            out.push(entity.id());
        }
    }
    out
}

/// Spawns a whole model: a root entity (with an [`AnimationPlayer`]), its bone
/// hierarchy and its meshes. `decorate` inserts the material of each part.
pub fn spawn_gr2_model(
    commands: &mut Commands,
    model: &Gr2Model,
    decorate: impl FnMut(&mut EntityCommands, &Gr2Mesh, &Gr2MeshPart),
) -> Gr2Spawn {
    let root = commands
        .spawn((Name::new(model.name.clone()), Transform::IDENTITY, Visibility::default(), AnimationPlayer::default()))
        .id();
    let bones = model.skeleton.as_ref().map(|s| spawn_gr2_skeleton(commands, root, s)).unwrap_or_default();
    let bone_by_name: HashMap<String, Entity> = model
        .skeleton
        .iter()
        .flat_map(|s| s.bones.iter().zip(&bones))
        .map(|(b, &e)| (b.name.clone(), e))
        .collect();
    let parts = spawn_gr2_meshes(commands, model, root, &bone_by_name, root, decorate);
    Gr2Spawn { root, bones, bone_by_name, parts }
}

// ---------------------------------------------------------------------------
// Motions
// ---------------------------------------------------------------------------

/// Keyframes `(time, value)` of one curve, evaluated like `GrannySampleCurve*`
/// (see the module documentation). `None` when the curve has no usable controls.
fn keyframes<T: Clone>(
    curve: &fmt::Curve,
    dim: usize,
    duration: f32,
    value: impl Fn(&[f32]) -> T,
) -> Option<Vec<(f32, T)>> {
    let controls = curve.controls.as_slice();
    if controls.len() < dim || (curve.control_count as usize) < dim {
        return None;
    }
    let n = (controls.len() / dim).min(curve.knots.len());
    let constant = curve.degree == 0 || n <= 1;
    if constant {
        let v = value(&controls[..dim]);
        let end = duration.max(1e-3);
        return Some(vec![(0.0, v.clone()), (end, v)]);
    }
    let mut out: Vec<(f32, T)> = Vec::with_capacity(n);
    for k in 0..n {
        let t = curve.knots[k];
        if !t.is_finite() || out.last().is_some_and(|(prev, _)| t <= *prev + 1e-6) {
            continue;
        }
        out.push((t, value(&controls[k * dim..(k + 1) * dim])));
    }
    if out.len() < 2 {
        let v = value(&controls[..dim]);
        return Some(vec![(0.0, v.clone()), (duration.max(1e-3), v)]);
    }
    Some(out)
}

/// Converts a Granny animation into an [`AnimationClip`] (see the module docs).
pub fn animation_to_clip(animation: &fmt::Animation) -> AnimationClip {
    let mut clip = AnimationClip::default();
    let duration = animation.duration.max(0.0);

    // Track groups with more tracks first; a bone's first track wins.
    let mut groups: Vec<&fmt::TrackGroup> = animation.track_groups.iter().collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.transform_tracks.len()));
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for track in groups.iter().flat_map(|g| &g.transform_tracks) {
        let bone = text(&track.name);
        if !seen.insert(bone.clone()) {
            continue;
        }
        let target = bone_target_id(&bone);

        if let Some(keys) = keyframes(&track.position, 3, duration, |c| coords::vec3([c[0], c[1], c[2]])) {
            if let Ok(curve) = AnimatableKeyframeCurve::new(keys) {
                clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::translation), curve));
            }
        }
        if let Some(keys) = keyframes(&track.orientation, 4, duration, |c| {
            let q = coords::quat([c[0], c[1], c[2], c[3]]);
            if q.length_squared() > 1e-12 { q.normalize() } else { bevy::math::Quat::IDENTITY }
        }) {
            if let Ok(curve) = AnimatableKeyframeCurve::new(keys) {
                clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::rotation), curve));
            }
        }
        if let Some(keys) = keyframes(&track.scale_shear, 9, duration, |c| coords::scale([c[0], c[4], c[8]])) {
            if let Ok(curve) = AnimatableKeyframeCurve::new(keys) {
                clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::scale), curve));
            }
        }
    }
    if duration > clip.duration() {
        clip.set_duration(duration);
    }
    clip
}

/// Loads the first animation of a `.gr2` file as an [`AnimationClip`].
#[derive(Default, TypePath)]
pub struct Gr2MotionLoader;

impl AssetLoader for Gr2MotionLoader {
    type Asset = AnimationClip;
    type Settings = ();
    type Error = Error;

    async fn load(&self, reader: &mut dyn Reader, _: &(), load_context: &mut LoadContext<'_>) -> Result<AnimationClip, Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let path = load_context.path().to_string();
        let file = fmt::parse(&bytes).map_err(|e| Error::Decode(format!("{path}: {e}")))?;
        let animation =
            file.animations.first().ok_or_else(|| Error::Decode(format!("{path}: file has no animation")))?;
        Ok(animation_to_clip(animation))
    }

    fn extensions(&self) -> &[&str] {
        &["gr2"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(degree: i32, knots: &[f32], controls: &[f32]) -> fmt::Curve {
        fmt::Curve {
            degree,
            knot_count: knots.len() as i32,
            knots: knots.to_vec(),
            control_count: controls.len() as i32,
            controls: controls.to_vec(),
        }
    }

    #[test]
    fn linear_keyframes_follow_the_knots() {
        let c = curve(2, &[0.0, 0.5, 1.0], &[0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 2.0, 4.0, 6.0]);
        let k = keyframes(&c, 3, 1.0, |v| [v[0], v[1], v[2]]).unwrap();
        assert_eq!(k.len(), 3);
        assert_eq!(k[1], (0.5, [1.0, 2.0, 3.0]));
    }

    #[test]
    fn constant_curves_get_two_keys() {
        let c = curve(0, &[0.0], &[1.0, 2.0, 3.0]);
        let k = keyframes(&c, 3, 2.0, |v| v[0]).unwrap();
        assert_eq!(k, vec![(0.0, 1.0), (2.0, 1.0)]);
        // No controls at all: the track keeps the rest pose.
        assert!(keyframes(&curve(0, &[], &[]), 3, 1.0, |v| v[0]).is_none());
    }

    #[test]
    fn duplicate_knots_are_dropped() {
        let c = curve(1, &[0.0, 0.0, 1.0], &[1.0, 2.0, 3.0]);
        let k = keyframes(&c, 1, 1.0, |v| v[0]).unwrap();
        assert_eq!(k, vec![(0.0, 1.0), (1.0, 3.0)]);
    }

    #[test]
    fn texture_names_resolve_against_the_model_folder() {
        assert_eq!(texture_asset_path("ymir work/pc/a", b"x.dds"), "ymir work/pc/a/x.dds");
        assert_eq!(texture_asset_path("ymir work/pc/a", b"D:\\Ymir Work\\Tex\\X.dds"), "ymir work/tex/x.dds");
        assert_eq!(texture_asset_path("ymir work/pc/a", b"sub\\X.DDS"), "ymir work/pc/a/sub/x.dds");
    }
}
