//! The character shadow (`SHADOW_LEVEL` 3, `SHADOW_ALL`).
//!
//! C++: `CMapOutdoor::BeginRenderCharacterShadowToTexture`
//! (`client/src/game/MapOutdoorCharacterShadow.cpp`) renders every actor flat
//! grey (`TFACTOR 0xFF808080`) onto a white 512² texture through a fixed
//! oblique orthographic view: 2550×2550 units around the camera target, the
//! eye at `target + (-√3, -1, 2√3) * 1250` (Metin2 space, z up). The terrain's
//! shadow pass (`__HardwareTransformPatch_RenderPatchSplat`) then multiplies
//! the ground by it through `m_matDynamicShadow`, so actors cast soft-edged
//! shadows that ignore the environment light, exactly as in the client.
//!
//! Here a second, orthographic [`Camera3d`] renders the [`SHADOW_LAYER`]
//! (actor meshes, see `model::spawn_model`) into [`CharacterShadow::texture`];
//! `fixed_function.wesl` outputs the grey for this view, which it recognises by
//! its zero exposure. The light view goes to the terrain through one shared [`ShaderBuffer`] whose contents
//! are rewritten in place, so no terrain material is re-prepared per frame.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, Exposure, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::light::cluster::ClusterConfig;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::storage::ShaderBuffer;
use bevy::asset::embedded_asset;
use bevy::material::AlphaMode;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::render::view::{Msaa, NoIndirectDrawing};

/// The render layer actors are drawn on in addition to layer 0, so the shadow
/// camera sees only them.
pub const SHADOW_LAYER: usize = 1;

/// `SetShadowTextureSize(512)` of `SHADOW_ALL`.
const SHADOW_TEXTURE_SIZE: u32 = 512;
/// `D3DXMatrixOrthoRH(2550, 2550, 1, 15000)`.
const SHADOW_BOX: f32 = 2550.0;
const SHADOW_NEAR: f32 = 1.0;
const SHADOW_FAR: f32 = 15_000.0;
/// An exposure value whose `exposure` (`2^-ev100 / 1.2`) underflows to zero:
/// the fixed-function shader draws actors grey for such a view.
const SHADOW_PASS_EV100: f32 = 200.0;

/// The point the scene camera looks at (`CCamera::GetTarget`), set by the
/// in-game camera every frame; the shadow box is centred on it. A frame that
/// sets none (select screen, viewer) turns the shadow camera off.
#[derive(Resource, Default, Clone, Copy)]
pub struct SceneCameraTarget(pub Option<Vec3>);

/// The shadow texture and the light view the terrain samples it with.
#[derive(Resource, Clone)]
pub struct CharacterShadow {
    pub texture: Handle<Image>,
    /// `ShadowView` in `terrain.wesl`: `light_from_world` (column major), then
    /// `(2 / SHADOW_BOX, enabled, 0, 0)`.
    pub view: Handle<ShaderBuffer>,
    /// The shared [`ShadowReceiverMaterial`].
    pub receiver: Handle<ShadowReceiverMaterial>,
}

#[derive(Component)]
struct ShadowCamera;

/// The character shadow on a walkable object (`m_ShadowReceiverVector`: the
/// objects with height data, bridges and platforms), as a multiply-blended
/// copy of its mesh drawn over it; see `shadow_receiver.wesl`. The client
/// only collects the receivers near the player, which is the only place the
/// shadow box reaches anyway.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct ShadowReceiverMaterial {
    #[storage(0, read_only)]
    pub view: Handle<ShaderBuffer>,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
}

impl Material for ShadowReceiverMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://m2_game/render/shadow_receiver.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Multiply
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// Registers the shadow camera and its per-frame placement.
pub struct CharacterShadowPlugin;

impl Plugin for CharacterShadowPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shadow_receiver.wesl");
        app.add_plugins(MaterialPlugin::<ShadowReceiverMaterial>::default())
            .init_resource::<SceneCameraTarget>()
            .add_systems(Startup, spawn_shadow_camera)
            .add_systems(PostUpdate, follow_target.before(TransformSystems::Propagate));
    }
}

/// The light's eye offset from the target, in Bevy space.
fn eye_offset() -> Vec3 {
    let s = 3f32.sqrt() * 1250.0;
    m2_assets::coords::vec3([-s, -1250.0, 2.0 * s])
}

fn view_data(light_from_world: Mat4, enabled: bool) -> [f32; 20] {
    let mut data = [0.0; 20];
    data[..16].copy_from_slice(&light_from_world.to_cols_array());
    data[16] = 2.0 / SHADOW_BOX;
    data[17] = if enabled { 1.0 } else { 0.0 };
    data
}

/// The shadow texture and light-view buffer, created on first use (the terrain
/// binds them even where no shadow camera runs: the viewer, tests).
pub fn character_shadow(world: &mut World) -> CharacterShadow {
    if let Some(shadow) = world.get_resource::<CharacterShadow>() {
        return shadow.clone();
    }
    let size = Extent3d { width: SHADOW_TEXTURE_SIZE, height: SHADOW_TEXTURE_SIZE, depth_or_array_layers: 1 };
    let mut image = Image::new_fill(size, TextureDimension::D2, &[255; 4], TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
    image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    let texture = world.resource_mut::<Assets<Image>>().add(image);
    let data = view_data(Mat4::IDENTITY, false).to_vec();
    let view = world.resource_mut::<Assets<ShaderBuffer>>().add(ShaderBuffer::new(data, RenderAssetUsages::default()));
    let receiver = world
        .resource_mut::<Assets<ShadowReceiverMaterial>>()
        .add(ShadowReceiverMaterial { view: view.clone(), texture: texture.clone() });
    let shadow = CharacterShadow { texture, view, receiver };
    world.insert_resource(shadow.clone());
    shadow
}

fn spawn_shadow_camera(world: &mut World) {
    let texture = character_shadow(world).texture;
    world.spawn((
        Name::new("character shadow camera"),
        ShadowCamera,
        Camera3d::default(),
        Camera { order: -1, is_active: false, clear_color: ClearColorConfig::Custom(Color::WHITE), ..default() },
        RenderTarget::Image(texture.into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed { width: SHADOW_BOX, height: SHADOW_BOX },
            near: SHADOW_NEAR,
            far: SHADOW_FAR,
            ..OrthographicProjection::default_3d()
        }),
        RenderLayers::layer(SHADOW_LAYER),
        // Marks the shadow pass for `fixed_function.wesl` (exposure underflows to 0).
        Exposure { ev100: SHADOW_PASS_EV100 },
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Off,
        NoIndirectDrawing,
        ClusterConfig::None,
        Transform::default(),
    ));
}

/// Centres the shadow box on the camera target and publishes the light view.
fn follow_target(
    mut target: ResMut<SceneCameraTarget>,
    shadow: Option<Res<CharacterShadow>>,
    mut cameras: Query<(&mut Camera, &mut Transform), With<ShadowCamera>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut last: Local<Option<Option<Vec3>>>,
) {
    let target = target.0.take();
    let (Some(shadow), Ok((mut camera, mut transform))) = (shadow, cameras.single_mut()) else { return };
    if *last == Some(target) {
        return;
    }
    *last = Some(target);
    camera.is_active = target.is_some();
    let data = match target {
        Some(target) => {
            *transform = Transform::from_translation(target + eye_offset()).looking_at(target, Vec3::Y);
            view_data(transform.to_matrix().inverse(), true)
        }
        None => view_data(Mat4::IDENTITY, false),
    };
    if let Some(mut buffer) = buffers.get_mut(&shadow.view) {
        buffer.clear();
        buffer.extend_from_slice(&data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_light_looks_down_from_the_client_eye() {
        // Metin2 (-√3, -1, 2√3): up (Bevy +y) twice as much as sideways.
        let eye = eye_offset();
        assert!(eye.y > 0.0);
        assert!((eye.length() - 1250.0 * 4.0).abs() < 1e-2);
        assert!((eye.y / Vec2::new(eye.x, eye.z).length() - 3f32.sqrt()).abs() < 1e-4);
    }
}
