//! Bevy draw adapter shared by the map effects and the `.mse` effect viewer.
//!
//! The simulation is in `m2-effect`; this module turns the draws it produces
//! into meshes and fixed-function materials and keeps one entity per batch
//! alive across frames.
use crate::SceneCamera;
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
pub type EffectMeshes = HashMap<Vec<u8>, Arc<EffectMesh>>;

pub fn load_data(packs: &PackManager, file: &[u8]) -> Result<Arc<EffectData>, String> {
    // Motion assets use Bevy's drive-less paths. Pack lookup and relative
    // effect references need the original d:/ymir work/... virtual name.
    // Normalize at this shared boundary for every effect creation path.
    let file = m2_formats::pack::virtual_name(file);
    load_effect(packs, &file)
        .map(Arc::new)
        .map_err(|e| format!("Effect {}: {e}", String::from_utf8_lossy(&file)))
}

pub fn load_meshes(packs: &PackManager, data: &EffectData) -> Result<EffectMeshes, String> {
    let mut meshes = EffectMeshes::new();
    for mesh in &data.meshes {
        let file = m2_formats::pack::virtual_name(&mesh.mesh_file_name);
        match load_effect_mesh(packs, &file) {
            Ok(loaded) => { meshes.insert(mesh.mesh_file_name.clone(), Arc::new(loaded)); }
            // CEffectMeshInstance::OnSetDataPointer can leave a missing mesh
            // unset. Only that element ends; particles and lights still play.
            Err(error) => warn!("Effect mesh {}: {error}; keeping the other effect elements", String::from_utf8_lossy(&file)),
        }
    }
    Ok(meshes)
}

/// Builds an instance from loaded data and meshes, placed at `matrix`.
pub fn instance_from_data(
    data: Arc<EffectData>,
    meshes: &EffectMeshes,
    matrix: [f32; 16],
) -> EffectInstance {
    let mut instance = EffectInstance::new(data, &|name: &[u8]| meshes.get(name).cloned(), 0.0);
    instance
        .set_global_matrix(m2_effect::Mat4 { m: std::array::from_fn(|r| std::array::from_fn(|c| matrix[r * 4 + c])) });
    instance
}

pub fn load_instance(packs: &PackManager, file: &[u8], matrix: [f32; 16]) -> Result<EffectInstance, String> {
    let data = load_data(packs, file)?;
    let meshes = load_meshes(packs, &data)?;
    Ok(instance_from_data(data, &meshes, matrix))
}

/// Simulates effect instances and keeps one entity per draw batch alive
/// across frames.
#[derive(Default)]
pub struct EffectRenderer {
    pub instances: Vec<EffectInstance>,
    /// Parent of the batch entities; they are top-level when `None`.
    pub root: Option<Entity>,
    /// The map environment's fog, when these are map effects.
    pub fog: Option<Fog>,
    rng: GlibcRandom,
    draws: Vec<(Entity, Handle<Mesh>, Handle<FixedFunctionMaterial>, MaterialKey)>,
    textures: HashMap<Vec<u8>, Handle<Image>>,
}

#[derive(PartialEq)]
struct MaterialKey {
    texture: Option<Handle<Image>>,
    blend: Option<(u8, u8)>,
    operation: u8,
    fog: Option<Fog>,
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

/// The D3D-to-Bevy axis conversion reverses the camera-facing particle quad.
/// Keep the simulator's original UVs intact and correct their handedness here.
fn particle_uv() -> [[f32; 2]; 4] {
    m2_effect::particle::QUAD_UV.map(|[u, v]| [1.0 - u, v])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_effect_keeps_its_lifetime_until_its_texture_is_loaded() {
        use bevy::asset::{AssetApp, AssetPlugin};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default())).init_asset::<Image>();
        let world = app.world_mut();
        world.insert_resource(Time::<()>::default());
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
        world.spawn((SceneCamera, GlobalTransform::IDENTITY));
        let mut data = EffectData::default();
        let mut particle = m2_formats::effect::ParticleSystemData::default();
        particle.emitter.cycle_length = 0.01;
        particle.particle.textures.push(b"waiting.png".to_vec());
        data.particles.push(particle);
        let mut renderer = EffectRenderer::default();
        renderer.instances.push(EffectInstance::new(Arc::new(data), &m2_effect::instance::NoMeshes, 0.0));
        renderer.update(world);
        assert!(renderer.instances[0].is_alive());
        assert_eq!(renderer.instances[0].particle_systems()[0].local_time(), 0.0);
        let handle = renderer.textures[b"waiting.png".as_slice()].clone();
        world.resource_mut::<Assets<Image>>().insert(handle.id(), Image::default()).unwrap();
        renderer.update(world);
        assert!(!renderer.instances[0].is_alive());
    }

    #[test]
    fn camera_facing_particle_textures_are_not_mirrored() {
        let mut particle = m2_effect::particle::Particle::default();
        particle.half_size = [1.0, 1.0];
        let props = m2_formats::effect::ParticleProperty { billboard_type: m2_effect::particle::billboard::ALL, ..Default::default() };
        let camera = Camera { up: [0.0, 0.0, 1.0], cross: [1.0, 0.0, 0.0], view: [0.0, -1.0, 0.0] };
        let quad = particle.quad(&props, &camera, None, None);
        let uv = particle_uv();
        for (point, [u, _]) in quad.iter().zip(uv) {
            assert_eq!(point[0].is_sign_positive(), u == 1.0);
        }
    }
}

impl EffectRenderer {
    pub fn update(&mut self, world: &mut World) {
        if self.instances.is_empty() {
            for (entity, _, _, _) in &self.draws {
                if let Some(mut visibility) = world.get_mut::<Visibility>(*entity)
                    && *visibility != Visibility::Hidden
                {
                    *visibility = Visibility::Hidden;
                }
            }
            return;
        }
        let Some(camera) = world.query_filtered::<&GlobalTransform, With<SceneCamera>>().iter(world).next().copied()
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
            // C++ loads effect textures synchronously. Our images load in the
            // background: starting a short effect before them can consume its
            // entire visible interval. Request every frame and hold its clock
            // until the available textures are ready.
            let mut ready = true;
            for name in instance.texture_names().filter(|name| !name.is_empty()) {
                let handle = self.textures.entry(name.to_vec()).or_insert_with(|| {
                    world.resource::<AssetServer>().load(paths::asset_source_uri(&String::from_utf8_lossy(name)))
                });
                if world.resource::<Assets<Image>>().get(handle.id()).is_none()
                    && !matches!(world.resource::<AssetServer>().get_load_state(handle.id()), Some(bevy::asset::LoadState::Failed(_)))
                {
                    ready = false;
                }
            }
            if !ready { continue; }
            instance.update(dt, &mut self.rng);
            let draw = instance.render(&camera_basis, &view, dt);
            for system in draw.particle_systems {
                // Batch each texture frame, preserving the simulator's quad order.
                let mut batches: Vec<Option<Draw>> = (0..system.textures.len()).map(|_| None).collect();
                for quad in &system.quads {
                    let Some(texture) = system.textures.get(quad.texture) else { continue };
                    let job = batches[quad.texture].get_or_insert_with(|| Draw {
                        positions: Vec::new(),
                        uvs: Vec::new(),
                        colors: Vec::new(),
                        indices: Vec::new(),
                        texture: Some(texture.clone()),
                        blend: Some((system.src_blend, system.dest_blend)),
                        operation: system.color_operation,
                    });
                    let base = job.positions.len() as u32;
                    job.positions.extend(quad.positions.map(|p| coords::vec3(p).to_array()));
                    job.uvs.extend(particle_uv());
                    job.colors.extend([d3d_color(quad.color).to_array(); 4]);
                    job.indices.extend([0, 2, 1, 2, 3, 1].map(|i| i + base));
                }
                jobs.extend(batches.into_iter().flatten());
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
            let key = MaterialKey { texture: texture.clone(), blend: job.blend, operation: job.operation, fog };
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
            if let Some((entity, mesh_handle, material_handle, old_key)) = self.draws.get_mut(index) {
                *world.resource_mut::<Assets<Mesh>>().get_mut(mesh_handle).unwrap() = mesh;
                if *old_key != key {
                    *world.resource_mut::<Assets<FixedFunctionMaterial>>().get_mut(material_handle).unwrap() = material;
                    *old_key = key;
                }
                if world.get::<Visibility>(*entity) != Some(&Visibility::Visible) {
                    world.entity_mut(*entity).insert(Visibility::Visible);
                }
            } else {
                let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
                let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(material);
                let entity = world
                    .spawn((
                        Name::new("Effect"),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::IDENTITY,
                        Visibility::Visible,
                        bevy::camera::visibility::NoFrustumCulling,
                    ))
                    .id();
                if let Some(root) = self.root {
                    world.entity_mut(entity).insert(ChildOf(root));
                }
                self.draws.push((entity, mesh, material, key));
            }
        }
        for (entity, _, _, _) in self.draws.iter().skip(count) {
            if world.get::<Visibility>(*entity) != Some(&Visibility::Hidden) {
                world.entity_mut(*entity).insert(Visibility::Hidden);
            }
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
