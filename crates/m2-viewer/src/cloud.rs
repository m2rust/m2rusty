//! The map environment's cloud layer (`CSkyBox::RenderCloud`,
//! `neoeter/src/render/SkyBox.cpp`).
//!
//! One large horizontal quad at `CloudHeight` above the camera, textured with
//! `CloudTextureFileName` and scrolled by `CloudSpeed` in texture space. The
//! client's texture stage is `D3DTOP_MODULATEINVALPHA_ADDCOLOR` (texture plus
//! the cloud colour times the inverse texture alpha) with
//! `ONE`/`INVSRCCOLOR` blending; the shader below reproduces it.
//!
//! The client renders the layer with its own fixed projection; a background-
//! sized quad under the normal camera is equivalent and needs no projection
//! switch.

use bevy::asset::{AssetServer, RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use m2_assets::{coords, paths};
use m2_formats::map::EnvironmentData;

use crate::viewer::{MainCamera, ViewerEntity};

/// The cloud colour and texture transform. `_pad` keeps the WGSL uniform
/// struct's 16-byte alignment explicit.
#[derive(Clone, ShaderType, Debug)]
pub struct CloudUniform {
    pub color: Vec4,
    pub offset: Vec2,
    pub scale: Vec2,
    pub _pad: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct CloudMaterial {
    #[uniform(0)]
    pub params: CloudUniform,
    #[texture(1)]
    #[sampler(2)]
    pub image: Handle<Image>,
}

impl Material for CloudMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://m2_viewer/cloud.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "embedded://m2_viewer/cloud.wesl".into()
    }
    fn alpha_mode(&self) -> bevy::material::AlphaMode {
        // The layer blends `ONE`/`INVSRCCOLOR`, so it belongs in the sorted
        // transparent phase.
        bevy::material::AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The quad is seen from below; never cull it. Depth test on, write off
        // as the client's `RenderCloud`.
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
            depth.depth_compare = Some(CompareFunction::LessEqual);
        }
        // `ONE`/`INVSRCCOLOR`, as `RenderCloud` sets it. Bevy would otherwise
        // pick its standard alpha blend for the transparent phase.
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::OneMinusSrc,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::OneMinusSrc,
                        operation: BlendOperation::Add,
                    },
                });
            }
        }
        Ok(())
    }
}

/// Scrolling state of one cloud layer; the material's uniform follows it.
#[derive(Component)]
pub struct CloudLayer {
    pub material: Handle<CloudMaterial>,
    pub speed: Vec2,
    pub offset: Vec2,
}

/// The layer follows the camera horizontally and stays `height` above it.
#[derive(Component)]
pub struct CloudFollow {
    pub height: f32,
}

pub struct CloudPlugin;

impl Plugin for CloudPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "cloud.wesl");
        app.add_plugins(MaterialPlugin::<CloudMaterial>::default())
            .add_systems(Update, (scroll_clouds, follow_camera).after(crate::camera::orbit_control));
    }
}

/// Builds the cloud layer of `environment`, if it names a texture.
pub fn spawn_cloud(world: &mut World, environment: &EnvironmentData) -> Option<Entity> {
    let sky = &environment.sky_box;
    if sky.cloud_texture_file_name.is_empty() {
        return None;
    }
    let image = world.resource::<AssetServer>().load(paths::asset_source_uri(&sky.cloud_texture_file_name));
    let params = CloudUniform {
        color: Vec4::from_array(sky.cloud_color.first),
        offset: Vec2::ZERO,
        scale: Vec2::from_array(sky.cloud_texture_scale),
        _pad: Vec4::ZERO,
    };
    let material = world.resource_mut::<Assets<CloudMaterial>>().add(CloudMaterial { params, image });

    // `Refresh`'s cloud quad, in the client's vertex order (its UVs are
    // (0,1), (0,0), (1,1), (1,0)).
    let corners = [[1.0, 1.0], [-1.0, 1.0], [1.0, -1.0], [-1.0, -1.0]];
    let mut positions = Vec::with_capacity(4);
    let mut uvs = Vec::with_capacity(4);
    for corner in corners {
        positions.push(coords::vec3([corner[0], corner[1], 0.0]).to_array());
    }
    uvs.extend([[0.0, 1.0], [0.0, 0.0], [1.0, 1.0], [1.0, 0.0]]);
    let mut indices: Vec<u32> = vec![0, 1, 2, 2, 1, 3];
    coords::flip_winding(&mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);

    let scale = coords::scale([sky.cloud_scale[0], sky.cloud_scale[1], 1.0]);
    Some(
        world
            .spawn((
                ViewerEntity,
                Name::new("Clouds"),
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::from_scale(scale),
                Visibility::Visible,
                NoFrustumCulling,
                CloudLayer { material, speed: Vec2::from_array(sky.cloud_speed), offset: Vec2::ZERO },
                CloudFollow { height: sky.cloud_height },
            ))
            .id(),
    )
}

/// `RenderCloud`'s scroll: `position += speed * dt`, wrapped at one texture
/// repeat.
fn scroll_clouds(time: Res<Time>, mut layers: Query<&mut CloudLayer>, mut materials: ResMut<Assets<CloudMaterial>>) {
    for mut layer in &mut layers {
        let offset = (layer.offset + layer.speed * time.delta_secs()).rem_euclid(Vec2::ONE);
        layer.offset = offset;
        if let Some(mut material) = materials.get_mut(&layer.material) {
            material.params.offset = offset;
        }
    }
}

/// `CSkyObject::Update` for the cloud layer: the plane stays above the eye.
fn follow_camera(
    camera: Query<&Transform, With<MainCamera>>,
    mut clouds: Query<(&CloudFollow, &mut Transform), Without<MainCamera>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    for (follow, mut transform) in &mut clouds {
        transform.translation = camera.translation + Vec3::Y * follow.height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_texture_builds_no_clouds() {
        let mut world = World::new();
        assert!(spawn_cloud(&mut world, &EnvironmentData::default()).is_none());
    }

    #[test]
    fn scroll_wraps_at_one_repeat() {
        let mut layer =
            CloudLayer { material: Handle::default(), speed: Vec2::new(0.5, -0.25), offset: Vec2::new(0.8, 0.1) };
        layer.offset = (layer.offset + layer.speed * 0.5).rem_euclid(Vec2::ONE);
        assert!((layer.offset - Vec2::new(0.05, 0.975)).abs().max_element() < 1e-5, "{:?}", layer.offset);
    }
}
