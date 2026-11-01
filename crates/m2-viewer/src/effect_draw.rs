//! Bevy draw adapter shared by the map effects and the `.mse` effect viewer.
//!
//! The simulation is in `m2-effect`; this module turns the draws it produces
//! into meshes and fixed-function materials and keeps one entity per batch
//! alive across frames.
use crate::viewer::{MainCamera, ViewerEntity};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use m2_assets::{coords, paths};
use m2_effect::{Camera, EffectInstance, GlibcRandom};
use m2_formats::{
    effect::{EffectData, EffectMesh, load_effect, load_effect_mesh},
    pack::PackManager,
};
use m2_render::{
    Blend, BlendFactor, CullMode, DepthState, FixedFunctionMaterial, Fog, TextureArg, TextureOp, TextureStage, d3d_color,
};
use std::{collections::HashMap, sync::Arc};

/// The `.mde` meshes an effect names, loaded once and shared by every
/// instance (and every restart) of that effect.
pub(crate) type EffectMeshes = HashMap<Vec<u8>, Arc<EffectMesh>>;

pub(crate) fn load_data(packs: &PackManager, file: &[u8]) -> Result<Arc<EffectData>, String> {
    load_effect(packs, file)
        .map(Arc::new)
        .map_err(|e| format!("Effect {}: {e:?}", String::from_utf8_lossy(file)))
}

pub(crate) fn load_meshes(packs: &PackManager, data: &EffectData) -> Result<EffectMeshes, String> {
    let mut meshes = EffectMeshes::new();
    for mesh in &data.meshes {
        let loaded = load_effect_mesh(packs, &mesh.mesh_file_name)
            .map_err(|e| format!("Effect mesh {}: {e:?}", String::from_utf8_lossy(&mesh.mesh_file_name)))?;
        meshes.insert(mesh.mesh_file_name.clone(), Arc::new(loaded));
    }
    Ok(meshes)
}

/// Builds an instance from loaded data and meshes, placed at `matrix`.
pub(crate) fn instance_from_data(
    data: Arc<EffectData>,
    meshes: &EffectMeshes,
    matrix: [f32; 16],
) -> EffectInstance {
    let mut instance = EffectInstance::new(data, &|name: &[u8]| meshes.get(name).cloned(), 0.0);
    instance
        .set_global_matrix(m2_effect::Mat4 { m: std::array::from_fn(|r| std::array::from_fn(|c| matrix[r * 4 + c])) });
    instance
}

pub(crate) fn load_instance(packs: &PackManager, file: &[u8], matrix: [f32; 16]) -> Result<EffectInstance, String> {
    let data = load_data(packs, file)?;
    let meshes = load_meshes(packs, &data)?;
    Ok(instance_from_data(data, &meshes, matrix))
}

#[derive(Default)]
pub(crate) struct EffectRenderer {
    pub instances: Vec<EffectInstance>,
    /// The map environment's fog, when these are map effects.
    pub fog: Option<Fog>,
    rng: GlibcRandom,
    draws: Vec<(Entity, Handle<Mesh>, Handle<FixedFunctionMaterial>)>,
    textures: HashMap<Vec<u8>, Handle<Image>>,
}

struct Draw {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
    texture: Option<Vec<u8>>,
    blend: Option<(u8, u8)>,
    operation: u8,
}

impl EffectRenderer {
    pub fn update(&mut self, world: &mut World) {
        if self.instances.is_empty() {
            return;
        }
        let Some(camera) = world.query_filtered::<&GlobalTransform, With<MainCamera>>().iter(world).next().copied()
        else {
            return;
        };
        let dt = world.resource::<Time>().delta_secs().min(0.1);
        let camera_basis = Camera {
            up: coords::vec3_back(*camera.up()),
            cross: coords::vec3_back(*camera.right()),
            view: coords::vec3_back(*camera.forward()),
        };
        let swap = Mat4::from_mat3(coords::SWAP_YZ);
        let view = m2_effect::Mat4 { m: (swap * camera.to_matrix().inverse() * swap).to_cols_array_2d() };
        let mut jobs = Vec::new();
        for instance in &mut self.instances {
            instance.update(dt, &mut self.rng);
            let draw = instance.render(&camera_basis, &view, dt);
            for system in draw.particle_systems {
                // Batch each texture frame, preserving the simulator's quad order.
                for (frame, texture) in system.textures.iter().enumerate() {
                    let mut job = Draw {
                        positions: Vec::new(),
                        uvs: Vec::new(),
                        colors: Vec::new(),
                        indices: Vec::new(),
                        texture: Some(texture.clone()),
                        blend: Some((system.src_blend, system.dest_blend)),
                        operation: system.color_operation,
                    };
                    for quad in system.quads.iter().filter(|q| q.texture == frame) {
                        let base = job.positions.len() as u32;
                        job.positions.extend(quad.positions.map(|p| coords::vec3(p).to_array()));
                        job.uvs.extend(m2_effect::particle::QUAD_UV);
                        job.colors.extend([d3d_color(quad.color).to_array(); 4]);
                        job.indices.extend([0, 2, 1, 2, 3, 1].map(|i| i + base));
                    }
                    if !job.positions.is_empty() {
                        jobs.push(job);
                    }
                }
            }
            for mesh in draw.meshes {
                let mut indices: Vec<u32> = (0..mesh.vertices.len() as u32).collect();
                coords::flip_winding(&mut indices);
                jobs.push(Draw {
                    positions: mesh
                        .vertices
                        .iter()
                        .map(|v| coords::vec3(mesh.world.transform_coord(v.position)).to_array())
                        .collect(),
                    uvs: mesh.vertices.iter().map(|v| v.tex_coord).collect(),
                    colors: vec![d3d_color(mesh.color).to_array(); mesh.vertices.len()],
                    indices,
                    texture: mesh.texture.map(<[u8]>::to_vec),
                    blend: mesh.blend,
                    operation: mesh.color_operation.unwrap_or(4),
                });
            }
        }
        let fog = self.fog;
        let count = jobs.len();
        for (index, job) in jobs.into_iter().enumerate() {
            let texture = job.texture.map(|name| {
                self.textures
                    .entry(name.clone())
                    .or_insert_with(|| {
                        world.resource::<AssetServer>().load(paths::asset_source_uri(&String::from_utf8_lossy(&name)))
                    })
                    .clone()
            });
            let mut material = texture
                .map(FixedFunctionMaterial::opaque)
                .unwrap_or_default()
                .cull(CullMode::None)
                .depth(DepthState { write: false, ..Default::default() })
                .stage0(TextureStage {
                    color_op: texture_op(job.operation),
                    color_arg1: TextureArg::DIFFUSE,
                    color_arg2: TextureArg::TEXTURE,
                    alpha_op: TextureOp::Modulate,
                    alpha_arg1: TextureArg::DIFFUSE,
                    alpha_arg2: TextureArg::TEXTURE,
                });
            material.blend = job.blend.map(|(src, dst)| Blend::new(blend_factor(src), blend_factor(dst)));
            if let Some(fog) = fog {
                material = material.fog(fog);
            }
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, job.positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, job.uvs);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, job.colors);
            mesh.insert_indices(Indices::U32(job.indices));
            if let Some((entity, mesh_handle, material_handle)) = self.draws.get(index) {
                *world.resource_mut::<Assets<Mesh>>().get_mut(mesh_handle).unwrap() = mesh;
                *world.resource_mut::<Assets<FixedFunctionMaterial>>().get_mut(material_handle).unwrap() = material;
                world.entity_mut(*entity).insert(Visibility::Visible);
            } else {
                let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
                let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(material);
                let entity = world
                    .spawn((
                        ViewerEntity,
                        Name::new("Effect"),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::IDENTITY,
                        Visibility::Visible,
                        bevy::camera::visibility::NoFrustumCulling,
                    ))
                    .id();
                self.draws.push((entity, mesh, material));
            }
        }
        for (entity, _, _) in self.draws.iter().skip(count) {
            world.entity_mut(*entity).insert(Visibility::Hidden);
        }
    }
}

fn blend_factor(value: u8) -> BlendFactor {
    match value {
        1 => BlendFactor::Zero,
        3 => BlendFactor::SrcColor,
        4 => BlendFactor::InvSrcColor,
        5 => BlendFactor::SrcAlpha,
        6 => BlendFactor::InvSrcAlpha,
        7 => BlendFactor::DestAlpha,
        8 => BlendFactor::InvDestAlpha,
        9 => BlendFactor::DestColor,
        10 => BlendFactor::InvDestColor,
        11 => BlendFactor::SrcAlphaSaturate,
        _ => BlendFactor::One,
    }
}
fn texture_op(value: u8) -> TextureOp {
    match value {
        1 => TextureOp::Disable,
        2 => TextureOp::SelectArg1,
        3 => TextureOp::SelectArg2,
        5 => TextureOp::Modulate2x,
        6 => TextureOp::Modulate4x,
        7 => TextureOp::Add,
        8 => TextureOp::AddSigned,
        9 => TextureOp::AddSigned2x,
        10 => TextureOp::Subtract,
        11 => TextureOp::AddSmooth,
        12 => TextureOp::BlendDiffuseAlpha,
        13 => TextureOp::BlendTextureAlpha,
        14 => TextureOp::BlendDiffuseAlpha,
        15 => TextureOp::BlendTextureAlphaPm,
        16 => TextureOp::BlendCurrentAlpha,
        _ => TextureOp::Modulate,
    }
}
